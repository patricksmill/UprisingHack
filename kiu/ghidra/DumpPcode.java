// Writes the high p-code ghidra/LiftOverrides.java lifts from, a block and
// an op a line, for looking into a lifted function that differs.
//
// args: <address list file> <out file>
// @category Icarus
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.pcode.*;

import java.io.PrintWriter;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;

public class DumpPcode extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        DecompInterface decomp = new LiftOverrides.SemanticDecompiler();
        if (!decomp.openProgram(currentProgram)) throw new IllegalStateException(decomp.getLastMessage());
        try (PrintWriter out = new PrintWriter(args[1])) {
            for (String line : Files.readAllLines(Path.of(args[0]))) {
                line = line.trim();
                if (line.isEmpty()) continue;
                Function f = getFunctionAt(toAddr(Long.parseLong(line, 16)));
                if (f == null) { out.println("== " + line + " no function"); continue; }
                DecompileResults r = decomp.decompileFunction(f, 30, monitor);
                if (!r.decompileCompleted()) { out.println("== " + line + " decompile failed"); continue; }
                HighFunction hf = r.getHighFunction();
                out.println("== " + line + " " + f.getSignature().getPrototypeString() + " frame " + f.getStackFrame().getFrameSize());
                ArrayList<PcodeBlockBasic> blocks = hf.getBasicBlocks();
                for (int i = 0; i < blocks.size(); i++) {
                    out.println("b" + i + " " + blocks.get(i).getStart() + ":");
                    Iterator<PcodeOp> ops = blocks.get(i).getIterator();
                    while (ops.hasNext()) {
                        PcodeOp op = ops.next();
                        StringBuilder s = new StringBuilder("  " + op.getSeqnum().getTarget() + " ");
                        if (op.getOutput() != null) s.append(show(op.getOutput())).append(" = ");
                        s.append(op.getMnemonic());
                        for (Varnode v : op.getInputs()) s.append(' ').append(show(v));
                        out.println(s);
                    }
                }
                out.println(r.getDecompiledFunction().getC());
            }
        }
    }

    private String show(Varnode v) {
        if (v.isConstant()) return "#" + Long.toHexString(v.getOffset());
        String where = v.isRegister() ? String.valueOf(currentProgram.getRegister(v.getAddress(), v.getSize()))
            : v.getAddress().isStackAddress() ? "stack" + Long.toHexString(v.getOffset()) : v.getAddress().toString();
        HighVariable h = v.getHigh();
        return where + ":" + v.getSize() + (h != null ? "{" + h.getName() + (v.isInput() ? ",in" : "") + "}" : "");
    }
}
