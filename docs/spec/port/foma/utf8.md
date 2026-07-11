# foma/utf8.c

> [spec:foma:def:utf8.decode-quoted-fn]
> void decode_quoted(char *s)

> [spec:foma:sem:utf8.decode-quoted-fn+1]
> In-place decoding of \uXXXX escapes in `s`. Let len=strlen(s). Two cursors i (read) and j (write) start at 0; loop while i < len:
> If s[i]=='\\' (0x5c) AND len-i > 5 AND s[i+1]=='u' (0x75) AND the four bytes at s+i+2 pass `[spec:foma:sem:utf8.ishexstr-fn]`: call `[spec:foma:sem:utf8.utf8code16tostr-fn]` on s+i+2 to get a UTF-8 byte string, copy its bytes to s[j..] advancing j per byte, then i += 6. Edge: the escape \\u0000 converts to an empty string (the leading NUL terminates the conversion buffer), so the escape is deleted from the output.
> Otherwise copy one whole UTF-8 character: skip = utf8skip(s+i)+1 bytes, copying s[i]→s[j] and advancing both per byte. If `[spec:foma:sem:utf8.utf8skip-fn]` returns -1 (invalid lead byte) skip would be 0 (the C looped forever); the port forces skip to at least 1, so the malformed byte is copied through (lossy) and decoding terminates.
> After the loop the string is truncated at j. It can only shrink or stay the same length (a \uXXXX escape is 6 bytes; its UTF-8 form ≤ 3), so no overflow.

> [spec:foma:def:utf8.dequote-string-fn]
> void dequote_string(char *s)

> [spec:foma:sem:utf8.dequote-string-fn]
> If both the first byte and the last byte of `s` are '"' (0x22): shift the interior bytes s[1..len-2] left by one, write '\0' after them (dropping both quotes), then run `[spec:foma:sem:utf8.decode-quoted-fn]` on the result. Otherwise leave `s` untouched (no decoding). In-place; no allocation. Edge: a one-byte string `"` satisfies both tests (same byte) and becomes the empty string.

> [spec:foma:def:utf8.escape-string-fn]
> char *escape_string(char *string, char chr)

> [spec:foma:sem:utf8.escape-string-fn]
> Backslash-escapes every occurrence of byte `chr` in `string`. First count occurrences j; if j==0, return the original `string` pointer (no allocation). Otherwise calloc a buffer of strlen(string)+j bytes and copy byte-by-byte, writing '\\' before each occurrence of chr; return the new buffer, original untouched. Ownership is therefore conditional: caller gets a fresh buffer only when at least one escape happened.
> Latent bug: the buffer is exactly strlen+j bytes — all of it is filled with data, leaving no room for a NUL terminator; the result is only "terminated" because calloc happened to zero the block, i.e. it relies on out-of-buffer semantics being avoided by exact fill and is missing the +1 for '\0'.

> [spec:foma:def:utf8.hexstrtoint-fn]
> int hexstrtoint(char *str)

> [spec:foma:sem:utf8.hexstrtoint-fn]
> file-static. Converts exactly two hex digit bytes at str[0..1] to an int 0–255: for each byte, if > 0x60 ('`') treat as lowercase a–f (subtract 0x57), else if > 0x40 ('@') treat as uppercase A–F (subtract 0x37), else treat as digit (subtract 0x30); result = high<<4 | low. No validation whatsoever — callers must pre-check with `[spec:foma:sem:utf8.ishexstr-fn]`; garbage in gives garbage out.

> [spec:foma:def:utf8.int2utf8str-fn]
> unsigned char *int2utf8str(int codepoint)

> [spec:foma:sem:utf8.int2utf8str-fn]
> Encodes a codepoint as a fresh NUL-terminated UTF-8 byte string. Always mallocs 5 bytes first. codepoint < 0x80: one byte (the codepoint) + NUL. < 0x800: two bytes 0xC0|(cp>>6), 0x80|(cp&0x3F) + NUL. < 0x10000: three bytes 0xE0|(cp>>12), 0x80|((cp>>6)&0x3F), 0x80|(cp&0x3F) + NUL. Otherwise returns NULL — no 4-byte encoding support, and the already-malloc'd 5-byte buffer is leaked in that case (latent leak). Caller owns the returned buffer. Negative codepoints fall into the <0x80 branch and produce a garbage byte.

> [spec:foma:def:utf8.ishexstr-fn]
> int ishexstr (char *str)

> [spec:foma:sem:utf8.ishexstr-fn]
> Returns 1 iff the four bytes str[0..3] are each an ASCII hex digit, tested by ranges 0x30–0x39 ('0'-'9'), 0x41–0x46 ('A'-'F'), 0x61–0x66 ('a'-'f'); returns 0 at the first byte outside all three ranges (so it never reads past an early NUL — the NUL itself fails). Bytes are compared as (signed) char, so bytes ≥ 0x80 are negative and correctly fail. Reads up to 4 bytes; caller must guarantee they exist.

> [spec:foma:def:utf8.streqrep-fn]
> char *streqrep(char *s, char *oldstring, char *newstring)

> [spec:foma:sem:utf8.streqrep-fn+1]
> Replaces every non-overlapping occurrence of `oldstring` in `s` with `newstring` (same length), in place. Returns `s`. A single left-to-right scan advances past each replacement, so it always terminates and is O(|s|). An empty `oldstring` and a `newstring` shorter than `oldstring` are treated as no-ops. The C source looped strstr from the start of `s` after every replacement, so old==new (or a replacement that still matched oldstring, or an empty oldstring) never terminated, and a shorter newstring made the memcpy read past its end.

> [spec:foma:def:utf8.utf8code16tostr-fn]
> unsigned char *utf8code16tostr(char *str)

> [spec:foma:sem:utf8.utf8code16tostr-fn]
> Parses four hex-digit bytes at str[0..3] as a 16-bit codepoint — (hexstrtoint(str) << 8) + hexstrtoint(str+2), see `[spec:foma:sem:utf8.hexstrtoint-fn]` — and returns `[spec:foma:sem:utf8.int2utf8str-fn]`(codepoint): a freshly malloc'd NUL-terminated UTF-8 string owned by the caller. No input validation; only BMP (\uXXXX, no surrogate handling — a surrogate half is encoded as-is into 3 bytes, i.e. CESU-8-like output).

> [spec:foma:def:utf8.utf8iscombining-fn]
> int utf8iscombining(unsigned char *s)

> [spec:foma:sem:utf8.utf8iscombining-fn]
> Tests whether the UTF-8 character starting at `s` is a Unicode combining mark (per the ranges below); returns the byte length of its encoding (2 or 3), or 0 if not combining. Pure byte-level checks:
> Return 0 if s[0] or s[1] is NUL, or if s[0] is not one of 0xCC, 0xCD, 0xE1, 0xE2, 0xEF (fast reject).
> Two-byte (U+0300–036F Combining Diacritical Marks): s[0]==0xCC with s[1] in 0x80–0xBF, or s[0]==0xCD with s[1] in 0x80–0xAF → return 2.
> Then return 0 if s[2] is NUL. Three-byte: 0xE1 0xAA with s[2] in 0xB0–0xBE (U+1AB0–1ABE Extended); 0xE1 0xB7 with s[2] in 0x80–0xBF (U+1DC0–1DFF Supplement); 0xE2 0x83 with s[2] in 0x90–0xB0 (U+20D0–20F0 For Symbols); 0xEF 0xB8 with s[2] in 0xA0–0xAD (U+FE20–FE2D Combining Half Marks) → return 3. Anything else → 0.

> [spec:foma:def:utf8.utf8skip-fn]
> int utf8skip(char *str)

> [spec:foma:sem:utf8.utf8skip-fn]
> Classifies the lead byte *str (cast to unsigned char) and returns how many CONTINUATION bytes follow it: < 0x80 (ASCII) → 0; (b & 0xE0)==0xC0 → 1; (b & 0xF0)==0xE0 → 2; (b & 0xF8)==0xF0 → 3; anything else (a continuation byte 0x80–0xBF, or invalid 0xF8–0xFF) → -1. Does not inspect the following bytes.

> [spec:foma:def:utf8.utf8strlen-fn]
> int utf8strlen(char *str)

> [spec:foma:sem:utf8.utf8strlen-fn+1]
> Not ported: this is `str::chars().count()`. Every call site held a `&str` (or a `String`) and passed its `.as_bytes()` only to hand them here, so the byte-scan reimplementation was pointless — callers now count characters directly with `str::chars().count()` (cast to i32 only where a value flows into an existing i32). The C behaviour was: with len=strlen(str), walk i from 0 while str[i] != '\0' and i < len, advancing by utf8skip(str+i)+1 and incrementing a count; an invalid lead byte (utf8skip == -1) advanced 0 and looped forever, worked around by forcing a step of at least 1.

> [spec:foma:def:utf8.xstrrev-fn]
> char *xstrrev(char *str)

> [spec:foma:sem:utf8.xstrrev-fn]
> Reverses the BYTES of `str` in place (not UTF-8 aware — multibyte characters are corrupted by design; callers reverse again or operate on byte level). NULL or empty input returned unchanged. Implementation detail: two pointers from both ends swap via triple XOR while end > start (strict inequality, so a pointer never XOR-swaps with itself); returns `str`.

