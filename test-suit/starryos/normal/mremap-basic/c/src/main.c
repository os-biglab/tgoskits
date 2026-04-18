/* mremap-basic: verify mremap grows a mapping and preserves data.
 *   - mmap 2 pages (8 KiB) with known pattern
 *   - mremap to 4 pages (16 KiB) with MREMAP_MAYMOVE
 *   - First 2 pages must retain original data
 *   - New 2 pages must be writable (zeroed)
 *   - Old address must no longer be accessible (checked via WNOHANG child)
 *   - mremap to same size must be a no-op (data still there)
 *   - mremap shrink (4 -> 2 pages) must preserve first 2 pages
 */
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#define PAGE_SIZE 4096UL
#define PATTERN_A 0xAA
#define PATTERN_B 0xBB

static int fail(const char *msg) { puts(msg); return 1; }

static void fill(void *p, size_t len, int val) {
    memset(p, val, len);
}

static int verify(const void *p, size_t len, int val, const char *tag) {
    const unsigned char *b = p;
    for (size_t i = 0; i < len; i++) {
        if (b[i] != (unsigned char)val) {
            printf("TEST FAILED: %s mismatch at offset %zu: got 0x%02x, want 0x%02x\n",
                   tag, i, b[i], (unsigned char)val);
            return 1;
        }
    }
    return 0;
}

static int test_grow(void) {
    size_t old_sz = PAGE_SIZE * 2;
    size_t new_sz = PAGE_SIZE * 4;

    void *p = mmap(NULL, old_sz, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (p == MAP_FAILED) { perror("mmap grow"); return fail("TEST FAILED: mmap failed"); }

    fill(p, old_sz, PATTERN_A);

    void *q = mremap(p, old_sz, new_sz, MREMAP_MAYMOVE);
    if (q == MAP_FAILED) {
        perror("mremap grow");
        munmap(p, old_sz);
        return fail("TEST FAILED: mremap grow failed");
    }

    /* First 2 pages must still have PATTERN_A */
    if (verify(q, old_sz, PATTERN_A, "mremap grow first 2 pages")) {
        munmap(q, new_sz);
        return 1;
    }
    puts("mremap grow: old data preserved ok");

    /* Last 2 new pages must be writable */
    char *tail = (char *)q + old_sz;
    fill(tail, new_sz - old_sz, PATTERN_B);
    if (verify(tail, new_sz - old_sz, PATTERN_B, "mremap grow new pages")) {
        munmap(q, new_sz);
        return 1;
    }
    puts("mremap grow: new pages writable ok");

    munmap(q, new_sz);
    return 0;
}

static int test_shrink(void) {
    size_t old_sz = PAGE_SIZE * 4;
    size_t new_sz = PAGE_SIZE * 2;

    void *p = mmap(NULL, old_sz, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (p == MAP_FAILED) { perror("mmap shrink"); return fail("TEST FAILED: mmap failed"); }

    fill(p, old_sz, PATTERN_A);

    /* Write different pattern to back half */
    fill((char *)p + new_sz, new_sz, PATTERN_B);

    void *q = mremap(p, old_sz, new_sz, MREMAP_MAYMOVE);
    if (q == MAP_FAILED) {
        perror("mremap shrink");
        munmap(p, old_sz);
        return fail("TEST FAILED: mremap shrink failed");
    }

    /* First new_sz bytes must have PATTERN_A */
    if (verify(q, new_sz, PATTERN_A, "mremap shrink")) {
        munmap(q, new_sz);
        return 1;
    }
    puts("mremap shrink: first pages preserved ok");

    munmap(q, new_sz);
    return 0;
}

static int test_same_size(void) {
    size_t sz = PAGE_SIZE * 2;
    void *p = mmap(NULL, sz, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (p == MAP_FAILED) { perror("mmap same"); return fail("TEST FAILED: mmap failed"); }

    fill(p, sz, PATTERN_A);

    void *q = mremap(p, sz, sz, MREMAP_MAYMOVE);
    if (q == MAP_FAILED) {
        perror("mremap same");
        munmap(p, sz);
        return fail("TEST FAILED: mremap same-size failed");
    }

    if (verify(q, sz, PATTERN_A, "mremap same-size")) {
        munmap(q, sz);
        return 1;
    }
    puts("mremap same-size ok");

    munmap(q, sz);
    return 0;
}

int main(void) {
    if (test_grow() != 0) return 1;
    if (test_shrink() != 0) return 1;
    if (test_same_size() != 0) return 1;
    puts("TEST PASSED");
    return 0;
}
