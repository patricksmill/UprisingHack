#include "overrides.h"

/* Scalar storage access, recovered from the USA build. Names describe the
   observed layout; the factors' game-level meaning is still unknown.
   Both routines return an observable r0 as well as the float bits in s0.
   Evidence: analysis/FINDINGS.md; checks: docs/decomp/LEDGER.md. */
enum {
    PACKED_MASK = 0x04,
    PACKED_SCALAR_BIT = 0x08,
    FACTOR_ANCHOR = 0x14,
    NODE_PAYLOAD = 0x00,
    NODE_NEXT = 0x04,
    PAYLOAD_FACTOR = 0x10,
};

/* Enabled low-nibble fields occupy consecutive words after the mask.
   The scalar is last when present; otherwise the result is the default. */
RECOMP_OVERRIDE(0x00B40F28) {
    BUDGET_ATOMIC(0x00B40F28, 9 + 4 * (5 + 11));
    uint32_t object = ctx->r[0];
    uint32_t mask = mem_read32(ctx, object + PACKED_MASK);
    uint32_t count = (uint32_t)__builtin_popcount(mask & 0x0Fu);
    if (mask & PACKED_SCALAR_BIT) {
        ctx->r[0] = object + PACKED_MASK + 4 * count;
        ctx->vfp[0] = mem_read32(ctx, ctx->r[0]);
    } else {
        BUDGET_LEFT -= 3;
        ctx->r[0] = count;
        ctx->vfp[0] = mem_read32(ctx, 0x00B40F98);
    }
    RETURN_TO(ctx->r[14]);
}

/* Wrapper nodes carry payload pointers. A payload equal to the anchor's
   first word terminates traversal, rather than a null next pointer. */
RECOMP_OVERRIDE(0x00B49860) {
    BUDGET_ATOMIC(0x00B49860, 4);
    uint32_t anchor = ctx->r[0] + FACTOR_ANCHOR;
    uint32_t product = mem_read32(ctx, 0x00B49890);
    uint32_t node = mem_read32(ctx, anchor + NODE_NEXT);
    for (;;) {
        BUDGET_LEFT -= 5;
        uint32_t end = mem_read32(ctx, anchor);
        uint32_t payload = mem_read32(ctx, node + NODE_PAYLOAD);
        if (payload == end)
            break;
        BUDGET_LEFT -= 3;
        uint32_t factor = mem_read32(ctx, payload + PAYLOAD_FACTOR);
        node = mem_read32(ctx, node + NODE_NEXT);
        /* Preserve the reference library's first-operand NaN payload;
           C multiplication alone can select either operand's payload. */
        if ((factor & 0x7FFFFFFFu) > 0x7F800000u)
            product = factor | 0x00400000u;
        else
            product = vfp_to_s(ctx, vfp_from_s(factor) * vfp_from_s(product));
    }
    ctx->r[0] = anchor;
    ctx->vfp[0] = product;
    RETURN_TO(ctx->r[14]);
}
