/* Runs the code two recompiled libraries hold for guest functions on the
   same random registers and memory, and reports where they part: recovered
   or lifted C Overrides against the generated code they replace.

   build: cc -O1 -I "$(python3 scripts/recomp_abi.py)" -o build/difftest scripts/difftest.c -Wl,--no-as-needed -lm -ldl
   usage: difftest <reference.so> <candidate.so> <address|@list> <pointers> [runs] [via] [size limit]

   The candidate is either a library 3dsrecomp built with the overrides, or
   a library of lifted overrides alone (ghidra/LiftOverrides.java), whose
   override_0x... functions are run directly; their calls go to the
   reference's code. @list runs every address in a file, one per line.

   pointers is how many of r0..r3 point into the test memory, from r0 up.
   via is the reference's function to enter instead, for a function the
   reference library holds only as a label inside its caller. size limit,
   when given, keeps the register after the pointers below it.

   Memory is 32 KiB of random words at 0x10000000, a random stack, and the
   game's own code.bin from 0x00100000 with zeroed bss, written copy on
   write. A run that reaches memory outside them, takes longer than the
   budget, or traps is not compared, and counted apart.

   DIFFTEST_SHAPE in the environment shapes that memory for functions random
   words do not reach far into: 1 gives r1's memory the first bytes of
   r0's, up to 64, for functions that compare the two; 2 makes an eighth of
   the words pointers into the memory, for functions that follow one.

   DIFFTEST_VFP_RESULTS=0..16 additionally compares s0 through s(N-1)
   for functions with known float results. The default compares none.

   What has to match: memory, the live stack (at and above sp on return),
   r0, r4..r11, sp, where it returned to, why it left, and for overrides
   built with 3dsrecomp the instructions charged. r1..r3, r12 and the flags
   are the caller's to lose under the ARM calling convention, as is r0 of a
   function that returns nothing; a run that differs only in r0 is counted
   apart. */
#include <dlfcn.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "recomp.h"

#define ARENA 0x10000000u
#define PAGES 8u
#define SIZE (PAGES << 12)
#define STACK 0x0FF00000u
#define RETURN_ADDRESS 0x00100000u
#define BUDGET_START 1000000
/* fpscr's control bits; the condition flags, QC and the cumulative
   exception bits are not preserved across calls under the AAPCS */
#define FPSCR_KEPT (~(0xF8000000u | 0x9Fu))
#define IMAGE 0x00100000u
#define IMAGE_PAGES 0x1000u /* code.bin and bss, 16 MiB */
#define CODE_BIN "dumps/exefs/code.bin"

typedef struct World {
    Context ctx;
    uint8_t memory[SIZE];
    uint8_t stack[0x1000];
    uint32_t vfp[64];
    uint32_t fpscr;
    uint8_t *own[IMAGE_PAGES]; /* image pages this world wrote */
    uint8_t **read_pages, **write_pages;
} World;

static World worlds[2];
static uint8_t *image;
static sigjmp_buf escape;
static int escaped_by;

static const Entry *ref_entries;
static uint32_t ref_count;

static void bail(int why) { escaped_by = why; siglongjmp(escape, 1); }

/* an image page the world has not written: the first write copies it */
static uint8_t *byte_at(Context *ctx, uint32_t address, int writing) {
    World *world = ctx->user;
    if (address - ARENA < SIZE) return &world->memory[address - ARENA];
    if (address - IMAGE < IMAGE_PAGES << 12) {
        uint32_t page = (address - IMAGE) >> 12;
        if (writing && !world->own[page]) {
            world->own[page] = malloc(0x1000);
            memcpy(world->own[page], image + (page << 12), 0x1000);
            world->read_pages[(IMAGE >> 12) + page] = world->write_pages[(IMAGE >> 12) + page] = world->own[page];
        }
        uint8_t *base = world->own[page] ? world->own[page] : image + (page << 12);
        return base + (address & 0xFFF);
    }
    bail(1);
    return 0;
}
static uint8_t read8(Context *c, uint32_t a) { return *byte_at(c, a, 0); }
static uint16_t read16(Context *c, uint32_t a) { return read8(c, a) | read8(c, a + 1) << 8; }
static uint32_t read32(Context *c, uint32_t a) { return read16(c, a) | (uint32_t)read16(c, a + 2) << 16; }
static void write8(Context *c, uint32_t a, uint8_t v) { *byte_at(c, a, 1) = v; }
static void write16(Context *c, uint32_t a, uint16_t v) { write8(c, a, v); write8(c, a + 1, v >> 8); }
static void write32(Context *c, uint32_t a, uint32_t v) { write16(c, a, v); write16(c, a + 2, v >> 16); }
static void interpret(Context *c, uint32_t a, uint32_t o) { (void)c; (void)a; (void)o; bail(2); }
static Code lookup(Context *c, uint32_t a) {
    (void)c;
    uint32_t lo = 0, hi = ref_count;
    while (lo < hi) {
        uint32_t mid = (lo + hi) / 2;
        if (ref_entries[mid].address < a) lo = mid + 1; else hi = mid;
    }
    return lo < ref_count && ref_entries[lo].address == a ? ref_entries[lo].code : 0;
}
static const Host host = {read8, read16, read32, write8, write16, write32, interpret, lookup};
static void trapped(int signal) { (void)signal; bail(3); }

static uint64_t seed = 0x9E3779B97F4A7C15ull;
static uint32_t rnd(void) {
    seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17;
    return (uint32_t)(seed >> 16);
}

static void *open_library(const char *path) {
    void *library = dlopen(path, RTLD_NOW);
    if (!library) { fprintf(stderr, "%s\n", dlerror()); exit(2); }
    return library;
}

static int by_address(const void *a, const void *b) {
    uint32_t x = ((const Entry *)a)->address, y = ((const Entry *)b)->address;
    return (x > y) - (x < y);
}

static void table(void *library, const Entry **entries, uint32_t *count) {
    const uint32_t *n = dlsym(library, "recomp_entry_count");
    const Entry *e = dlsym(library, "recomp_entries");
    if (!n || !e) { *entries = 0; *count = 0; return; }
    Entry *sorted = malloc(*n * sizeof *sorted);
    memcpy(sorted, e, *n * sizeof *sorted);
    qsort(sorted, *n, sizeof *sorted, by_address);
    *entries = sorted;
    *count = *n;
}

static Code find(const Entry *entries, uint32_t count, uint32_t address) {
    for (uint32_t i = 0; i < count; i++)
        if (entries[i].address == address) return entries[i].code;
    return 0;
}

static void reset(World *world) {
    for (uint32_t i = 0; i < IMAGE_PAGES; i++) {
        free(world->own[i]);
        world->own[i] = 0;
        world->read_pages[(IMAGE >> 12) + i] = image + (i << 12);
        world->write_pages[(IMAGE >> 12) + i] = 0;
    }
}

/* runs code until it returns to its caller. Generated code also hands back
   to the host when it jumps somewhere its function does not hold, an
   indirect call's target or the return from one; the host goes on from
   there with the reference's code, as Zakuro does. */
static void run_to_return(Context *ctx, Code code) {
    uint32_t lr = ctx->r[14], sp = ctx->r[13];
    uint32_t back = lr & (lr & 1 ? ~1u : ~3u);
    for (;;) {
        code(ctx);
        if (ctx->exit) return;
        if (ctx->r[15] == back && ctx->thumb == (lr & 1) && ctx->r[13] >= sp) return;
        code = lookup(ctx, ctx->r[15] | ctx->thumb);
        if (!code) bail(2);
    }
}

/* runs code in a world; 0 when it stayed inside the test memory */
static int run(World *world, Code code) {
    for (uint32_t i = 0; i < PAGES; i++)
        world->read_pages[(ARENA >> 12) + i] = world->write_pages[(ARENA >> 12) + i] = world->memory + (i << 12);
    world->read_pages[STACK >> 12] = world->write_pages[STACK >> 12] = world->stack;
    world->ctx.read_pages = world->read_pages;
    world->ctx.write_pages = world->write_pages;
    world->ctx.vfp = world->vfp;
    world->ctx.fpscr = &world->fpscr;
    world->ctx.host = &host;
    world->ctx.user = world;
    escaped_by = 0;
    if (sigsetjmp(escape, 1)) { alarm(0); return escaped_by; }
    alarm(1);
    run_to_return(&world->ctx, code);
    alarm(0);
    return 0;
}

static int same_image(World *a, World *b) {
    for (uint32_t i = 0; i < IMAGE_PAGES; i++) {
        if (!a->own[i] && !b->own[i]) continue;
        const uint8_t *x = a->own[i] ? a->own[i] : image + (i << 12);
        const uint8_t *y = b->own[i] ? b->own[i] : image + (i << 12);
        if (memcmp(x, y, 0x1000)) return 0;
    }
    return 1;
}

typedef struct Tally { uint32_t runs, compared, differ, aliased_differ, r0_only, escaped, scratch; char why[96]; } Tally;
static uint32_t vfp_results;

static Tally check(Code reference, Code candidate, uint32_t address, uint32_t pointers, uint32_t runs, uint32_t limit,
                   int exact_budget, int verbose) {
    Tally t = {0};
    const char *shaped = getenv("DIFFTEST_SHAPE");
    int shape = shaped ? atoi(shaped) : 0;
    World *a = &worlds[0], *b = &worlds[1];
    for (uint32_t n = 0; n < runs; n++) {
        t.runs++;
        reset(a);
        reset(b);
        int aliased = 0;
        memset(&a->ctx, 0, sizeof a->ctx);
        for (uint32_t i = 0; i < SIZE; i += 4) {
            /* a third of the words small, so tests on bytes and flags go both ways */
            uint32_t word = rnd() % 3 ? rnd() : rnd() & 0x101;
            if (shape & 2 && rnd() % 8 == 0) word = ARENA + rnd() % (SIZE / 2);
            memcpy(a->memory + i, &word, 4);
        }
        for (uint32_t i = 0; i < sizeof a->stack; i += 4) {
            uint32_t word = rnd() % 3 ? rnd() & 0xFF : ARENA + (rnd() % SIZE & ~3u);
            memcpy(a->stack + i, &word, 4);
        }
        for (uint32_t i = 0; i < 13; i++) a->ctx.r[i] = rnd();
        /* pointers stay in the first half, so a structure fits after them;
           some alias or overlap the one before */
        for (uint32_t i = 0; i < pointers && i < 4; i++) {
            uint32_t pick = rnd() % 10;
            if (i > 0 && pick == 0) { a->ctx.r[i] = a->ctx.r[i - 1]; aliased = 1; }
            else if (i > 0 && pick == 1) { a->ctx.r[i] = a->ctx.r[i - 1] + 4 * (rnd() % 0x100); aliased = 1; }
            else a->ctx.r[i] = ARENA + (rnd() % (SIZE / 2) & (pick < 8 ? ~3u : ~0u));
        }
        if (limit && pointers < 4) a->ctx.r[pointers] = rnd() % limit;
        if (shape & 1 && pointers >= 2)
            memmove(a->memory + (a->ctx.r[1] - ARENA), a->memory + (a->ctx.r[0] - ARENA), rnd() % 65);
        a->ctx.r[13] = STACK + 0x800;
        a->ctx.r[14] = RETURN_ADDRESS;
        a->ctx.r[15] = address & ~1u;
        a->ctx.thumb = address & 1;
        a->ctx.n = rnd() & 1; a->ctx.z = rnd() & 1; a->ctx.c = rnd() & 1; a->ctx.v = rnd() & 1;
        a->ctx.budget = BUDGET_START;
        for (uint32_t i = 0; i < 64; i++) a->vfp[i] = rnd();
        a->fpscr = 0x03000000; /* flush to zero and default NaN, as the game runs */
        memcpy(&b->ctx, &a->ctx, sizeof a->ctx);
        memcpy(b->memory, a->memory, SIZE);
        memcpy(b->stack, a->stack, sizeof a->stack);
        memcpy(b->vfp, a->vfp, sizeof a->vfp);
        b->fpscr = a->fpscr;

        int ea = run(a, reference);
        if (ea || a->ctx.exit) { t.escaped++; continue; }
        int eb = run(b, candidate);
        if (eb) {
            /* the reference finished in the test memory; the candidate did not */
            t.compared++;
            if (aliased) { t.aliased_differ++; continue; }
            if (!t.differ) snprintf(t.why, sizeof t.why, "%s", eb == 3 ? "trapped" : "left the test memory");
            if (t.differ++ < 3 && verbose) printf("  run %u: candidate %s\n", n, eb == 3 ? "trapped" : "left the test memory");
            continue;
        }
        t.compared++;
        uint32_t live = a->ctx.r[13] - STACK < sizeof a->stack ? a->ctx.r[13] - STACK : sizeof a->stack;
        int mem = !memcmp(a->memory, b->memory, SIZE), stack = !memcmp(a->stack + live, b->stack + live, sizeof a->stack - live);
        /* s16..s31 are the callee's to keep; s0..s15 are scratch, as r1..r3 */
        /* fpscr's flags and cumulative exception bits are the caller's to lose */
        int img = same_image(a, b), vfp = !memcmp(a->vfp + 16, b->vfp + 16, 16 * 4) && ((a->fpscr ^ b->fpscr) & FPSCR_KEPT) == 0;
        vfp = vfp && !memcmp(a->vfp, b->vfp, vfp_results * sizeof(uint32_t));
        int regs = !memcmp(&a->ctx.r[4], &b->ctx.r[4], 8 * 4) && a->ctx.r[13] == b->ctx.r[13];
        int flow = a->ctx.r[15] == b->ctx.r[15] && a->ctx.thumb == b->ctx.thumb && a->ctx.exit == b->ctx.exit;
        int rest = mem && stack && img && vfp && regs && flow && (!exact_budget || a->ctx.budget == b->ctx.budget);
        if (rest && a->ctx.r[0] != b->ctx.r[0]) { t.r0_only++; continue; }
        if (!rest) {
            if (aliased) { t.aliased_differ++; continue; }
            if (!t.differ)
                snprintf(t.why, sizeof t.why, "%s%s%s%s%s%s", mem ? "" : " memory", stack ? "" : " stack", img ? "" : " image",
                         vfp ? "" : " vfp", regs ? "" : " r4-r11/sp", flow ? "" : " return");
            if (t.differ++ < 3 && verbose) {
                printf("  run %u differs:%s%s%s%s%s%s r0 %08X/%08X r15 %08X/%08X charged %d/%d\n", n, mem ? "" : " memory",
                       stack ? "" : " stack", img ? "" : " image", vfp ? "" : " vfp", regs ? "" : " r4-r11/sp", flow ? "" : " return",
                       a->ctx.r[0], b->ctx.r[0], a->ctx.r[15], b->ctx.r[15], BUDGET_START - a->ctx.budget, BUDGET_START - b->ctx.budget);
                for (uint32_t i = 0, shown = 0; i < SIZE && shown < 6; i++)
                    if (a->memory[i] != b->memory[i]) { printf("    0x%08X %02X/%02X\n", ARENA + i, a->memory[i], b->memory[i]); shown++; }
            }
            continue;
        }
        t.scratch += memcmp(&a->ctx.r[1], &b->ctx.r[1], 3 * 4) || a->ctx.r[12] != b->ctx.r[12];
    }
    return t;
}

int main(int argc, char **argv) {
    if (argc < 5) {
        fprintf(stderr, "usage: difftest <reference.so> <candidate.so> <address|@list> <pointers> [runs] [via] [size limit]\n");
        return 2;
    }
    const char *result_text = getenv("DIFFTEST_VFP_RESULTS");
    if (result_text) {
        char *end;
        unsigned long count = strtoul(result_text, &end, 10);
        if (*result_text < '0' || *result_text > '9' || *end || count > 16) {
            fprintf(stderr, "DIFFTEST_VFP_RESULTS must be an integer from 0 to 16\n");
            return 2;
        }
        vfp_results = (uint32_t)count;
    }
    uint32_t pointers = strtoul(argv[4], 0, 10);
    uint32_t runs = argc > 5 ? strtoul(argv[5], 0, 10) : 100000;
    uint32_t via = argc > 6 && argv[6][0] ? strtoul(argv[6], 0, 16) : 0;
    uint32_t limit = argc > 7 ? strtoul(argv[7], 0, 0) : 0;

    void *ref = open_library(argv[1]), *cand = open_library(argv[2]);
    table(ref, &ref_entries, &ref_count);
    if (!ref_entries) { fprintf(stderr, "%s has no entry table\n", argv[1]); return 2; }
    const Entry *cand_entries;
    uint32_t cand_count;
    table(cand, &cand_entries, &cand_count);
    int lifted = cand_entries == 0;

    FILE *code = fopen(CODE_BIN, "rb");
    if (!code) { fprintf(stderr, "run from the project: no %s\n", CODE_BIN); return 2; }
    image = calloc(IMAGE_PAGES, 0x1000);
    fread(image, 1, IMAGE_PAGES << 12, code);
    fclose(code);
    for (int w = 0; w < 2; w++) {
        worlds[w].read_pages = calloc(1 << 20, sizeof(uint8_t *));
        worlds[w].write_pages = calloc(1 << 20, sizeof(uint8_t *));
    }
    signal(SIGILL, trapped);
    signal(SIGSEGV, trapped);
    signal(SIGFPE, trapped);
    signal(SIGTRAP, trapped);
    signal(SIGALRM, trapped);

    uint32_t list[65536], n = 0;
    if (argv[3][0] == '@') {
        FILE *f = fopen(argv[3] + 1, "r");
        if (!f) { fprintf(stderr, "no %s\n", argv[3] + 1); return 2; }
        char line[64];
        while (n < 65536 && fgets(line, sizeof line, f)) if (line[0] > ' ') list[n++] = strtoul(line, 0, 16);
        fclose(f);
    } else {
        list[n++] = strtoul(argv[3], 0, 16);
    }

    int failed = 0;
    for (uint32_t i = 0; i < n; i++) {
        uint32_t address = list[i];
        Code reference = lookup(0, via ? via : address);
        Code candidate;
        if (lifted) {
            char name[32];
            snprintf(name, sizeof name, "override_0x%08X", address);
            candidate = dlsym(cand, name);
        } else {
            candidate = find(cand_entries, cand_count, address);
        }
        if (!reference || !candidate) {
            printf("0x%08X: missing in the %s\n", address, reference ? "candidate" : "reference");
            continue;
        }
        Tally t = check(reference, candidate, address, pointers, runs, limit, !lifted, n == 1);
        const char *verdict = t.compared == 0 ? "untested" : t.differ ? "DIFFERS" : t.aliased_differ ? "alias-only" : t.r0_only ? "r0-only" : "same";
        printf("0x%08X: %s, %u runs, %u compared, %u differ, %u differ with aliased pointers, %u differ only in r0, %u left the test memory%s%s\n",
               address, verdict, t.runs, t.compared, t.differ, t.aliased_differ, t.r0_only, t.escaped, t.differ ? "; first:" : "",
               t.differ ? t.why : "");
        fflush(stdout);
        failed |= t.compared == 0 || t.differ != 0 || t.aliased_differ != 0 || t.r0_only != 0;
    }
    return failed;
}
