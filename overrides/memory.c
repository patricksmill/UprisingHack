#include "overrides.h"

/* The game's word-aligned memory copy, recovered from the USA build. See
   docs/decomp/LEDGER.md for the checks. */

/* reads a piece whole, then writes it, as the original's ldm/stm pairs do;
   it matters when the two ranges overlap. */
static void copy_piece(Context *ctx, uint32_t to, uint32_t from, uint32_t size) {
    uint8_t piece[32];
    for (uint32_t i = 0; i < size; i++)
        piece[i] = mem_read8(ctx, from + i);
    for (uint32_t i = 0; i < size; i++)
        mem_write8(ctx, to + i, piece[i]);
}

/* copies size bytes forward from source to destination, 32 at a time, then
   16, 8, 4 and the last 3 bytes or fewer. Returns past the destination's
   end in r0 and past the source's in r1. (destination, source, size) */
RECOMP_OVERRIDE(0x00425F00) {
    uint32_t to = ctx->r[0], from = ctx->r[1], size = ctx->r[2];
    uint32_t chunks = size / 32;
    /* 6 on the way in, 7 a chunk, 16 for the tail */
    BUDGET_ATOMIC(0x00425F00, 6 + 7 * chunks + 16);
    for (uint32_t i = 0; i < chunks; i++, to += 32, from += 32)
        copy_piece(ctx, to, from, 32);
    for (uint32_t piece = 16; piece >= 4; piece /= 2)
        if (size & piece) {
            copy_piece(ctx, to, from, piece);
            to += piece;
            from += piece;
        }
    copy_piece(ctx, to, from, size & 3);
    ctx->r[0] = to + (size & 3);
    ctx->r[1] = from + (size & 3);
    RETURN_TO(ctx->r[14]);
}
