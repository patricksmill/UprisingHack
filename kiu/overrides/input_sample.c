#include "overrides.h"

/* Input samples, recovered from the USA build (code.bin SHA-256 2718aa20...).

   The input manager keeps a ring of 16 samples, 0x25C bytes each, and once
   a game frame folds the samples taken since the last frame into one frame
   sample (0x00A4245C). These five leaf functions are that folding. Evidence
   and the equivalence checks are in docs/decomp/LEDGER.md.

   The fields named here are the ones these functions touch. HELD2 and SUMS
   are named by position: what they carry is not established yet. */
enum {
    SAMPLE_HELD = 0x000,     /* buttons down */
    SAMPLE_PRESSED = 0x004,  /* down now, up in the frame before */
    SAMPLE_RELEASED = 0x008, /* up now, down in the frame before */
    SAMPLE_HELD2 = 0x018,    /* a second button mask, with its own edges */
    SAMPLE_PRESSED2 = 0x01C,
    SAMPLE_RELEASED2 = 0x020,
    SAMPLE_TOUCH = 0x204,    /* x in the low half, y in the high half */
    SAMPLE_TOUCHING = 0x208, /* a byte; the word is copied whole */
    SAMPLE_SUMS = 0x20C,     /* four words added up across samples */
    SAMPLE_SUM_COUNT = 4,
};

static inline void or_into(Context *ctx, uint32_t into, uint32_t from, uint32_t field) {
    mem_write32(ctx, into + field, mem_read32(ctx, into + field) | mem_read32(ctx, from + field));
}

/* a & ~b of one field, stored in another field of a. */
static inline void store_edge(Context *ctx, uint32_t a, uint32_t b, uint32_t field, uint32_t edge) {
    mem_write32(ctx, a + edge, mem_read32(ctx, a + field) & ~mem_read32(ctx, b + field));
}

/* adds one sample to the frame's: buttons held at any point count as held,
   and the latest touch wins. (frame, sample) */
RECOMP_OVERRIDE(0x00919E90) {
    BUDGET_ATOMIC(0x00919E90, 16);
    uint32_t frame = ctx->r[0], sample = ctx->r[1];
    or_into(ctx, frame, sample, SAMPLE_HELD);
    or_into(ctx, frame, sample, SAMPLE_HELD2);
    if (mem_read8(ctx, sample + SAMPLE_TOUCHING)) {
        uint32_t touching = mem_read32(ctx, sample + SAMPLE_TOUCHING);
        uint32_t touch = mem_read32(ctx, sample + SAMPLE_TOUCH);
        mem_write32(ctx, frame + SAMPLE_TOUCH, touch);
        mem_write32(ctx, frame + SAMPLE_TOUCHING, touching);
    }
    RETURN_TO(ctx->r[14]);
}

/* clears the frame's buttons and touch before the samples are added. (frame) */
RECOMP_OVERRIDE(0x00919ED0) {
    BUDGET_ATOMIC(0x00919ED0, 9);
    uint32_t frame = ctx->r[0];
    mem_write32(ctx, frame + SAMPLE_RELEASED, 0);
    mem_write32(ctx, frame + SAMPLE_PRESSED, 0);
    mem_write8(ctx, frame + SAMPLE_TOUCHING, 0);
    mem_write32(ctx, frame + SAMPLE_HELD, 0);
    mem_write32(ctx, frame + SAMPLE_RELEASED2, 0);
    mem_write32(ctx, frame + SAMPLE_PRESSED2, 0);
    mem_write32(ctx, frame + SAMPLE_HELD2, 0);
    RETURN_TO(ctx->r[14]);
}

/* works out what was pressed and released since the frame before.
   (frame, previous frame) */
RECOMP_OVERRIDE(0x00919EF4) {
    BUDGET_ATOMIC(0x00919EF4, 17);
    uint32_t frame = ctx->r[0], previous = ctx->r[1];
    store_edge(ctx, frame, previous, SAMPLE_HELD, SAMPLE_PRESSED);
    mem_write32(ctx, frame + SAMPLE_RELEASED,
                mem_read32(ctx, previous + SAMPLE_HELD) & ~mem_read32(ctx, frame + SAMPLE_HELD));
    store_edge(ctx, frame, previous, SAMPLE_HELD2, SAMPLE_PRESSED2);
    mem_write32(ctx, frame + SAMPLE_RELEASED2,
                mem_read32(ctx, previous + SAMPLE_HELD2) & ~mem_read32(ctx, frame + SAMPLE_HELD2));
    RETURN_TO(ctx->r[14]);
}

/* adds a sample's sums to the frame's. (frame, sample) */
RECOMP_OVERRIDE(0x00919F38) {
    BUDGET_ATOMIC(0x00919F38, 17);
    uint32_t frame = ctx->r[0], sample = ctx->r[1];
    for (uint32_t i = 0; i < SAMPLE_SUM_COUNT; i++) {
        uint32_t at = SAMPLE_SUMS + 4 * i;
        mem_write32(ctx, frame + at, mem_read32(ctx, frame + at) + mem_read32(ctx, sample + at));
    }
    RETURN_TO(ctx->r[14]);
}

/* clears the frame's sums. (frame) */
RECOMP_OVERRIDE(0x00919F7C) {
    BUDGET_ATOMIC(0x00919F7C, 6);
    uint32_t frame = ctx->r[0];
    for (uint32_t i = 0; i < SAMPLE_SUM_COUNT; i++)
        mem_write32(ctx, frame + SAMPLE_SUMS + 4 * i, 0);
    RETURN_TO(ctx->r[14]);
}
