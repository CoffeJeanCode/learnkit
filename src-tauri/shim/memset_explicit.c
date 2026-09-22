/* Windows-GNU (MSVCRT) compatibility shim.
 *
 * libsodium (via iota_stronghold -> tauri-plugin-stronghold) calls
 * `memset_explicit` (C23), which MSVCRT does not export (it exists only in
 * UCRT). MSVC builds link UCRT and don't need this. This file is compiled
 * only for `windows-gnu` targets (see build.rs) and provides an equivalent
 * non-optimizable memory setter.
 */
#include <stddef.h>

void *memset_explicit(void *s, int c, size_t n) {
    volatile unsigned char *p = (volatile unsigned char *)s;
    while (n--) {
        *p++ = (unsigned char)c;
    }
    return s;
}
