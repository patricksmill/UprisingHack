"""Says, for each function a replay found differing only in r0, whether its
direct callers read r0 after the call, from the C 3dsrecomp generated for
them: a lifted function whose r0 no caller reads returns nothing anyone
sees, one whose r0 a caller reads has a return value Ghidra missed.

usage: python3 scripts/r0_live.py <replay output> [generated C dir]

Prints a line a function: read (a caller reads r0 before writing it),
dead (every direct caller writes r0 first), unknown (a call site's flow
leaves the block first, or there are no direct calls: the function is
reached through pointers, whose callers this does not see).
"""
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CALL = re.compile(r"CALL\(f_([0-9A-F]{8})\);")
WRITE = re.compile(r"^(uint32_t\s+)?reg0_\s*=(.*)$")
LEAVES = ("goto", "CALL(", "RETURN", "switch", "OUT_OF_BUDGET", "INTERPRET", "SVC(")


def after_call(code: str) -> str:
    """read, dead or unknown for r0, from the generated lines after a call."""
    depth = 0
    for line in code.split("\n")[1:]:
        line = line.strip()
        if not line or line.startswith(("#", "/*", "RETURNED", "L_", "BLOCK_TO")):
            continue
        # a conditional instruction: what it reads counts, what it writes does not
        if line.startswith("if (") and line.endswith("{"):
            if "reg0_" in line:
                return "read"
            depth += 1
            continue
        if line == "}":
            depth = max(0, depth - 1)
            continue
        if "reg0_" in line:
            write = WRITE.match(line)
            if write and "reg0_" not in write.group(2):
                if depth == 0:
                    return "dead"
                continue
            return "read"
        if line.startswith(LEAVES):
            return "unknown"
    return "unknown"


def main() -> None:
    report = Path(sys.argv[1]).read_text().splitlines()
    generated = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "build/lib"
    targets = {line[2:10] for line in report if " r0-only," in line}
    sites: dict[str, list[str]] = {target: [] for target in targets}
    for path in sorted(generated.glob("code*.c")):
        code = path.read_text()
        for call in CALL.finditer(code):
            if call.group(1) in targets:
                sites[call.group(1)].append(after_call(code[call.end():call.end() + 8000]))
    totals = Counter()
    for target in sorted(sites):
        found = sites[target]
        verdict = "read" if "read" in found else "dead" if found and all(v == "dead" for v in found) else "unknown"
        totals[verdict] += 1
        print(f"0x{target}: {verdict}, {len(found)} direct call sites")
    print("totals:", dict(totals))


if __name__ == "__main__":
    main()
