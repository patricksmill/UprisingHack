// Lifts functions to C Overrides from Ghidra's high p-code: each function's
// semantics as the decompiler sees them (variables, not registers and flags),
// with every memory access going through the guest's memory and every call
// through the guest calling convention, so the result can be checked against
// the generated code by scripts/difftest.c.
//
// args: <address list file> <out .c> <report file> [result contracts]
// The list holds one hex address per line. Functions the lifter cannot
// express yet are skipped with the reason in the report.
// @category Icarus
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSpace;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.VariableStorage;
import ghidra.program.model.data.UnsignedIntegerDataType;
import ghidra.program.model.data.UnsignedLongLongDataType;
import ghidra.program.model.data.VoidDataType;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.pcode.*;

import java.io.PrintWriter;
import java.io.IOException;
import ghidra.app.decompiler.DecompileException;
import static ghidra.program.model.pcode.ElementId.*;
import static ghidra.program.model.pcode.AttributeId.*;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;

public class LiftOverrides extends GhidraScript {
    // Ghidra's trivialarith rule assumes x == x even for NaN; recovery must retain that test.
    static class SemanticDecompiler extends DecompInterface {
        SemanticDecompiler() {
            DecompileOptions options = new DecompileOptions();
            options.setNanIgnore(DecompileOptions.NanIgnoreEnum.None);
            setOptions(options);
            toggleSyntaxTree(true);
            setSimplificationStyle("decompile");
        }

        @Override
        protected void initializeProcess() throws IOException, DecompileException {
            super.initializeProcess();
            PatchPackedEncode enc = new PatchPackedEncode();
            enc.clear();
            enc.openElement(ELEM_OPTIONSLIST);
            enc.openElement(ELEM_TOGGLERULE);
            enc.openElement(ELEM_PARAM1);
            enc.writeString(ATTRIB_CONTENT, "trivialarith");
            enc.closeElement(ELEM_PARAM1);
            enc.openElement(ELEM_PARAM2);
            enc.writeString(ATTRIB_CONTENT, "off");
            enc.closeElement(ELEM_PARAM2);
            enc.closeElement(ELEM_TOGGLERULE);
            enc.closeElement(ELEM_OPTIONSLIST);
            StringIngest response = new StringIngest();
            decompProcess.sendCommand1Param("setOptions", enc, response);
            if (!response.toString().equals("t")) throw new IOException("Cannot disable trivialarith");
        }
    }

    static class Skip extends Exception {
        Skip(String why) { super(why); }
    }

    private HighFunction hf;
    private Map<HighVariable, String> names;
    private StringBuilder body;
    private int instructionsInFunction;
    private long lowestStack; // the lowest stack offset the function names, below its entry sp

    @Override
    protected void run() throws Exception {
        // Ghidra rolls nested aborts back when its script-wide transaction ends.
        int transaction = currentProgram.startTransaction("Provisional result contracts");
        try {
            if (getScriptArgs().length > 3) {
                DecompInterface pre = new SemanticDecompiler();
                try {
                    if (!pre.openProgram(currentProgram)) throw new IOException(pre.getLastMessage());
                    applyResultContracts(Path.of(getScriptArgs()[3]), pre);
                } finally {
                    pre.dispose();
                }
            }
            liftFunctions();
        } finally {
            currentProgram.endTransaction(transaction, false);
        }
    }

    // Result words are listed least significant first, independently of their register bank.
    private void applyResultContracts(Path file, DecompInterface pre) throws Exception {
        boolean hashChecked = false;
        Set<Long> addresses = new HashSet<>();
        for (String line : Files.readAllLines(file)) {
            line = line.split("#", 2)[0].trim();
            if (line.isEmpty()) continue;
            String[] fields = line.split("\\s+");
            if (!hashChecked) {
                if (fields.length != 2 || !fields[0].equals("sha256") || !fields[1].matches("[0-9a-fA-F]{64}")
                        || !fields[1].equalsIgnoreCase(currentProgram.getExecutableSHA256()))
                    throw new IOException("Result contracts do not match the program's executable SHA-256");
                hashChecked = true;
                continue;
            }
            if (fields.length != 2 || !fields[0].matches("[0-9a-fA-F]{8}"))
                throw new IOException("Invalid result contract: " + line);
            long address = Long.parseLong(fields[0], 16);
            String[] words = fields[1].split(",", -1);
            if (!addresses.add(address) || words.length > 2 || new HashSet<>(Arrays.asList(words)).size() != words.length)
                throw new IOException("Duplicate address/register or result exceeds 64 bits: " + line);
            Register[] pieces = new Register[words.length];
            for (int i = 0; i < words.length; i++) {
                if (!words[i].matches("r[0-3]|s([0-9]|1[0-5])")) throw new IOException("Invalid result register: " + words[i]);
                pieces[words.length - 1 - i] = currentProgram.getRegister(words[i]);
                if (pieces[words.length - 1 - i] == null) throw new IOException("Missing register: " + words[i]);
            }
            Function callee = getFunctionAt(toAddr(address));
            if (callee == null) throw new IOException("No function at " + fields[0]);
            DecompileResults r = pre.decompileFunction(callee, 30, monitor);
            if (!r.decompileCompleted()) throw new IOException(r.getErrorMessage());
            HighFunctionDBUtil.commitParamsToDatabase(r.getHighFunction(), true,
                HighFunctionDBUtil.ReturnCommitOption.NO_COMMIT, SourceType.ANALYSIS);
            callee.setCustomVariableStorage(true);
            callee.setReturn(words.length == 2 ? UnsignedLongLongDataType.dataType : UnsignedIntegerDataType.dataType,
                new VariableStorage(currentProgram, pieces), SourceType.USER_DEFINED);
            pre.flushCache();
        }
        if (!hashChecked || addresses.isEmpty()) throw new IOException("Empty result contracts");
    }

    private void liftFunctions() throws Exception {
        String[] args = getScriptArgs();
        List<String> wanted = Files.readAllLines(Path.of(args[0]));
        DecompInterface decomp = new SemanticDecompiler();
        if (!decomp.openProgram(currentProgram)) throw new IOException("Open failed: " + decomp.getLastMessage());
        int lifted = 0, skipped = 0;
        try (PrintWriter out = new PrintWriter(args[1]); PrintWriter report = new PrintWriter(args[2])) {
            out.println("/* lifted by ghidra/LiftOverrides.java from Ghidra's high p-code; not hand-checked */");
            out.println("#include \"lifted.h\"\n");
            for (String line : wanted) {
                line = line.trim();
                if (line.isEmpty()) continue;
                Address at = toAddr(Long.parseLong(line, 16));
                Function f = getFunctionAt(at);
                if (f == null) { report.println(line + " skip no function"); skipped++; continue; }
                DecompileResults r = decomp.decompileFunction(f, 30, monitor);
                if (!r.decompileCompleted()) { report.println(line + " skip decompile failed"); skipped++; continue; }
                int transaction = currentProgram.startTransaction("Observe machine return");
                try {
                    // A return-type edit locks the signature, including an
                    // empty parameter list. Keep inferred inputs first. These
                    // provisional edits persist until the script-wide rollback.
                    HighFunction original = r.getHighFunction();
                    String c = lift(f, original);
                    FunctionPrototype prototype = original.getFunctionPrototype();
                    boolean deadStackInput = false;
                    Iterator<PcodeOpAST> inputOps = original.getPcodeOps();
                    while (inputOps.hasNext()) {
                        for (Varnode v : inputOps.next().getInputs())
                            deadStackInput |= v.isInput() && v.getAddress().isStackAddress()
                                && v.getOffset() < 0;
                    }
                    int originalFrame = f.getStackFrame().getFrameSize();
                    boolean narrowInput = false;
                    for (int i = 0; i < prototype.getNumParams(); i++)
                        narrowInput |= prototype.getParam(i).getDataType().getLength() < 4;
                    if (prototype.getReturnType() instanceof VoidDataType && !narrowInput && !deadStackInput) {
                        List<String> calls = callShapes(original);
                        HighFunctionDBUtil.commitParamsToDatabase(original, true,
                            HighFunctionDBUtil.ReturnCommitOption.NO_COMMIT, SourceType.ANALYSIS);
                        f.setReturnType(UnsignedIntegerDataType.dataType, SourceType.USER_DEFINED);
                        decomp.flushCache();
                        r = decomp.decompileFunction(f, 30, monitor);
                        if (r.decompileCompleted() && originalFrame == f.getStackFrame().getFrameSize()
                                && calls.equals(callShapes(r.getHighFunction()))) {
                            try {
                                c = lift(f, r.getHighFunction());
                            } catch (Skip unsupportedRecovery) {
                                // Retain the original lift if observing r0 exposes unsupported p-code.
                            }
                        }
                    }
                    out.println(c);
                    report.println(line + " lifted");
                    lifted++;
                } catch (Skip s) {
                    report.println(line + " skip " + s.getMessage());
                    skipped++;
                } finally {
                    currentProgram.endTransaction(transaction, false);
                    decomp.flushCache();
                }
            }
        }
        println("lifted " + lifted + ", skipped " + skipped);
    }

    // Return inference may rewrite a tail call's inferred arguments. Do not
    // trade a missing return for a changed call contract. Narrow inputs are
    // also left alone: inferred byte types discard the incoming upper bits.
    private List<String> callShapes(HighFunction high) {
        List<String> shapes = new ArrayList<>();
        Iterator<PcodeOpAST> ops = high.getPcodeOps();
        while (ops.hasNext()) {
            PcodeOp op = ops.next();
            if (op.getOpcode() != PcodeOp.CALL) continue;
            StringBuilder shape = new StringBuilder(op.getSeqnum().getTarget().toString());
            for (int i = 0; i < op.getNumInputs(); i++) {
                Varnode input = op.getInput(i);
                shape.append(':').append(input.getSize()).append('/').append(isFloat(input));
                if (i == 0) shape.append('/').append(input.getOffset());
            }
            shapes.add(shape.toString());
        }
        return shapes;
    }

    private String lift(Function f, HighFunction high) throws Skip {
        hf = high;
        names = new HashMap<>();
        body = new StringBuilder();
        instructionsInFunction = 0;
        lowestStack = 0;
        String hex = String.format("0x%08X", f.getEntryPoint().getOffset());
        StringBuilder decls = new StringBuilder();

        // every block is a label; ops in order, with phi copies at the ends
        ArrayList<PcodeBlockBasic> blocks = hf.getBasicBlocks();
        Map<PcodeBlockBasic, Integer> index = new HashMap<>();
        for (int i = 0; i < blocks.size(); i++) index.put(blocks.get(i), i);
        for (int i = 0; i < blocks.size(); i++) {
            PcodeBlockBasic b = blocks.get(i);
            body.append("b").append(i).append(":\n");
            int count = instructionCount(b);
            if (count > 0) body.append("    lift_charge(ctx, ").append(count).append(");\n");
            Iterator<PcodeOp> ops = b.getIterator();
            PcodeOp last = null;
            List<PcodeOp> list = new ArrayList<>();
            while (ops.hasNext()) list.add(ops.next());
            for (PcodeOp op : list) {
                int code = op.getOpcode();
                if (code == PcodeOp.BRANCH || code == PcodeOp.CBRANCH || code == PcodeOp.RETURN) { last = op; break; }
                statement(op);
            }
            // phi copies for each successor, before the jump
            if (last != null && last.getOpcode() == PcodeOp.CBRANCH) {
                // true edge is out(1) per Ghidra's convention for CBRANCH blocks
                PcodeBlock t = b.getTrueOut(), fl = b.getFalseOut();
                body.append("    if (").append(expr(last.getInput(1))).append(") {\n");
                phiCopies(b, t);
                body.append("        goto b").append(index.get(t)).append(";\n    }\n");
                phiCopies(b, fl);
                body.append("    goto b").append(index.get(fl)).append(";\n");
            } else if (last != null && last.getOpcode() == PcodeOp.RETURN) {
                if (last.getNumInputs() > 1) {
                    Varnode v = last.getInput(1);
                    if (f.hasCustomVariableStorage()) {
                        VariableStorage storage = hf.getFunctionPrototype().getReturnStorage();
                        if (storage.size() != v.getSize()) throw new Skip("return storage size");
                        body.append("    { uint64_t result = ").append(widen(v)).append(";\n");
                        int shift = storage.size() * 8;
                        for (Varnode piece : storage.getVarnodes()) {
                            shift -= piece.getSize() * 8;
                            body.append("        ").append(resultRegister(piece)).append(" = (uint32_t)(result >> ").append(shift).append(");\n");
                        }
                        body.append("    }\n");
                    } else {
                        body.append("    lift_return_value(ctx, ").append(widen(v)).append(", ").append(v.getSize()).append(", ")
                            .append(isFloat(v) ? 1 : 0).append(");\n");
                    }
                }
                body.append("    LIFT_RETURN();\n");
            } else {
                if (b.getOutSize() != 1) {
                    if (b.getOutSize() == 0) { body.append("    LIFT_RETURN();\n"); continue; }
                    throw new Skip("block " + i + " ends without a branch and has " + b.getOutSize() + " ways out");
                }
                PcodeBlock next = b.getOut(0);
                phiCopies(b, next);
                body.append("    goto b").append(index.get(next)).append(";\n");
            }
        }
        // a variable that starts as an input holds it from the start
        for (Map.Entry<HighVariable, String> e : new ArrayList<>(names.entrySet())) {
            String init = "0";
            for (Varnode instance : e.getKey().getInstances())
                if (instance.isInput()) { init = input(instance); break; }
            decls.append("    ").append(ctype(e.getKey().getSize())).append(" ").append(e.getValue()).append(" = (")
                .append(ctype(e.getKey().getSize())).append(")(").append(init).append(");\n");
        }
        // Ghidra's frame can be smaller than what the function uses; a call
        // made with sp inside the locals would write over them
        int frame = (int) ((Math.max(f.getStackFrame().getFrameSize(), -lowestStack) + 7) & ~7);
        return "/* " + f.getName() + " */\nRECOMP_OVERRIDE(" + hex + ") {\n    LIFT_ENTER(" + hex + ", " + frame + ");\n" + decls
            + "    goto b0;\n" + body + "}\n";
    }

    private int instructionCount(PcodeBlockBasic b) {
        Address start = b.getStart(), stop = b.getStop();
        if (start == null || stop == null || !start.isMemoryAddress()) return 0;
        long n = 0;
        var it = currentProgram.getListing().getInstructions(new ghidra.program.model.address.AddressSet(start, stop), true);
        while (it.hasNext()) { it.next(); n++; }
        return (int) n;
    }

    private void phiCopies(PcodeBlockBasic from, PcodeBlock to) throws Skip {
        if (!(to instanceof PcodeBlockBasic)) return;
        int slot = -1;
        for (int i = 0; i < to.getInSize(); i++) if (to.getIn(i) == from) slot = i;
        if (slot < 0) return;
        Iterator<PcodeOp> it = ((PcodeBlockBasic) to).getIterator();
        while (it.hasNext()) {
            PcodeOp op = it.next();
            if (op.getOpcode() != PcodeOp.MULTIEQUAL) continue;
            Varnode out = op.getOutput(), in = op.getInput(slot);
            if (isMemory(out)) {
                if (!isMemory(in) && !in.isConstant()) assign(out, value(in), "        ");
                continue;
            }
            // A shared high variable does not keep its C local current when a callee writes the stack.
            if (out.getHigh() == in.getHigh() && !isMemory(in)) continue;
            assign(out, value(in), "        ");
        }
    }

    // ---- statements

    private void statement(PcodeOp op) throws Skip {
        int code = op.getOpcode();
        switch (code) {
            case PcodeOp.MULTIEQUAL: return; // done on the edges
            case PcodeOp.INDIRECT: {
                Varnode out = op.getOutput(), in = op.getInput(0);
                // a value held in a register until a call or store that may
                // read the stack slot it belongs in: this is where it lands
                if (isMemory(out)) {
                    if (!isMemory(in) && !in.isConstant()) assign(out, value(in), "    ");
                    return;
                }
                if (out.getHigh() == in.getHigh()) return;
                assign(out, value(in), "    ");
                return;
            }
            case PcodeOp.STORE: {
                Varnode ptr = op.getInput(1), v = op.getInput(2);
                body.append("    lift_write(ctx, ").append(widen(ptr)).append(", ").append(v.getSize()).append(", ")
                    .append(widen(v)).append(");\n");
                return;
            }
            case PcodeOp.CALL: { call(op); return; }
            case PcodeOp.CALLIND: throw new Skip("indirect call");
            case PcodeOp.BRANCHIND: throw new Skip("indirect branch");
            case PcodeOp.CALLOTHER: throw new Skip("callother " + op.getInput(0).getOffset());
            default: {
                Varnode out = op.getOutput();
                if (out == null) throw new Skip("op without output " + op.getMnemonic());
                // Ghidra's memory-state bookkeeping is not an executable memory access.
                if (code == PcodeOp.COPY && isMemory(out) && isMemory(op.getInput(0))
                        && out.getSize() == op.getInput(0).getSize()
                        && out.getAddress().equals(op.getInput(0).getAddress())) return;
                if (!explicit(out)) return; // printed where it is used
                assign(out, render(op), "    ");
            }
        }
    }

    private void assign(Varnode out, String value, String indent) throws Skip {
        if (isMemory(out)) {
            body.append(indent).append("lift_write(ctx, ").append(memoryAddress(out)).append(", ").append(out.getSize())
                .append(", ").append(value).append(");\n");
        } else {
            body.append(indent).append(name(out)).append(" = (").append(ctype(out.getSize())).append(")(").append(value).append(");\n");
        }
    }

    private void call(PcodeOp op) throws Skip {
        Varnode target = op.getInput(0);
        if (!target.isAddress()) throw new Skip("call target not an address");
        StringBuilder s = new StringBuilder("    {\n        LiftArgs a = {0};\n");
        for (int i = 1; i < op.getNumInputs(); i++) {
            Varnode v = op.getInput(i);
            s.append("        lift_arg(&a, ").append(widen(v)).append(", ").append(v.getSize()).append(", ").append(isFloat(v) ? 1 : 0)
                .append(");\n");
        }
        s.append(String.format("        lift_call(ctx, &a, 0x%08Xu);\n", target.getOffset()));
        Varnode out = op.getOutput();
        if (out != null) {
            Function callee = getFunctionAt(target.getAddress());
            VariableStorage storage = callee == null ? null : callee.getReturn().getVariableStorage();
            String r;
            if (callee != null && callee.hasCustomVariableStorage()) {
                if (storage.size() != out.getSize()) throw new Skip("call result storage size");
                r = storageRead(storage);
            } else {
                r = "lift_result(ctx, " + out.getSize() + ", " + (isFloat(out) ? 1 : 0) + ")";
            }
            if (isMemory(out)) s.append("        lift_write(ctx, ").append(memoryAddress(out)).append(", ").append(out.getSize()).append(", ").append(r).append(");\n");
            else s.append("        ").append(name(out)).append(" = (").append(ctype(out.getSize())).append(")").append(r).append(";\n");
        }
        s.append("    }\n");
        body.append(s);
    }

    private String resultRegister(Varnode v) throws Skip {
        if (!v.isRegister() || v.getSize() != 4) throw new Skip("unsupported result storage " + v);
        Register reg = currentProgram.getRegister(v.getAddress(), v.getSize());
        if (reg == null) throw new Skip("unknown result register");
        String n = reg.getName().toLowerCase();
        if (n.matches("r[0-3]")) return "ctx->r[" + n.substring(1) + "]";
        if (n.matches("s([0-9]|1[0-5])")) return "ctx->vfp[" + n.substring(1) + "]";
        throw new Skip("unsupported result register " + n);
    }

    private String storageRead(VariableStorage storage) throws Skip {
        if (storage.size() > 8 || storage.size() == 0) throw new Skip("unsupported result size");
        String value = "0ULL";
        for (Varnode piece : storage.getVarnodes())
            value = "((" + value + ") << " + (piece.getSize() * 8) + ") | (uint64_t)(" + resultRegister(piece) + ")";
        return value;
    }

    // ---- expressions

    // a varnode gets a variable of its own unless the decompiler would print
    // its defining op inline: used once, in the same block, no memory
    private boolean explicit(Varnode out) {
        if (isMemory(out)) return true;
        HighVariable h = out.getHigh();
        if (h == null) return true;
        if (h instanceof HighLocal || h instanceof HighParam || h instanceof HighGlobal) return true;
        if (h.getInstances().length > 1) return true;
        Iterator<PcodeOp> uses = out.getDescendants();
        int n = 0;
        PcodeOp only = null;
        while (uses.hasNext()) { only = uses.next(); n++; }
        if (n != 1) return true;
        PcodeOp def = out.getDef();
        if (only.getParent() != def.getParent()) return true;
        int code = only.getOpcode();
        if (code == PcodeOp.MULTIEQUAL || code == PcodeOp.INDIRECT) return true;
        // an inline LOAD must not move past a STORE or call between def and use
        if (def.getOpcode() == PcodeOp.LOAD || def.getOpcode() == PcodeOp.CALL) return true;
        return false;
    }

    private String value(Varnode v) throws Skip {
        if (v.isConstant()) return constant(v);
        if (isMemory(v)) return "lift_read(ctx, " + memoryAddress(v) + ", " + v.getSize() + ")";
        PcodeOp def = v.getDef();
        if (def == null || v.isInput()) return truncated(input(v), v.getSize());
        if (!explicit(v)) return truncated(render(def), v.getSize());
        return name(v);
    }

    // a value printed where it is used has no variable to cut it to its
    // size: a byte taken from a word, a narrow sum, a byte in a register
    private static String truncated(String value, int size) {
        return size >= 8 ? "(" + value + ")" : "((" + value + ") & " + mask(size) + ")";
    }

    private String expr(Varnode v) throws Skip { return value(v); }

    private String widen(Varnode v) throws Skip { return "(uint64_t)(" + value(v) + ")"; }

    private String constant(Varnode v) {
        long c = v.getOffset();
        if (v.getSize() >= 8) return String.format("0x%016XULL", c);
        return String.format("0x%XU", c & 0xFFFFFFFFL);
    }

    private String input(Varnode v) throws Skip {
        if (v.isRegister()) {
            Register r = currentProgram.getRegister(v.getAddress(), v.getSize());
            if (r == null) r = currentProgram.getRegister(v.getAddress());
            if (r == null) throw new Skip("input register at " + v.getAddress());
            String n = r.getName().toLowerCase();
            if (n.matches("r\\d+")) return "lift_in_r[" + n.substring(1) + "]";
            if (n.equals("sp")) return "lift_entry_sp";
            if (n.equals("lr")) return "lift_entry_lr";
            if (n.matches("s\\d+")) return "lift_in_vfp[" + n.substring(1) + "]";
            if (n.matches("d\\d+")) {
                int d = Integer.parseInt(n.substring(1));
                return "((uint64_t)lift_in_vfp[" + (2 * d + 1) + "] << 32 | lift_in_vfp[" + (2 * d) + "])";
            }
            throw new Skip("input register " + n);
        }
        if (v.getAddress().isStackAddress()) {
            lowestStack = Math.min(lowestStack, v.getOffset());
            return "lift_read(ctx, lift_entry_sp + " + v.getOffset() + "u, " + v.getSize() + ")";
        }
        if (v.isUnique()) return name(v);
        if (isMemory(v)) return "lift_read(ctx, " + memoryAddress(v) + ", " + v.getSize() + ")"; // a global as it was on the way in
        throw new Skip("input " + v);
    }

    private boolean isMemory(Varnode v) {
        if (v.isConstant() || v.isUnique() || v.isRegister()) return false;
        AddressSpace s = v.getAddress().getAddressSpace();
        return s.isMemorySpace() || s.isStackSpace();
    }

    private String memoryAddress(Varnode v) {
        if (v.getAddress().isStackAddress()) {
            lowestStack = Math.min(lowestStack, v.getOffset());
            return "lift_entry_sp + (uint32_t)" + v.getOffset();
        }
        return String.format("0x%08XU", v.getOffset());
    }

    private String name(Varnode v) throws Skip {
        HighVariable h = v.getHigh();
        if (h == null) throw new Skip("varnode without a high variable");
        return names.computeIfAbsent(h, k -> "v" + names.size());
    }

    private static String ctype(int size) {
        switch (size) {
            case 1: return "uint8_t";
            case 2: return "uint16_t";
            case 4: return "uint32_t";
            case 8: return "uint64_t";
            default: return "uint64_t";
        }
    }

    private boolean isFloat(Varnode v) {
        HighVariable h = v.getHigh();
        if (h != null && h.getDataType() instanceof ghidra.program.model.data.AbstractFloatDataType) return true;
        PcodeOp def = v.getDef();
        if (def != null) {
            int c = def.getOpcode();
            if (c >= PcodeOp.FLOAT_EQUAL && c <= PcodeOp.FLOAT_ROUND && c != PcodeOp.FLOAT_EQUAL && c != PcodeOp.FLOAT_NOTEQUAL
                && c != PcodeOp.FLOAT_LESS && c != PcodeOp.FLOAT_LESSEQUAL && c != PcodeOp.FLOAT_NAN && c != PcodeOp.FLOAT_TRUNC)
                return true;
        }
        return false;
    }

    private String render(PcodeOp op) throws Skip {
        int size = op.getOutput() == null ? 4 : op.getOutput().getSize();
        String m = "0x" + Long.toHexString(size >= 8 ? -1L : (1L << (8 * size)) - 1) + "ULL";
        String a = op.getNumInputs() > 0 ? value(op.getInput(0)) : "";
        String b = op.getNumInputs() > 1 ? value(op.getInput(1)) : "";
        int sa = op.getNumInputs() > 0 ? op.getInput(0).getSize() : 4;
        String sx = "lift_sext(" + a + ", " + sa + ")";
        String sy = op.getNumInputs() > 1 ? "lift_sext(" + b + ", " + op.getInput(1).getSize() + ")" : "";
        switch (op.getOpcode()) {
            case PcodeOp.COPY: case PcodeOp.CAST: return a;
            case PcodeOp.LOAD: return "lift_read(ctx, " + widen(op.getInput(1)) + ", " + size + ")";
            case PcodeOp.INT_ADD: return "(" + a + ") + (" + b + ")";
            case PcodeOp.INT_SUB: return "(" + a + ") - (" + b + ")";
            case PcodeOp.INT_MULT: return "(uint64_t)(" + a + ") * (uint64_t)(" + b + ")";
            case PcodeOp.INT_DIV: return "lift_udiv(" + a + ", " + b + ")";
            case PcodeOp.INT_REM: return "lift_urem(" + a + ", " + b + ")";
            case PcodeOp.INT_SDIV: return "lift_sdiv(" + sx + ", " + sy + ")";
            case PcodeOp.INT_SREM: return "lift_srem(" + sx + ", " + sy + ")";
            case PcodeOp.INT_AND: return "(" + a + ") & (" + b + ")";
            case PcodeOp.INT_OR: return "(" + a + ") | (" + b + ")";
            case PcodeOp.INT_XOR: return "(" + a + ") ^ (" + b + ")";
            case PcodeOp.INT_NEGATE: return "~(" + a + ")";
            case PcodeOp.INT_2COMP: return "-(" + a + ")";
            case PcodeOp.INT_LEFT: return "lift_shl(" + a + ", " + b + ", " + size + ")";
            case PcodeOp.INT_RIGHT: return "lift_shr((" + a + ") & " + mask(sa) + ", " + b + ", " + sa + ")";
            case PcodeOp.INT_SRIGHT: return "lift_sar(" + sx + ", " + b + ", " + sa + ")";
            case PcodeOp.INT_EQUAL: return "(" + a + ") == (" + b + ")";
            case PcodeOp.INT_NOTEQUAL: return "(" + a + ") != (" + b + ")";
            case PcodeOp.INT_LESS: return "((" + a + ") & " + mask(sa) + ") < ((" + b + ") & " + mask(sa) + ")";
            case PcodeOp.INT_LESSEQUAL: return "((" + a + ") & " + mask(sa) + ") <= ((" + b + ") & " + mask(sa) + ")";
            case PcodeOp.INT_SLESS: return sx + " < " + sy;
            case PcodeOp.INT_SLESSEQUAL: return sx + " <= " + sy;
            case PcodeOp.INT_CARRY: return "lift_carry(" + a + ", " + b + ", " + sa + ")";
            case PcodeOp.INT_SCARRY: return "lift_scarry(" + a + ", " + b + ", " + sa + ")";
            case PcodeOp.INT_SBORROW: return "lift_sborrow(" + a + ", " + b + ", " + sa + ")";
            case PcodeOp.INT_ZEXT: return "(uint64_t)((" + a + ") & " + mask(sa) + ")";
            case PcodeOp.INT_SEXT: return "(uint64_t)" + sx;
            case PcodeOp.BOOL_NEGATE: return "!(" + a + ")";
            case PcodeOp.BOOL_AND: return "(" + a + ") && (" + b + ")";
            case PcodeOp.BOOL_OR: return "(" + a + ") || (" + b + ")";
            case PcodeOp.BOOL_XOR: return "(!!(" + a + ")) ^ (!!(" + b + "))";
            case PcodeOp.PIECE: return "(((uint64_t)(" + a + ") << " + (8 * op.getInput(1).getSize()) + ") | ((" + b + ") & "
                + mask(op.getInput(1).getSize()) + "))";
            case PcodeOp.SUBPIECE: return "lift_shr((uint64_t)(" + a + "), " + (8 * op.getInput(1).getOffset()) + ", 8)";
            case PcodeOp.PTRADD: return "(" + a + ") + (" + b + ") * " + constant(op.getInput(2));
            case PcodeOp.PTRSUB: {
                Varnode base = op.getInput(0);
                if (base.isConstant() && base.getOffset() == 0) return b; // a global's address
                if (base.isRegister() || (base.getAddress() != null && base.getAddress().isStackAddress()) || isStackBase(base)) {
                    if (op.getInput(1).isConstant()) lowestStack = Math.min(lowestStack, (int) op.getInput(1).getOffset());
                    return "lift_entry_sp + (uint32_t)(" + b + ")";
                }
                return "(" + a + ") + (" + b + ")";
            }
            case PcodeOp.POPCOUNT: return "__builtin_popcountll((uint64_t)(" + a + ") & " + mask(sa) + ")";
            case PcodeOp.LZCOUNT: return "lift_lzcount(" + a + ", " + sa + ")";
            // VFP arithmetic as the generated code does it: plain C on the
            // values, results flushed to zero when fpscr asks
            case PcodeOp.FLOAT_ADD: return flt(sa, a, "+", b);
            case PcodeOp.FLOAT_SUB: return flt(sa, a, "-", b);
            case PcodeOp.FLOAT_MULT: return flt(sa, a, "*", b);
            case PcodeOp.FLOAT_DIV: return flt(sa, a, "/", b);
            case PcodeOp.FLOAT_NEG: return "(" + a + ") ^ " + (sa == 8 ? "0x8000000000000000ULL" : "0x80000000ULL");
            case PcodeOp.FLOAT_ABS: return "(" + a + ") & " + (sa == 8 ? "0x7FFFFFFFFFFFFFFFULL" : "0x7FFFFFFFULL");
            case PcodeOp.FLOAT_SQRT: return sa == 8 ? "lift_dbits(ctx, sqrt(lift_d(" + a + ")))" : "lift_fbits(ctx, sqrtf(lift_f(" + a + ")))";
            case PcodeOp.FLOAT_EQUAL: return fcmp(sa, a, "==", b);
            case PcodeOp.FLOAT_NOTEQUAL: return fcmp(sa, a, "!=", b);
            case PcodeOp.FLOAT_LESS: return fcmp(sa, a, "<", b);
            case PcodeOp.FLOAT_LESSEQUAL: return fcmp(sa, a, "<=", b);
            case PcodeOp.FLOAT_NAN: return sa == 8 ? "isnan(lift_d(" + a + "))" : "isnan(lift_f(" + a + "))";
            case PcodeOp.FLOAT_INT2FLOAT: return size == 8 ? "lift_dbits(ctx, (double)" + sx + ")" : "lift_fbits(ctx, (float)" + sx + ")";
            case PcodeOp.FLOAT_FLOAT2FLOAT:
                if (sa == 4 && size == 8) return "lift_dbits(ctx, (double)lift_f(" + a + "))";
                if (sa == 8 && size == 4) return "lift_fbits(ctx, (float)lift_d(" + a + "))";
                throw new Skip("float2float " + sa + " to " + size);
            case PcodeOp.FLOAT_TRUNC:
                if (size != 4) throw new Skip("trunc to " + size);
                return "vfp_to_s32(" + (sa == 8 ? "lift_d(" + a + ")" : "(double)lift_f(" + a + ")") + ")";
            case PcodeOp.FLOAT_CEIL: return sa == 8 ? "lift_dbits(ctx, ceil(lift_d(" + a + ")))" : "lift_fbits(ctx, ceilf(lift_f(" + a + ")))";
            case PcodeOp.FLOAT_FLOOR: return sa == 8 ? "lift_dbits(ctx, floor(lift_d(" + a + ")))" : "lift_fbits(ctx, floorf(lift_f(" + a + ")))";
            case PcodeOp.FLOAT_ROUND: return sa == 8 ? "lift_dbits(ctx, round(lift_d(" + a + ")))" : "lift_fbits(ctx, roundf(lift_f(" + a + ")))";
            default: throw new Skip("op " + op.getMnemonic());
        }
    }

    private static String flt(int size, String a, String operator, String b) throws Skip {
        if (size == 8) return "lift_dbits(ctx, lift_d(" + a + ") " + operator + " lift_d(" + b + "))";
        if (size == 4) return "lift_fbits(ctx, lift_f(" + a + ") " + operator + " lift_f(" + b + "))";
        throw new Skip("float of size " + size);
    }

    private static String fcmp(int size, String a, String operator, String b) throws Skip {
        if (size == 8) return "(lift_d(" + a + ") " + operator + " lift_d(" + b + "))";
        if (size == 4) return "(lift_f(" + a + ") " + operator + " lift_f(" + b + "))";
        throw new Skip("float compare of size " + size);
    }

    private boolean isStackBase(Varnode v) {
        return v.isRegister() && "sp".equalsIgnoreCase(String.valueOf(currentProgram.getRegister(v.getAddress(), v.getSize())));
    }

    private static String mask(int size) {
        return size >= 8 ? "0xFFFFFFFFFFFFFFFFULL" : String.format("0x%XULL", (1L << (8 * size)) - 1);
    }
}
