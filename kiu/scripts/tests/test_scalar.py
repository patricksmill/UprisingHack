"""Directed checks for the two recovered raw core/float result pairs."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from recomp_abi import abi_dir  # noqa: E402


class ScalarResults(unittest.TestCase):
    def test_packed_fields_and_nan_payload_order(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            (work / "overrides.h").write_text('#include "recomp.h"\n')
            source = work / "check.c"
            source.write_text('''#include <assert.h>
#include <stdlib.h>
#include "recomp.h"
void override_0x00B40F28(Context *);
void override_0x00B49860(Context *);
int main(void) {
    uint8_t data[4096] = {0}, packed_code[4096] = {0}, list_code[4096] = {0};
    uint8_t **pages = calloc(1 << 20, sizeof *pages);
    pages[0x10000] = data;
    pages[0xB40] = packed_code;
    pages[0xB49] = list_code;
    store32(packed_code + 0xF98, 0x3F800000);
    store32(list_code + 0x890, 0x3F800000);
    uint32_t vfp[64] = {0}, fpscr = 0x03000000;
    Context ctx = {.read_pages=pages, .vfp=vfp, .fpscr=&fpscr};
    ctx.r[14] = 0x100000;
    for (uint32_t mask = 0; mask < 16; mask++) {
        store32(data + 4, 0xFFFF0000 | mask);
        for (uint32_t i = 0; i < 4; i++) store32(data + 8 + 4*i, 0x7FC01000+i);
        ctx.r[0] = 0x10000000; ctx.budget = 1000;
        override_0x00B40F28(&ctx);
        uint32_t count = __builtin_popcount(mask);
        assert(ctx.r[0] == (mask & 8 ? 0x10000004+4*count : count));
        assert(vfp[0] == (mask & 8 ? 0x7FC00FFF+count : 0x3F800000));
        assert(ctx.budget == 1000 - (mask & 8 ? 73 : 76));
    }
    store32(data+0x14, 0x12345678); store32(data+0x18, 0x10000100);
    store32(data+0x100, 0x10000200); store32(data+0x104, 0x10000108);
    store32(data+0x108, 0x10000220); store32(data+0x10C, 0x10000110);
    store32(data+0x110, 0x12345678);
    store32(data+0x210, 0x7FC01234); store32(data+0x230, 0xFF812345);
    ctx.r[0] = 0x10000000; ctx.budget = 1000;
    override_0x00B49860(&ctx);
    assert(ctx.r[0] == 0x10000014 && vfp[0] == 0xFFC12345);
    assert(ctx.budget == 1000 - 25);
    free(pages);
}
''')
            for optimization in ["-O0", "-O2"]:
                subprocess.run(["cc", optimization, "-ffp-contract=off", "-I", str(work),
                                "-I", str(abi_dir()), str(source),
                                str(ROOT / "overrides/scalar.c"), "-o", str(work / "check"), "-lm"], check=True)
                subprocess.run([str(work / "check")], check=True)


if __name__ == "__main__":
    unittest.main()
