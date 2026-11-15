/* What functions lifted by ghidra/LiftOverrides.java run on: guest memory,
   the instruction budget, and calls through the guest calling convention
   (AAPCS with VFP arguments). Each lifted function runs to its end once
   entered, as BUDGET_ATOMIC describes, and so does every call it makes. */
#ifndef LIFTED_H
#define LIFTED_H

#include "recomp.h"

/* entering: yields only before doing anything; keeps where it was called
   from and the stack it was called with, and lowers sp past its frame so
   its calls leave the frame alone. the registers it was called with are
   kept too: its calls change them, the lifted code reads them as they were. */
#define LIFT_ENTER(address, frame) \
    BUDGET_ATOMIC(address, 0); \
    const uint32_t lift_entry_sp = ctx->r[13], lift_entry_lr = ctx->r[14]; \
    uint32_t lift_in_r[16], lift_in_vfp[32]; \
    memcpy(lift_in_r, ctx->r, sizeof lift_in_r); \
    memcpy(lift_in_vfp, ctx->vfp, sizeof lift_in_vfp); \
    (void)lift_in_r; (void)lift_in_vfp; \
    ctx->r[13] = lift_entry_sp - (frame)

/* floats are held as their bits; the arithmetic runs on the values and
   flushes a subnormal result to zero when fpscr asks, as generated code does */
static inline float lift_f(uint64_t bits) { return vfp_from_s((uint32_t)bits); }
static inline double lift_d(uint64_t bits) { return vfp_from_d((uint32_t)bits, (uint32_t)(bits >> 32)); }
static inline uint64_t lift_fbits(Context *ctx, float value) { return vfp_to_s_in(*ctx->fpscr, value); }
static inline uint64_t lift_dbits(Context *ctx, double value) { return vfp_to_d_in(*ctx->fpscr, value); }

#define LIFT_RETURN() do { ctx->r[13] = lift_entry_sp; RETURN_TO(lift_entry_lr); } while (0)

static inline void lift_charge(Context *ctx, int32_t count) { ctx->budget -= count; }

static inline uint64_t lift_read(Context *ctx, uint64_t address, int size) {
    uint32_t a = (uint32_t)address;
    switch (size) {
    case 1: return mem_read8(ctx, a);
    case 2: return mem_read16(ctx, a);
    case 4: return mem_read32(ctx, a);
    case 8: return mem_read32(ctx, a) | (uint64_t)mem_read32(ctx, a + 4) << 32;
    default: {
        uint64_t v = 0;
        for (int i = 0; i < size && i < 8; i++) v |= (uint64_t)mem_read8(ctx, a + i) << (8 * i);
        return v;
    }
    }
}

static inline void lift_write(Context *ctx, uint64_t address, int size, uint64_t value) {
    uint32_t a = (uint32_t)address;
    switch (size) {
    case 1: mem_write8(ctx, a, (uint8_t)value); break;
    case 2: mem_write16(ctx, a, (uint16_t)value); break;
    case 4: mem_write32(ctx, a, (uint32_t)value); break;
    case 8: mem_write32(ctx, a, (uint32_t)value); mem_write32(ctx, a + 4, (uint32_t)(value >> 32)); break;
    default:
        for (int i = 0; i < size && i < 8; i++) mem_write8(ctx, a + i, (uint8_t)(value >> (8 * i)));
    }
}

static inline uint64_t lift_mask(int size) { return size >= 8 ? ~0ULL : (1ULL << (8 * size)) - 1; }
static inline int64_t lift_sext(uint64_t v, int size) {
    if (size >= 8) return (int64_t)v;
    int shift = 64 - 8 * size;
    return (int64_t)(v << shift) >> shift;
}
/* p-code shifts past the width give zero (or the sign), unlike C */
static inline uint64_t lift_shl(uint64_t v, uint64_t n, int size) { return n >= (uint64_t)(8 * size) ? 0 : (v << n) & lift_mask(size); }
static inline uint64_t lift_shr(uint64_t v, uint64_t n, int size) { return n >= (uint64_t)(8 * size) ? 0 : (v & lift_mask(size)) >> n; }
static inline uint64_t lift_sar(int64_t v, uint64_t n, int size) {
    if (n >= (uint64_t)(8 * size)) n = 8 * size - 1;
    return (uint64_t)(v >> n) & lift_mask(size);
}
static inline uint64_t lift_udiv(uint64_t a, uint64_t b) { return b ? a / b : 0; }
static inline uint64_t lift_urem(uint64_t a, uint64_t b) { return b ? a % b : 0; }
static inline uint64_t lift_sdiv(int64_t a, int64_t b) { return b ? (uint64_t)(a / b) : 0; }
static inline uint64_t lift_srem(int64_t a, int64_t b) { return b ? (uint64_t)(a % b) : 0; }
static inline int lift_carry(uint64_t a, uint64_t b, int size) {
    uint64_t m = lift_mask(size);
    return ((a & m) + (b & m)) > m || (size >= 8 && a + b < a);
}
static inline int lift_scarry(uint64_t a, uint64_t b, int size) {
    int64_t x = lift_sext(a, size), y = lift_sext(b, size), r = lift_sext(a + b, size);
    return (x >= 0) == (y >= 0) && (r >= 0) != (x >= 0);
}
static inline int lift_sborrow(uint64_t a, uint64_t b, int size) {
    int64_t x = lift_sext(a, size), y = lift_sext(b, size), r = lift_sext(a - b, size);
    return (x >= 0) != (y >= 0) && (r >= 0) != (x >= 0);
}
static inline uint64_t lift_lzcount(uint64_t v, int size) {
    v &= lift_mask(size);
    return v ? (uint64_t)(__builtin_clzll(v) - (64 - 8 * size)) : (uint64_t)(8 * size);
}

/* arguments in the order the callee takes them */
typedef struct LiftArgs {
    uint32_t core[64];
    int cores;
    uint32_t vfp[16];
    uint16_t vfp_used;
    int floats_done;
} LiftArgs;

static inline void lift_arg(LiftArgs *a, uint64_t v, int size, int is_float) {
    if (is_float && !a->floats_done) {
        int words = size == 8 ? 2 : 1;
        for (int i = 0; i + words <= 16; i += words) {
            uint16_t bits = (uint16_t)(words == 2 ? 3u << i : 1u << i);
            if (!(a->vfp_used & bits)) {
                a->vfp[i] = (uint32_t)v;
                if (words == 2) a->vfp[i + 1] = (uint32_t)(v >> 32);
                a->vfp_used |= bits;
                return;
            }
        }
        a->floats_done = 1; /* the rest go on the stack */
    }
    if (size == 8) {
        a->cores = (a->cores + 1) & ~1;
        a->core[a->cores++] = (uint32_t)v;
        a->core[a->cores++] = (uint32_t)(v >> 32);
    } else {
        a->core[a->cores++] = (uint32_t)v;
    }
}

/* where a call returns to: no code is here, the callee only stores it */
#define LIFT_SENTINEL 0x00000004u

/* calls the guest function at address to its end, charging what it ran. */
static inline void lift_call(Context *ctx, const LiftArgs *a, uint32_t address) {
    uint32_t sp = ctx->r[13];
    int stacked = a->cores > 4 ? a->cores - 4 : 0;
    uint32_t args_sp = (sp - 4 * (uint32_t)stacked) & ~7u;
    for (int i = 0; i < stacked; i++) mem_write32(ctx, args_sp + 4 * i, a->core[4 + i]);
    for (int i = 0; i < 4 && i < a->cores; i++) ctx->r[i] = a->core[i];
    for (int i = 0; i < 16; i++) if (a->vfp_used & (1u << i)) ctx->vfp[i] = a->vfp[i];
    ctx->r[13] = args_sp;
    ctx->r[14] = LIFT_SENTINEL;
    ctx->r[15] = address & ~1u;
    ctx->thumb = address & 1;
    int32_t left = ctx->budget;
    uint32_t depth = ctx->depth;
    ctx->budget = 1 << 30;
    /* generated code hands back the next address when it jumps somewhere it
       does not hold, a tail call say, for the host to go on from */
    while (!(ctx->r[15] == LIFT_SENTINEL && !ctx->thumb)) {
        Code code = ctx->host->lookup(ctx, ctx->r[15] | ctx->thumb);
        if (!code) __builtin_trap(); /* a closed function never stops on the way */
        code(ctx);
        if (ctx->exit) __builtin_trap();
    }
    ctx->depth = depth;
    ctx->budget = left - ((1 << 30) - ctx->budget);
    ctx->r[13] = sp;
}

static inline uint64_t lift_result(Context *ctx, int size, int is_float) {
    if (is_float) return size == 8 ? (ctx->vfp[0] | (uint64_t)ctx->vfp[1] << 32) : ctx->vfp[0];
    return size == 8 ? (ctx->r[0] | (uint64_t)ctx->r[1] << 32) : ctx->r[0] & (uint32_t)lift_mask(size);
}

static inline void lift_return_value(Context *ctx, uint64_t v, int size, int is_float) {
    if (is_float) {
        ctx->vfp[0] = (uint32_t)v;
        if (size == 8) ctx->vfp[1] = (uint32_t)(v >> 32);
    } else {
        ctx->r[0] = (uint32_t)v;
        if (size == 8) ctx->r[1] = (uint32_t)(v >> 32);
    }
}

#endif
