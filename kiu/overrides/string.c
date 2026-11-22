#include "overrides.h"

/* The game's C string routines, the string hash and the set-once flag,
   recovered from the USA build. See docs/decomp/LEDGER.md for the checks.

   The generated code charges a whole block of instructions as it enters
   one, so each function here adds up the blocks its original would have
   entered; the counts beside the charges are those blocks' sizes. */

/* mov pc, lr: back to the caller in ARM state whatever lr's low bits say,
   where bx lr (RETURN_TO) would switch to Thumb. */
#define RETURN_ARM() do { ctx->r[15] = ctx->r[14] & ~3u; return; } while (0)

/* a mark in the top bit of each zero character of a word, counting from
   the lowest; marks above the first zero are not to be trusted. */
static inline uint32_t zero_marks(uint32_t word) {
    return (word - 0x01010101u) & ~word & 0x80808080u;
}

/* the length of a string: by characters up to a word boundary, then by
   words, so it reads up to 3 bytes past the end. (string) */
RECOMP_OVERRIDE(0x00425DB4) {
    BUDGET_ATOMIC(0x00425DB4, 2);
    uint32_t string = ctx->r[0], at = string;
    for (; at & 3; at++) {
        BUDGET_LEFT -= 3 + 4; /* the boundary test, a character */
        if (!mem_read8(ctx, at)) {
            ctx->r[0] = at - string;
            RETURN_ARM();
        }
    }
    BUDGET_LEFT -= 3;
    uint32_t marks;
    do {
        BUDGET_LEFT -= 15;
        marks = zero_marks(mem_read32(ctx, at));
        at += 4;
    } while (!marks);
    ctx->r[0] = at - 4 + (uint32_t)__builtin_ctz(marks) / 8 - string;
    RETURN_ARM();
}

/* how two words of characters compare from the lowest character up, given
   the first word's zero marks: 0 when they match as far as the first
   word's first zero. Otherwise half the difference at the first character
   that differs, with the top bit set when the first word's is the lower:
   below zero for a first string that sorts first, above for the second. */
static uint32_t compare_words(uint32_t first, uint32_t second, uint32_t marks) {
    for (uint32_t character = 0; character < 4; character++) {
        uint32_t shift = 24 - 8 * character;
        uint32_t x = first << shift, y = second << shift;
        if (x != y)
            return (y > x ? 0x80000000u : 0) | (y - x) >> 1;
        if (marks & 0x80u << 8 * character)
            return 0;
    }
    return 0;
}

/* compares two strings: 0 when equal, below zero when the first sorts
   first, above when the second does. Word-aligned strings are compared a
   word at a time and return compare_words' value; others a character at a
   time and return the difference of the characters. (first, second) */
RECOMP_OVERRIDE(0x00425F74) {
    BUDGET_ATOMIC(0x00425F74, 5);
    uint32_t first = ctx->r[0], second = ctx->r[1];
    if ((first | second) & 3) {
        uint32_t x, y, characters = 0;
        do {
            x = mem_read8(ctx, first++);
            y = mem_read8(ctx, second++);
            characters++;
        } while (x != 0 && x == y);
        BUDGET_LEFT -= 10 * (int32_t)((characters + 1) / 2) + 2; /* two characters a block */
        ctx->r[0] = x - y;
        RETURN_ARM();
    }
    uint32_t x, y, marks, words = 0;
    do {
        x = mem_read32(ctx, first);
        y = mem_read32(ctx, second);
        first += 4;
        second += 4;
        words++;
        marks = zero_marks(x);
    } while (!marks && x == y);
    BUDGET_LEFT -= 14 * (int32_t)((words + 1) / 2) + 13 + 2; /* two words a block */
    ctx->r[0] = compare_words(x, y, marks);
    RETURN_ARM();
}

/* sets a flag the first time it is asked: 1 when the flag was clear, and
   it is set now; 0 when it was set already. (flag) */
RECOMP_OVERRIDE(0x003B50EC) {
    BUDGET_ATOMIC(0x003B50EC, 7);
    uint32_t flag = ctx->r[0];
    uint32_t first = mem_read32(ctx, flag) == 0;
    if (first)
        mem_write32(ctx, flag, 1);
    ctx->r[0] = first;
    RETURN_ARM();
}

/* the hash of a string: each character, as a signed one, combined into
   the hash so far. (pointer to the string's address) */
RECOMP_OVERRIDE(0x00448770) {
    BUDGET_ATOMIC(0x00448770, 7);
    uint32_t string = mem_read32(ctx, ctx->r[0]);
    uint32_t length = 0;
    while (mem_read8(ctx, string + length))
        length++;
    uint32_t hash = 0;
    for (uint32_t i = 0; i < length; i++)
        hash ^= (uint32_t)(int8_t)mem_read8(ctx, string + i) + 0x9E3779B9u + (hash << 6) + (hash >> 2);
    /* a block a character for the length; the characters before the last
       are hashed two a block, after one of their own when there are an
       odd number of them; then the last character and the return */
    int32_t charged = 4 * (int32_t)length + 12 + 7 + 2;
    if (length >= 2)
        charged += 3 + 12 * (int32_t)((length - 1) / 2);
    BUDGET_LEFT -= charged;
    ctx->r[0] = hash;
    RETURN_TO(ctx->r[14]);
}
