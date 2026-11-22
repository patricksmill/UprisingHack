"""Works out, from the C 3dsrecomp generated, which argument registers each
function reads before writing them and which result registers its callers
read after calling it: the contract ghidra/LiftOverrides.java gives Ghidra
for a callee in place of the prototype it would guess.

usage: python3 scripts/contracts.py [generated C dir] [out]

Writes a line a function: <address> in=<r0..r3,s0..s15 read on the way in>
ret=<r0,r1,s0,s1 a caller reads on the way back>. A register is left out
when the code writes it first or the answer is not reached (a switch, an
indirect call, too long a walk): the lifter adds what Ghidra sees the
callee read. A function that returns straight after a call returns what
its callee does, so a result its callers read is its callee's too. A
result counts when two callers read it, or one in five, or one of three or
fewer; a register written under a condition and read later is not reached.
"""
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
START = re.compile(r"^void f_([0-9A-F]{8})\(Context \*ctx\) \{$", re.M)
CALL = re.compile(r"CALL\(f_([0-9A-F]{8})\)")
GOTO = re.compile(r"goto (L_[0-9A-F]{8});")
SKIPPED = ("#", "/*", "RETURNED", "BLOCK_TO", "L_")
STOPS = ("switch", "OUT_OF_BUDGET", "INTERPRET", "SVC(", "CALL(", "JUMP_TO", "VFP_VECTOR")
IN = [f"reg{n}_" for n in range(4)] + [f"vfp{n}_" for n in range(16)]
OUT = ["reg0_", "reg1_", "vfp0_", "vfp1_"]
SHOWN = {**{f"reg{n}_": f"r{n}" for n in range(4)}, **{f"vfp{n}_": f"s{n}" for n in range(16)}}

functions: dict[str, list[str]] = {}   # address -> its lines
labels: dict[str, dict[str, int]] = {}  # address -> label -> line index
memo: dict[tuple[str, str], str] = {}


def load(generated: Path) -> None:
    for path in sorted(generated.glob("code*.c")):
        text = path.read_text(encoding="latin1")
        starts = list(START.finditer(text))
        for i, m in enumerate(starts):
            end = text.find("\n}\n", m.end())
            lines = [line.strip() for line in text[m.end():end].split("\n")]
            functions[m.group(1)] = lines
            labels[m.group(1)] = {line[:-1]: n for n, line in enumerate(lines) if line.startswith("L_") and line.endswith(":")}


def walk(address: str, at: int, name: str, budget: list[int], seen: frozenset) -> str:
    """read, dead (written first), pass (the function returns without
    touching it) or unknown, for a register from a line of a function on"""
    lines = functions[address]
    write = re.compile(r"^(uint32_t\s+)?" + name + r"\s*=(.*)$")
    depth = 0
    maybe_written = False  # under a condition: a later read may be of that
    for line in lines[at:]:
        if not line or line.startswith(SKIPPED):
            continue
        if line.startswith("if (") and line.endswith("{"):
            if name in line:
                return "unknown" if maybe_written else "read"
            depth += 1
            continue
        if line == "}":
            depth = max(0, depth - 1)
            continue
        if name in line:
            w = write.match(line)
            if w and name not in w.group(2):
                if depth == 0:
                    return "dead"
                maybe_written = True
                continue
            return "unknown" if maybe_written else "read"
        g = GOTO.match(line)
        if g:
            label = g.group(1)
            if budget[0] <= 0 or label not in labels[address]:
                return "unknown"
            budget[0] -= 1
            there = "pass" if label in seen else walk(address, labels[address][label] + 1, name, budget, seen | {label})
            if depth == 0 or there in ("read", "unknown"):
                return there
            continue
        c = CALL.search(line)
        if c:
            if budget[0] <= 0:
                return "unknown"
            budget[0] -= 1
            inner = entry_use(c.group(1), name, budget)
            if inner == "pass":
                continue
            if depth == 0 or inner in ("read", "unknown"):
                return inner
            continue
        if line.startswith("RETURN"):
            if depth == 0:
                return "pass"
            continue
        if line.startswith(STOPS):
            return "unknown"
    return "unknown"


def entry_use(address: str, name: str, budget: list[int] | None = None) -> str:
    key = (address, name)
    if key in memo:
        return memo[key]
    if address not in functions:
        return "unknown"
    memo[key] = "unknown"  # a function reached again while it is being walked
    start = labels[address].get("L_" + address)
    result = "unknown" if start is None else walk(address, start + 1, name, budget or [80], frozenset())
    memo[key] = result
    return result


def main() -> None:
    generated = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "build/lib"
    out = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "build/contracts.txt"
    load(generated)
    returns: dict[str, set[str]] = defaultdict(set)
    reads: dict[tuple[str, str], int] = defaultdict(int)
    sites: dict[str, int] = defaultdict(int)
    passes: dict[tuple[str, str], set[str]] = defaultdict(set)  # (caller, register) -> callees it hands the result on from
    for caller, lines in functions.items():
        for n, line in enumerate(lines):
            c = CALL.search(line)
            if not c:
                continue
            sites[c.group(1)] += 1
            for name in OUT:
                after = walk(caller, n + 1, name, [40], frozenset())
                if after == "read":
                    reads[(c.group(1), name)] += 1
                elif after == "pass":
                    passes[(caller, name)].add(c.group(1))
    # one caller in many that seems to read a result is a stale register
    # reaching some later code, not a result
    for (callee, name), count in reads.items():
        if count >= 2 or sites[callee] <= 3 or count * 5 >= sites[callee]:
            returns[callee].add(name)
    changed = True
    while changed:
        changed = False
        for (caller, name), callees in passes.items():
            if name in returns[caller]:
                for callee in callees:
                    if name not in returns[callee]:
                        returns[callee].add(name)
                        changed = True
    with open(out, "w") as f:
        for address in sorted(functions):
            ins = [SHOWN[name] for name in IN if entry_use(address, name) == "read"]
            rets = [SHOWN[name] for name in OUT if name in returns[address]]
            f.write(f"{address} in={','.join(ins)} ret={','.join(rets)}\n")
    print(f"{len(functions)} functions, {sum(1 for a in functions if returns[a])} with a result a caller reads")


if __name__ == "__main__":
    main()
