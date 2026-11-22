"""Float result checks must catch errors hidden by the ordinary scratch ABI."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from recomp_abi import abi_dir  # noqa: E402


class FloatResults(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.work = Path(cls.temporary.name)
        abi = abi_dir()
        cls.binary = cls.work / "difftest"
        subprocess.run(["cc", "-O1", "-I", str(abi), "-o", str(cls.binary),
                        str(ROOT / "scripts/difftest.c"), "-lm", "-ldl"], check=True)
        (cls.work / "dumps/exefs").mkdir(parents=True)
        (cls.work / "dumps/exefs/code.bin").write_bytes(bytes(4))
        source = cls.work / "result.c"
        source.write_text('''#include "recomp.h"
static void result(Context *ctx) {
    ctx->vfp[0] = 0x3F800000;
    ctx->vfp[1] = 0x40000000 ^ MUTANT;
    ctx->r[15] = ctx->r[14];
}
RECOMP_EXPORT const uint32_t recomp_entry_count = 1;
RECOMP_EXPORT const Entry recomp_entries[] = {{0x00200000, result}};
''')
        for mutant in [0, 1]:
            subprocess.run(["cc", "-shared", "-fPIC", "-I", str(abi),
                            f"-DMUTANT={mutant}", str(source), "-o",
                            str(cls.work / f"result{mutant}.so")], check=True)

    def compare(self, count):
        env = dict(os.environ, DIFFTEST_VFP_RESULTS=count)
        return subprocess.run([str(self.binary), str(self.work / "result0.so"),
                               str(self.work / "result1.so"), "00200000", "0", "8"],
                              cwd=self.work, env=env, capture_output=True, text=True)

    def test_only_requested_result_registers_are_compared(self):
        for count in ["0", "1"]:
            with self.subTest(count=count):
                self.assertEqual(self.compare(count).returncode, 0)
        result = self.compare("2")
        self.assertEqual(result.returncode, 1)
        self.assertIn("8 differ", result.stdout)
        self.assertIn("vfp", result.stdout)

    def test_invalid_result_counts_are_rejected(self):
        for count in ["", "-1", "17", "1x"]:
            with self.subTest(count=count):
                result = self.compare(count)
                self.assertEqual(result.returncode, 2)
                self.assertIn("must be an integer from 0 to 16", result.stderr)


if __name__ == "__main__":
    unittest.main()
