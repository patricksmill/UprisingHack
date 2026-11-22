"""Lifts guest functions to C Overrides and checks them against the generated
code: Ghidra (ghidra/LiftOverrides.java) writes the C, gcc builds it into a
library of its own, and scripts/difftest.c runs every function against the
reference library.

usage: python3 scripts/lift.py <address list> [out dir] [runs] [result contracts]

Writes <out>/lifted.c, report.txt (what was skipped and why), lifted.so and
check.txt (a verdict for each lifted function), and prints the totals.
"""
import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "build/lib/0004000000030100.so"
PARTS = 48


def ghidra(addresses: Path, out: Path, contracts: Path | None = None) -> None:
    jdk = next((ROOT / "tools").glob("jdk-25*"))
    env = dict(os.environ, JAVA_HOME=str(jdk), PATH=f"{jdk}/bin:{os.environ['PATH']}")
    # Headless Ghidra can exit zero after a script exception; never reuse old outputs.
    with tempfile.TemporaryDirectory(prefix=".ghidra-", dir=out) as temporary:
        staged = Path(temporary)
        result = subprocess.run(
            [str(ROOT / "tools/ghidra_12.2_PUBLIC/support/analyzeHeadless"), str(ROOT / "ghidra/project"), "kiu",
             "-process", "code.bin", "-noanalysis", "-readOnly", "-scriptPath", str(ROOT / "ghidra"),
             "-postScript", "LiftOverrides.java", str(addresses), str(staged / "lifted.c"), str(staged / "report.txt"),
             *([str(contracts)] if contracts is not None else [])],
            env=env, capture_output=True, text=True)
        (out / "ghidra.log").write_text(result.stdout + result.stderr)
        result.check_returncode()
        wanted = [line.strip() for line in addresses.read_text().splitlines() if line.strip()]
        report = staged / "report.txt"
        if (not (staged / "lifted.c").is_file() or not report.is_file()
                or [line.split()[0] for line in report.read_text().splitlines() if line.strip()] != wanted):
            raise RuntimeError(f"Ghidra did not complete the requested lift; see {out / 'ghidra.log'}")
        for name in ["lifted.c", "report.txt"]:
            (staged / name).replace(out / name)


def build(out: Path) -> None:
    shutil.copy(ROOT / "ghidra/lifted.h", out)
    shutil.copy(ROOT / "tools/3dsrecomp/abi/recomp.h", out)
    source = (out / "lifted.c").read_text()
    functions = re.split(r"(?m)^(?=/\* \S+ \*/\nRECOMP_OVERRIDE)", source)[1:]

    def compile_part(i: int) -> str:
        part = out / f"part{i:03}.c"
        part.write_text('#include "lifted.h"\n\n' + "".join(functions[i::PARTS]))
        result = subprocess.run(["gcc", "-O0", "-fPIC", "-c", "-w", "-o", str(part.with_suffix(".o")), str(part)],
                                capture_output=True, text=True)
        return result.stderr if result.returncode else ""

    with ThreadPoolExecutor(os.cpu_count()) as pool:
        errors = [e for e in pool.map(compile_part, range(PARTS)) if e]
    if errors:
        sys.exit("lifted C does not compile:\n" + errors[0][:2000])
    subprocess.run(["gcc", "-shared", "-o", str(out / "lifted.so"), *map(str, sorted(out.glob("part*.o"))), "-lm"], check=True)


def check(out: Path, runs: int) -> Counter:
    difftest = ROOT / "build/difftest"
    subprocess.run(["cc", "-O1", "-I", str(ROOT / "tools/3dsrecomp/abi"), "-o", str(difftest), str(ROOT / "scripts/difftest.c"),
                    "-Wl,--no-as-needed", "-lm", "-ldl"], check=True)
    lifted = [line.split()[0] for line in (out / "report.txt").read_text().splitlines() if line.split()[1] == "lifted"]
    (out / "lifted.txt").write_text("\n".join(lifted) + "\n")
    with open(out / "check.txt", "w") as log:
        subprocess.run([str(difftest), str(REFERENCE), str(out / "lifted.so"), f"@{out / 'lifted.txt'}", "4", str(runs)],
                       cwd=ROOT, stdout=log)
    return Counter(line.split()[1].rstrip(",") for line in (out / "check.txt").read_text().splitlines() if line.startswith("0x"))


def main() -> None:
    addresses = Path(sys.argv[1]).resolve()
    out = Path(sys.argv[2] if len(sys.argv) > 2 else ROOT / "build/lift").resolve()
    runs = int(sys.argv[3]) if len(sys.argv) > 3 else 200
    out.mkdir(parents=True, exist_ok=True)
    contracts = Path(sys.argv[4]).resolve() if len(sys.argv) > 4 else None
    ghidra(addresses, out, contracts)
    build(out)
    verdicts = check(out, runs)
    skipped = Counter(re.sub(r" (0x|at |input register ).*| \d+ ways.*", "", line.split(" ", 2)[2])
                      for line in (out / "report.txt").read_text().splitlines() if line.split()[1] == "skip")
    print("verdicts:", dict(verdicts))
    print("skipped:", sum(skipped.values()), dict(skipped.most_common(8)))


if __name__ == "__main__":
    main()
