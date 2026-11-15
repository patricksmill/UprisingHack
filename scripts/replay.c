/* Replays functions from the machine states kiutrace's capture wrote, as
   the game entered them, through two recompiled libraries, and reports
   where they part: real inputs where scripts/difftest.c has random ones.

   build: cc -O1 -I tools/3dsrecomp/abi -o build/replay scripts/replay.c -Wl,--no-as-needed -lm -ldl
   usage: replay <reference.so> <candidate.so> <capture dir>...

   The candidate is a library 3dsrecomp built with overrides, or a library
   of lifted overrides alone (override_0x... functions); calls the replayed
   function makes run the reference's code in both.

   Every captured page is mapped, read-only until written, when the world
   copies it. A run that reaches memory outside the capture, takes over a
   second, or traps is counted apart.

   What has to match: every byte written, but for the stack below the
   entry's sp (the frames of the function and its calls, dead once it
   returns); r4..r11, sp, where it returned to, why it left; s16..s31 and
   fpscr. r0 is compared apart, as a function returning nothing leaves
   whatever it had there. r1..r3, r12 and the flags are the caller's to lose.

   REPLAY_ONLY=<address> in the environment replays that function alone and
   prints, for each capture that differs, the registers and up to 32 of the
   bytes that do. REPLAY_SHOW=1 prints that for the first capture of every
   function that differs, and for a candidate that did not finish, the
   address it reached for outside the capture, the signal that stopped it,
   and the last code it looked up with why that code left.

   REPLAY_CALLS=<dir> writes, for the first capture of every function that
   differs, the calls each world made in order, as <address>.ref and
   <address>.cand: the target, r0..r3, s0..s3 and sp of each. It needs a
   reference built to report its calls (a recomp.h whose CALL calls
   recomp_trace_call) and this program linked with -rdynamic; the
   candidate's own calls are seen as it looks their code up.

   Exit status is nonzero for differences (including r0-only), functions
   with no comparable capture, or an empty selection.

   REPLAY_VFP_RESULTS=<0..16> additionally compares s0 through s(N-1) as
   result registers for every selected function. Use it only for known
   float-return contracts, usually with REPLAY_ONLY. */
#include <dirent.h>
#include <dlfcn.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "recomp.h"

#define PAGE_COUNT (1u << 20)
#define DEAD_STACK 0x10000u /* below the entry's sp, compared not at all */
#define BUDGET_START (1 << 30)
/* fpscr's control bits; the condition flags, QC and the cumulative
   exception bits are not preserved across calls under the AAPCS */
#define FPSCR_KEPT (~(0xF8000000u | 0x9Fu))

typedef struct Page { char name[33]; uint8_t *bytes; } Page;

/* pages by name, loaded once and shared by every capture, in an open
   addressed table keyed by the name's first hash */
#define LOADED_SLOTS (1u << 20)
static Page *loaded;

typedef struct World {
    Context ctx;
    uint32_t vfp[64];
    uint32_t fpscr;
    uint8_t **read_pages, **write_pages;
    uint8_t **own;       /* pages this world wrote, by number */
    uint32_t *written;   /* their numbers */
    uint32_t written_count;
} World;

static const uint8_t **base;   /* the capture's pages, by number */
static uint32_t *mapped;       /* their numbers */
static uint32_t mapped_count;
static sigjmp_buf escape;
static int escaped_by;
static const Entry *ref_entries;
static uint32_t ref_count;

static uint32_t stray_address, last_lookup;
static uint32_t vfp_results;

typedef struct Call { uint32_t target, r[4], s[4], sp; char direct; } Call;
#define CALLS_KEPT 4000u
static Call calls[2][CALLS_KEPT];
static uint32_t call_count[2];
static World worlds[2];

/* direct is 1 for a call the replayed candidate made itself */
static void note_call(Context *ctx, uint32_t target, char direct) {
    int w = ctx->user == &worlds[1];
    if (call_count[w] >= CALLS_KEPT) return;
    Call *call = &calls[w][call_count[w]++];
    call->target = target;
    call->direct = direct;
    call->sp = ctx->r[13];
    memcpy(call->r, ctx->r, sizeof call->r);
    memcpy(call->s, ctx->vfp, sizeof call->s);
}

/* called by a reference built to report its calls, before each one */
void recomp_trace_call(Context *ctx, const char *name) {
    note_call(ctx, name[0] == 'f' && name[1] == '_' ? (uint32_t)strtoul(name + 2, 0, 16) : ctx->r[15], 0);
}
static int stop_signal;
static void bail(int why) { escaped_by = why; siglongjmp(escape, 1); }

static uint8_t *byte_at(Context *ctx, uint32_t address, int writing) {
    World *world = ctx->user;
    uint32_t page = address >> 12;
    if (!base[page]) { stray_address = address; bail(1); }
    if (writing && !world->own[page]) {
        world->own[page] = malloc(0x1000);
        memcpy(world->own[page], base[page], 0x1000);
        world->read_pages[page] = world->write_pages[page] = world->own[page];
        world->written[world->written_count++] = page;
    }
    const uint8_t *bytes = world->own[page] ? world->own[page] : base[page];
    return (uint8_t *)bytes + (address & 0xFFF);
}
static uint8_t read8(Context *c, uint32_t a) { return *byte_at(c, a, 0); }
static uint16_t read16(Context *c, uint32_t a) { return read8(c, a) | read8(c, a + 1) << 8; }
static uint32_t read32(Context *c, uint32_t a) { return read16(c, a) | (uint32_t)read16(c, a + 2) << 16; }
static void write8(Context *c, uint32_t a, uint8_t v) { *byte_at(c, a, 1) = v; }
static void write16(Context *c, uint32_t a, uint16_t v) { write8(c, a, v); write8(c, a + 1, v >> 8); }
static void write32(Context *c, uint32_t a, uint32_t v) { write16(c, a, v); write16(c, a + 2, v >> 16); }
static void interpret(Context *c, uint32_t a, uint32_t o) { (void)c; (void)a; (void)o; bail(2); }
static Code lookup(Context *c, uint32_t a) {
    last_lookup = a;
    /* a lifted function's call: it leaves this where the callee returns to */
    if (c && c->r[14] == 4 && c->r[15] == (a & ~1u)) note_call(c, a & ~1u, 1);
    uint32_t lo = 0, hi = ref_count;
    while (lo < hi) {
        uint32_t mid = (lo + hi) / 2;
        if (ref_entries[mid].address < a) lo = mid + 1; else hi = mid;
    }
    return lo < ref_count && ref_entries[lo].address == a ? ref_entries[lo].code : 0;
}
static const Host host = {read8, read16, read32, write8, write16, write32, interpret, lookup};
static void trapped(int signal) { stop_signal = signal; bail(3); }

static void *open_library(const char *path) {
    void *library = dlopen(path, RTLD_NOW);
    if (!library) { fprintf(stderr, "%s\n", dlerror()); exit(2); }
    return library;
}

static int by_address(const void *a, const void *b) {
    uint32_t x = ((const Entry *)a)->address, y = ((const Entry *)b)->address;
    return (x > y) - (x < y);
}

static const Entry *sorted_table(void *library, uint32_t *count) {
    const uint32_t *n = dlsym(library, "recomp_entry_count");
    const Entry *e = dlsym(library, "recomp_entries");
    if (!n || !e) return 0;
    Entry *sorted = malloc(*n * sizeof *sorted);
    memcpy(sorted, e, *n * sizeof *sorted);
    qsort(sorted, *n, sizeof *sorted, by_address);
    *count = *n;
    return sorted;
}

static const uint8_t *page_named(const char *dir, const char *name) {
    uint32_t slot;
    char key[17];
    snprintf(key, sizeof key, "%.16s", name);
    slot = (uint32_t)(strtoull(key, 0, 16) % LOADED_SLOTS);
    for (; loaded[slot].bytes; slot = (slot + 1) % LOADED_SLOTS)
        if (!strcmp(loaded[slot].name, name)) return loaded[slot].bytes;
    char path[4096];
    snprintf(path, sizeof path, "%s/pages/%s", dir, name);
    FILE *f = fopen(path, "rb");
    if (!f) { fprintf(stderr, "no page %s\n", path); exit(2); }
    uint8_t *bytes = malloc(0x1000);
    if (fread(bytes, 1, 0x1000, f) != 0x1000) { fprintf(stderr, "short page %s\n", path); exit(2); }
    fclose(f);
    snprintf(loaded[slot].name, sizeof loaded[slot].name, "%.32s", name);
    loaded[slot].bytes = bytes;
    return bytes;
}

typedef struct State {
    uint32_t entry, r[16], vfp[32], fpscr, tls;
    uint8_t n, z, c, v, q, ge, thumb;
} State;

/* reads a capture, mapping its pages; 0 when it is not one */
static int read_capture(const char *dir, const char *file, State *s) {
    char path[4096];
    snprintf(path, sizeof path, "%s/%s", dir, file);
    FILE *f = fopen(path, "r");
    if (!f) return 0;
    for (uint32_t i = 0; i < mapped_count; i++) base[mapped[i]] = 0;
    mapped_count = 0;
    char word[64];
    memset(s, 0, sizeof *s);
    while (fscanf(f, "%63s", word) == 1) {
        if (!strcmp(word, "entry")) fscanf(f, "%x", &s->entry);
        else if (!strcmp(word, "frame")) fscanf(f, "%*u");
        else if (!strcmp(word, "r")) for (int i = 0; i < 16; i++) fscanf(f, "%x", &s->r[i]);
        else if (!strcmp(word, "flags")) {
            unsigned n, z, c, v, q, ge, thumb;
            fscanf(f, "%u %u %u %u %u %u %u", &n, &z, &c, &v, &q, &ge, &thumb);
            s->n = n; s->z = z; s->c = c; s->v = v; s->q = q; s->ge = ge; s->thumb = thumb;
        } else if (!strcmp(word, "vfp")) for (int i = 0; i < 32; i++) fscanf(f, "%x", &s->vfp[i]);
        else if (!strcmp(word, "fpscr")) fscanf(f, "%x", &s->fpscr);
        else if (!strcmp(word, "tls")) fscanf(f, "%x", &s->tls);
        else if (!strcmp(word, "page")) {
            uint32_t address;
            char name[64];
            fscanf(f, "%x %63s", &address, name);
            base[address >> 12] = page_named(dir, name);
            mapped[mapped_count++] = address >> 12;
        }
    }
    fclose(f);
    return s->entry != 0;
}

static void reset(World *w) {
    for (uint32_t i = 0; i < w->written_count; i++) {
        uint32_t page = w->written[i];
        free(w->own[page]);
        w->own[page] = 0;
        w->read_pages[page] = w->write_pages[page] = 0;
    }
    w->written_count = 0;
    /* captured pages read directly; writes go through write8 and copy */
    for (uint32_t i = 0; i < mapped_count; i++) {
        w->read_pages[mapped[i]] = (uint8_t *)base[mapped[i]];
        w->write_pages[mapped[i]] = 0;
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

static int run(World *w, Code code, const State *s) {
    memset(&w->ctx, 0, sizeof w->ctx);
    memcpy(w->ctx.r, s->r, sizeof s->r);
    w->ctx.n = s->n; w->ctx.z = s->z; w->ctx.c = s->c; w->ctx.v = s->v; w->ctx.q = s->q; w->ctx.ge = s->ge;
    w->ctx.thumb = s->thumb;
    w->ctx.tls = s->tls;
    w->ctx.budget = BUDGET_START;
    memset(w->vfp, 0, sizeof w->vfp);
    memcpy(w->vfp, s->vfp, sizeof s->vfp);
    w->fpscr = s->fpscr;
    w->ctx.read_pages = w->read_pages;
    w->ctx.write_pages = w->write_pages;
    w->ctx.vfp = w->vfp;
    w->ctx.fpscr = &w->fpscr;
    w->ctx.host = &host;
    w->ctx.user = w;
    escaped_by = 0;
    if (sigsetjmp(escape, 1)) { alarm(0); return escaped_by; }
    alarm(1);
    run_to_return(&w->ctx, code);
    alarm(0);
    return 0;
}

/* the first address at page where the worlds differ, but in the dead stack,
   or 0 */
static uint32_t page_differs(uint32_t page, uint32_t sp) {
    const uint8_t *x = worlds[0].own[page] ? worlds[0].own[page] : base[page];
    const uint8_t *y = worlds[1].own[page] ? worlds[1].own[page] : base[page];
    for (uint32_t i = 0; i < 0x1000; i++) {
        uint32_t address = (page << 12) + i;
        if (address < sp && sp - address <= DEAD_STACK) continue;
        if (x[i] != y[i]) return address;
    }
    return 0;
}

/* compares the two worlds after a run: 0 same, 1 r0 only, 2 differ */
static int compare(const State *s, char *why, size_t size) {
    Context *a = &worlds[0].ctx, *b = &worlds[1].ctx;
    uint32_t sp = s->r[13];
    for (int w = 0; w < 2; w++)
        for (uint32_t i = 0; i < worlds[w].written_count; i++) {
            uint32_t page = worlds[w].written[i], at = page_differs(page, sp);
            if (at) {
                const uint8_t *x = worlds[0].own[page] ? worlds[0].own[page] : base[page];
                const uint8_t *y = worlds[1].own[page] ? worlds[1].own[page] : base[page];
                snprintf(why, size, "memory at %08X %02X/%02X, sp %08X", at, x[at & 0xFFF], y[at & 0xFFF], sp);
                return 2;
            }
        }
    if (memcmp(&a->r[4], &b->r[4], 8 * 4) || a->r[13] != b->r[13]) { snprintf(why, size, "r4-r11/sp"); return 2; }
    if (a->r[15] != b->r[15] || a->thumb != b->thumb || a->exit != b->exit) {
        snprintf(why, size, "return %08X/%08X exit %u/%u", a->r[15], b->r[15], a->exit, b->exit);
        return 2;
    }
    if (memcmp(worlds[0].vfp + 16, worlds[1].vfp + 16, 16 * 4) || ((worlds[0].fpscr ^ worlds[1].fpscr) & FPSCR_KEPT)) { snprintf(why, size, "vfp"); return 2; }
    for (uint32_t i = 0; i < vfp_results; i++)
        if (a->vfp[i] != b->vfp[i]) { snprintf(why, size, "s%u %08X/%08X", i, a->vfp[i], b->vfp[i]); return 2; }
    return a->r[0] != b->r[0];
}

/* every byte the worlds differ in above the dead stack, up to 32 */
static void show_differences(const State *s, const char *file) {
    Context *a = &worlds[0].ctx, *b = &worlds[1].ctx;
    printf("  %s:", file);
    for (int i = 0; i < 16; i++) if (a->r[i] != b->r[i]) printf(" r%d %08X/%08X", i, a->r[i], b->r[i]);
    for (uint32_t i = 0; i < vfp_results; i++) if (a->vfp[i] != b->vfp[i]) printf(" s%u %08X/%08X", i, a->vfp[i], b->vfp[i]);
    printf("\n");
    uint32_t shown = 0;
    for (int w = 0; w < 2; w++)
        for (uint32_t i = 0; i < worlds[w].written_count; i++) {
            uint32_t page = worlds[w].written[i];
            if (w == 1 && worlds[0].own[page]) continue; /* seen from the first world */
            const uint8_t *x = worlds[0].own[page] ? worlds[0].own[page] : base[page];
            const uint8_t *y = worlds[1].own[page] ? worlds[1].own[page] : base[page];
            for (uint32_t at = 0; at < 0x1000 && shown < 32; at++) {
                uint32_t address = (page << 12) + at;
                if (address < s->r[13] && s->r[13] - address <= DEAD_STACK) continue;
                if (x[at] != y[at]) { printf("    %08X %02X/%02X\n", address, x[at], y[at]); shown++; }
            }
        }
}

typedef struct Tally { uint32_t entry, runs, same, r0_only, differ, escaped; char why[96]; } Tally;

static int by_name(const void *a, const void *b) { return strcmp(*(char *const *)a, *(char *const *)b); }

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: replay <reference.so> <candidate.so> <capture dir>...\n");
        return 2;
    }
    const char *result_text = getenv("REPLAY_VFP_RESULTS");
    if (result_text) {
        char *end;
        unsigned long count = strtoul(result_text, &end, 10);
        if (*result_text < '0' || *result_text > '9' || *end || count > 16) {
            fprintf(stderr, "REPLAY_VFP_RESULTS must be an integer from 0 to 16\n");
            return 2;
        }
        vfp_results = (uint32_t)count;
    }
    void *ref = open_library(argv[1]), *cand = open_library(argv[2]);
    ref_entries = sorted_table(ref, &ref_count);
    if (!ref_entries) { fprintf(stderr, "%s has no entry table\n", argv[1]); return 2; }
    uint32_t cand_count = 0;
    const Entry *cand_entries = sorted_table(cand, &cand_count);
    base = calloc(PAGE_COUNT, sizeof *base);
    loaded = calloc(LOADED_SLOTS, sizeof *loaded);
    mapped = malloc(PAGE_COUNT * sizeof *mapped);
    for (int w = 0; w < 2; w++) {
        worlds[w].read_pages = calloc(PAGE_COUNT, sizeof(uint8_t *));
        worlds[w].write_pages = calloc(PAGE_COUNT, sizeof(uint8_t *));
        worlds[w].own = calloc(PAGE_COUNT, sizeof(uint8_t *));
        worlds[w].written = malloc(PAGE_COUNT * sizeof(uint32_t));
    }
    signal(SIGILL, trapped);
    signal(SIGSEGV, trapped);
    signal(SIGFPE, trapped);
    signal(SIGTRAP, trapped);
    signal(SIGALRM, trapped);

    const char *only_text = getenv("REPLAY_ONLY");
    uint32_t only = only_text ? strtoul(only_text, 0, 16) : 0;
    int show = getenv("REPLAY_SHOW") != 0;
    const char *calls_dir = getenv("REPLAY_CALLS");
    Tally *tallies = calloc(65536, sizeof *tallies);
    uint32_t tally_count = 0;
    for (int d = 3; d < argc; d++) {
        DIR *dir = opendir(argv[d]);
        if (!dir) { fprintf(stderr, "no %s\n", argv[d]); return 2; }
        char **files = 0;
        uint32_t file_count = 0;
        for (struct dirent *e; (e = readdir(dir));) {
            size_t length = strlen(e->d_name);
            if (length > 4 && !strcmp(e->d_name + length - 4, ".txt")) {
                files = realloc(files, (file_count + 1) * sizeof *files);
                files[file_count++] = strdup(e->d_name);
            }
        }
        closedir(dir);
        qsort(files, file_count, sizeof *files, by_name);
        for (uint32_t i = 0; i < file_count; i++) {
            State s;
            if (!read_capture(argv[d], files[i], &s)) continue;
            if (only && s.entry != only) continue;
            Tally *t = 0;
            for (uint32_t j = 0; j < tally_count; j++) if (tallies[j].entry == s.entry) t = &tallies[j];
            if (!t) { t = &tallies[tally_count++]; t->entry = s.entry; }
            t->runs++;
            Code reference = lookup(0, s.entry), candidate = 0;
            if (cand_entries) {
                for (uint32_t j = 0; j < cand_count; j++) if (cand_entries[j].address == s.entry) candidate = cand_entries[j].code;
            } else {
                char name[32];
                snprintf(name, sizeof name, "override_0x%08X", s.entry);
                candidate = dlsym(cand, name);
            }
            if (!reference || !candidate) { t->escaped++; continue; }
            reset(&worlds[0]);
            reset(&worlds[1]);
            call_count[0] = call_count[1] = 0;
            if (run(&worlds[0], reference, &s) || worlds[0].ctx.exit) { t->escaped++; continue; }
            int eb = run(&worlds[1], candidate, &s);
            char why[96] = "";
            int verdict = eb ? 2 : compare(&s, why, sizeof why);
            if (eb) snprintf(why, sizeof why, "%s", eb == 3 ? "trapped" : "left the captured memory");
            if (verdict == 0) t->same++;
            else if (verdict == 1) t->r0_only++;
            else {
                if (!t->differ) snprintf(t->why, sizeof t->why, "%s (%s)", why, files[i]);
                t->differ++;
                if (calls_dir && t->differ == 1)
                    for (int w = 0; w < 2; w++) {
                        char path[4096];
                        snprintf(path, sizeof path, "%s/%08X.%s", calls_dir, s.entry, w ? "cand" : "ref");
                        FILE *f = fopen(path, "w");
                        if (!f) { fprintf(stderr, "no %s\n", path); return 2; }
                        for (uint32_t k = 0; k < call_count[w]; k++) {
                            const Call *call = &calls[w][k];
                            fprintf(f, "%08X %d %08X %08X %08X %08X %08X %08X %08X %08X %08X\n", call->target, call->direct, call->r[0],
                                    call->r[1], call->r[2], call->r[3], call->s[0], call->s[1], call->s[2], call->s[3], call->sp);
                        }
                        fclose(f);
                    }
                if ((only || (show && t->differ == 1)) && !eb) { printf("0x%08X", s.entry); show_differences(&s, files[i]); }
                if ((only || (show && t->differ == 1)) && eb)
                    printf("0x%08X  %s: %s, stray %08X, signal %d, last lookup %08X, exit %u\n", s.entry, files[i],
                           eb == 3 ? "trapped" : "left", eb == 1 ? stray_address : 0, eb == 3 ? stop_signal : 0, last_lookup,
                           worlds[1].ctx.exit);
            }
        }
    }
    int failed = tally_count == 0;
    for (uint32_t i = 0; i < tally_count; i++) {
        Tally *t = &tallies[i];
        const char *verdict = t->differ ? "DIFFERS" : t->r0_only ? "r0-only" : t->same ? "same" : "untested";
        printf("0x%08X: %s, %u captures, %u same, %u differ only in r0, %u differ, %u not comparable%s%s\n", t->entry, verdict,
               t->runs, t->same, t->r0_only, t->differ, t->escaped, t->differ ? "; first: " : "", t->differ ? t->why : "");
        failed |= t->differ != 0 || t->r0_only != 0 || t->same == 0;
    }
    return failed;
}
