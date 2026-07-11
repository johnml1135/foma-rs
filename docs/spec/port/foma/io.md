# foma/io.c

> [spec:foma:def:io.binaryline]
> struct binaryline {
>   int type;
>   int state;
>   int in;
>   int target;
>   int out;
>   int symbol;
>   char *name;
>   char *value;
> }

> [spec:foma:def:io.bom]
> typedef struct BOM

> [spec:foma:def:io.check-bom-fn]
> BOM *check_BOM(char *buffer)

> [spec:foma:sem:io.check-bom-fn+1]
> Compares the start of `buffer` against a static table of byte-order marks, in order:
> UTF-8 (EF BB BF, len 3), UTF-32LE (FF FE 00 00, len 4), UTF-32BE (00 00 FE FF, len 4),
> UTF16-LE (FF FE, len 2), UTF16-BE (FE FF, len 2). Each entry's `len` bytes are compared
> exactly against the buffer; returns a reference to the first matching table entry (whose
> `name` field is the encoding name above), or None if none match. A buffer shorter than an
> entry's `len` cannot match it.
> The C tested each entry with strncmp(code, buffer, len), which stops at a
> mutual NUL, so any buffer whose first byte was 0x00 false-matched the UTF-32BE entry
> (00 00 FE FF) and "FF FE 00 <any>" false-matched UTF-32LE (its 4th byte was never
> checked, and UTF-32LE precedes UTF16-LE in the table). Comparing the actual bytes exactly
> removes both false positives: a lone leading 0x00 no longer matches UTF-32BE, and
> "FF FE 00 <non-00>" now classifies as UTF16-LE.

> [spec:foma:def:io.escape-print-fn]
> void escape_print(FILE *stream, char* string)

> [spec:foma:sem:io.escape-print-fn]
> Writes `string` to `stream`, escaping double quotes for prolog output. If strchr finds
> no `"` in the string, prints the whole string with a single fprintf. Otherwise iterates
> byte-by-byte, emitting the two characters `\"` (backslash, quote) for each `"` and the
> byte unchanged otherwise. Backslashes and all other characters pass through unescaped,
> so a symbol already containing a backslash is emitted ambiguously; the prolog reader
> `[spec:foma:sem:io.fsm-read-prolog-fn]` performs no unescaping (asymmetry).

> [spec:foma:def:io.explode-line-fn]
> static INLINE int explode_line(char *buf, int *values)

> [spec:foma:sem:io.explode-line-fn]
> Parses a line of space-separated ASCII integers in place. Scans `buf`, splitting fields
> at each ' ' (each found space is overwritten with '\0'), converts each field with atoi
> into successive slots of `values`, and stops after converting the field terminated by
> the original '\0'. Returns the number of fields converted; always >= 1 (an empty line
> yields one field of value 0), and consecutive spaces yield empty fields converted to 0.
> No bound check is performed on `values`: the only caller passes int[5], so a states
> line with more than 5 fields overruns the array (latent bug — document, do not rely on).

> [spec:foma:def:io.file-to-mem-fn]
> char *file_to_mem(char *name)

> [spec:foma:sem:io.file-to-mem-fn+1]
> Reads the whole file `name` into a freshly allocated NUL-terminated buffer; used by the
> text and spaced-text readers. Steps: open `name`; on failure print "Error opening file
> '<name>'\n" to stdout and fail. Size = on-disk length; allocate size+1. If the read is
> short, print "Error reading file '<name>'\n" and fail. Then run
> `[spec:foma:sem:io.check-bom-fn+1]` on the buffer: if any BOM is detected, print
> "<encoding> BOM mark is detected in file '<name>'.\n" and fail — BOM-prefixed files are
> rejected outright, not skipped past. Otherwise store '\0' at buffer[size] and return the
> buffer.
> Returns `Result<Vec<u8>, FomaError>` instead of the C `char *`/NULL sentinel —
> `Err(FomaError::Io(..))` for an open/read failure, `Err(FomaError::Format(..))` for a
> rejected BOM, `Ok(bytes)` otherwise; the printed diagnostics are retained for CLI-output
> compatibility. With the exact-match BOM check (`[spec:foma:sem:io.check-bom-fn+1]`) an
> empty file no longer false-matches UTF-32BE and reads as the lone terminating '\0'.

> [spec:foma:def:io.foma-net-print-fn]
> int foma_net_print(struct fsm *net, gzFile outfile)

> [spec:foma:sem:io.foma-net-print-fn+1]
> Serializes `net` in the textual foma binary format to the already-open sink
> `outfile`, returning `Ok(())` on success and propagating the first write failure
> as its `io::Error` (the C returned a vestigial `1` no caller inspected; a write
> error was silently dropped). All output goes through the writer, so the result is
> gzip-compressed by the zlib layer. Several networks may be written back-to-back into
> one file.
> Header: the line "##foma-net 1.0##\n", then "##props##\n", then one props line of 13
> space-separated fields: arity arccount statecount linecount finalcount pathcount
> (printed with %lld, 64-bit) is_deterministic is_pruned is_minimized is_epsilon_free
> is_loop_free extras name — where extras = is_completed | (arcs_sorted_in << 2) |
> (arcs_sorted_out << 4), and name is net->name printed verbatim (a name containing
> spaces would corrupt the field count on read).
> Sigma section: the line "##sigma##\n", then for each sigma entry, stopping at list end
> or an entry with number -1: "<number> <symbol>\n".
> States section: the line "##states##\n", then one line per fsm_state array entry using
> the compressed 2/3/4/5-field scheme, tracking laststate (initialized -1): if state_no
> != laststate, emit "state_no in target final_state" (4 fields) when in == out, else
> "state_no in out target final_state" (5 fields); if state_no == laststate, emit
> "in target" (2 fields) when in == out, else "in out target" (3 fields). States without
> transitions are ordinary 4-field lines "state_no -1 -1 final_state". The loop stops at
> the state_no == -1 sentinel entry; then the sentinel line "-1 -1 -1 -1 -1\n" is written.
> If net->medlookup and its confusion_matrix are both non-NULL, write "##cmatrix##\n"
> followed by (sigma_max+1)^2 lines, one integer per line, in row-major order.
> Finally write "##end##\n".

> [spec:foma:def:io.foma-write-prolog-fn]
> int foma_write_prolog (struct fsm *net, char *filename)

> [spec:foma:sem:io.foma-write-prolog-fn+1]
> Writes `net` as prolog clauses to `filename`, or to stdout if filename is NULL; if
> fopen(filename, "w") fails it prints "Error writing to file '<f>'. Using stdout.\n" and
> falls back to stdout, and whenever filename != NULL it prints "Writing prolog to file
> '<f>'.\n" (even after that fallback). Always returns 1.
> Calls fsm_count(net) first; copies net->name into a 100-byte local identifier via
> strcpy (unchecked, but FSM_NAME_LEN is 40) and emits "network(<name>).\n".
> One pass over the state array records each state's final flag (into a malloc'd
> int[statecount] indexed by state_no) and marks every in/out symbol number used on some
> line (into a calloc'd int[sigma_max+1]).
> For each sigma symbol number i from 3 through sigma_max that is unused on any arc,
> emits `symbol(<name>, "<sym>").` with the symbol text printed through
> `[spec:foma:sem:io.escape-print-fn]`; a symbol whose text is literally "0" is written
> as "%0".
> Then for every line with target != -1 emits `arc(<name>, <state_no>, <target>, ...`.
> The in/out strings are "0" for symbol 0 (EPSILON), "?" for 1 (UNKNOWN) or 2 (IDENTITY),
> else the sigma string; then literal-symbol escapes are applied: a sigma string "0"
> (in > 0 / out > 0) becomes "%0"; a sigma string "?" with symbol number > 2 becomes "%?".
> The C out-side "?" escape tested stateptr->in > 2 (a copy typo, so a literal
> "?" out-symbol was only escaped when the in-symbol number was > 2); it now tests
> stateptr->out > 2, symmetrically with the in side, so a literal "?" out-symbol is escaped
> by its own symbol number.
> Arc payload: arity 2 with in == out == IDENTITY → `"?").`; arity 2 with in == out and
> in != UNKNOWN → single quoted symbol `"<in>").`; any other arity 2 → `"<in>":"<out>").`;
> arity 1 → `"<in>").` — all symbol texts run through escape_print.
> Finally emits `final(<name>, <i>).` for every state number i whose final flag is set,
> closes the file if one was opened, and frees the two temporary arrays.

> [spec:foma:def:io.fsm-read-binary-file-fn]
> struct fsm *fsm_read_binary_file(char *filename)

> [spec:foma:sem:io.fsm-read-binary-file-fn+1]
> Reads the first network from the foma binary file `filename`: create a handle with
> `[spec:foma:sem:io.io-init-fn]`, load the whole (possibly gzipped) file into memory via
> `[spec:foma:sem:io.io-gz-file-to-mem-fn]`, parse one net with
> `[spec:foma:sem:io.io-net-read-fn]`, free the handle, and return the net. The net-name
> string that C strdup'd and leaked is dropped here.
> Returns `Result<Box<Fsm>, FomaError>` instead of the C NULL sentinel — an
> unreadable or empty file (io_gz_file_to_mem == 0) is `Err(FomaError::Io(..))`, a
> structurally malformed image (io_net_read returns None) is `Err(FomaError::Format(..))`,
> and a parsed net is `Ok`.

> [spec:foma:def:io.fsm-read-binary-fn]
> fn fsm_read_binary<R: Read>(reader: R) -> Result<Box<Fsm>, FomaError>

> [spec:foma:sem:io.fsm-read-binary-fn]
> New public API (no C counterpart): generic stream binary read. Drains `reader`
> to a Vec and delegates to `[spec:foma:sem:io.fsm-read-binary-mem-fn]`. A read
> error is `Err(FomaError::Io(..))`.

> [spec:foma:def:io.fsm-read-binary-mem-fn]
> fn fsm_read_binary_mem(bytes: &[u8]) -> Result<Box<Fsm>, FomaError>

> [spec:foma:sem:io.fsm-read-binary-mem-fn]
> New public API (no C counterpart): read a foma binary image from memory. Sniffs
> the gzip magic (1f 8b) like `[spec:foma:sem:io.io-gz-file-to-mem-fn]`: if gzip,
> GzDecoder-decompress into a Vec, else use the bytes as-is; push a trailing 0
> terminator and parse with `[spec:foma:sem:io.io-net-read-fn]`. A malformed image
> (io_net_read None) is `Err(FomaError::Format(..))`, gzip decode failure is
> `Err(FomaError::Io(..))`.

> [spec:foma:def:io.fsm-read-binary-file-multiple-fn]
> struct fsm *fsm_read_binary_file_multiple(fsm_read_binary_handle fsrh)

> [spec:foma:sem:io.fsm-read-binary-file-multiple-fn]
> Reads the next network from a handle created by
> `[spec:foma:sem:io.fsm-read-binary-file-multiple-init-fn]`: casts the opaque handle
> back to struct io_buf_handle* and calls `[spec:foma:sem:io.io-net-read-fn]`. If that
> returns NULL (end of buffer or format error) the handle is freed with
> `[spec:foma:sem:io.io-free-fn]` and NULL is returned — the handle must not be used
> again after a NULL return. On success the returned net-name string is freed and the net
> returned. Callers loop until NULL to read every net in a multi-net file.

> [spec:foma:def:io.fsm-read-binary-file-multiple-init-fn]
> fsm_read_binary_handle fsm_read_binary_file_multiple_init(char *filename)

> [spec:foma:sem:io.fsm-read-binary-file-multiple-init-fn]
> Creates an opaque handle for iterating over all networks in a foma binary file:
> `[spec:foma:sem:io.io-init-fn]`, then `[spec:foma:sem:io.io-gz-file-to-mem-fn]` on
> `filename`; if that returns 0 (unreadable or empty file), frees the handle and returns
> NULL; otherwise returns the io_buf_handle pointer cast to fsm_read_binary_handle
> (a void* typedef).

> [spec:foma:def:io.fsm-read-prolog-fn]
> struct fsm *fsm_read_prolog (char *filename)

> [spec:foma:sem:io.fsm-read-prolog-fn+1]
> Parses a prolog-format network from `filename`; returns NULL if fopen fails or if no
> "network(" clause was seen. Reads lines of up to 1023 chars with fgets and recognizes
> only lines that begin exactly with "network(", "final(", "symbol(", or "arc(".
> The C source parsed each recognized line with unchecked strstr/strchr lookups (a missing
> delimiter NULL-derefs; a "final("/"symbol("/"arc(" fact before any "network(" clause
> dereferences a NULL handle). On any missing delimiter or absent net handle, print
> "File format error in prolog file.\n" and return NULL instead of crashing.
> network(: if one was already seen, prints "WARNING: prolog file contains multiple nets.
> Only returning the first one.\n" (via perror) and stops reading; otherwise extracts the
> name between "network(" and ")." and calls fsm_construct_init(name).
> final(: the text between the line's first space and ")." is atoi'd and that state is
> marked final.
> symbol(: the text between the first `, "` and `").` is the symbol; "%0" is unescaped to
> "0"; the symbol is added to the sigma if fsm_construct_check_symbol reports it absent.
> arc(: arity is 1 if the line contains no `":"` substring, or if it contains `, ":").`
> (a transition on the literal colon symbol); otherwise 2. Source = atoi of the text
> between the first space and the following comma; target = atoi of the text between the
> next space and comma. The in-symbol is the text after the next `"` up to `":` (arity 2)
> or `").` (arity 1); the arity-2 out-symbol is the text between `:"` and `").`.
> Symbol decoding, in this order: arity-1 "?" → "@_IDENTITY_SYMBOL_@"; arity-2 "?" →
> "@_UNKNOWN_SYMBOL_@" (each side independently); "0" → "@_EPSILON_SYMBOL_@"; "%0" →
> "0"; "%?" → "?". Adds arc source→target with in:in (arity 1) or in:out (arity 2).
> No `\"` unescaping is done, asymmetric with `[spec:foma:sem:io.foma-write-prolog-fn+1]`.
> After EOF (or on the second network( line): closes the file; if a net was started, set
> state 0 initial, fsm_construct_done, fsm_topsort (return value of topsort ignored —
> relies on in-place update), and return the net; else return NULL.

> [spec:foma:def:io.fsm-read-spaced-text-file-fn]
> struct fsm *fsm_read_spaced_text_file(char *filename)

> [spec:foma:sem:io.fsm-read-spaced-text-file-fn]
> Reads a spaced-text word list into a trie and returns fsm_trie_done(th) (deterministic,
> minimized). Loads the whole file with `[spec:foma:sem:io.file-to-mem-fn+1]` (error →
> return NULL). Records are separated by one or more blank lines; each record is either
> one line (an identity word) or two consecutive lines (an upper/lower transducer pair) —
> two adjacent non-blank lines are always consumed as a pair.
> Skip blank lines; take the next line as t1 (none → done); look at the following line as t2.
> If t2 is missing or blank: t1 is a single word; for each whitespace-separated token add token:token to the trie, except
> "0" adds "@_EPSILON_SYMBOL_@":"@_EPSILON_SYMBOL_@" and "%0" adds "0":"0"; then
> fsm_trie_end_word.
> Otherwise tokenize t1 and t2 in lockstep until both are exhausted; per side, a NULL
> token (shorter line) or "0" becomes "@_EPSILON_SYMBOL_@" and "%0" becomes "0"; add each
> in:out pair with fsm_trie_symbol, then fsm_trie_end_word.
> Frees the file buffer before returning.

> [spec:foma:def:io.fsm-read-text-file-fn]
> struct fsm *fsm_read_text_file(char *filename)

> [spec:foma:sem:io.fsm-read-text-file-fn]
> Reads a plain word list, one word per line, into a trie. Loads the file with
> `[spec:foma:sem:io.file-to-mem-fn+1]` (error → return None), then splits the buffer on
> '\n' in place: each non-empty line is added with fsm_trie_add_word; empty lines are
> skipped; a final segment without a trailing newline is included; iteration stops at the
> terminating '\0'. Frees the buffer and returns fsm_trie_done(th) — the deterministic
> minimal automaton accepting exactly the listed words.

> [spec:foma:def:io.fsm-write-binary-file-fn]
> int fsm_write_binary_file(struct fsm *net, char *filename)

> [spec:foma:sem:io.fsm-write-binary-file-fn]
> Saves `net` to `filename` in the gzip-compressed foma format: gzopen(filename, "wb");
> if that fails return 1; otherwise write the net with
> `[spec:foma:sem:io.foma-net-print-fn]`, gzclose, and return 0. Note the return
> convention: 0 = success, 1 = failure.

> [spec:foma:def:io.fsm-write-binary-fn]
> fn fsm_write_binary<W: Write>(net: &Fsm, out: W) -> std::io::Result<()>

> [spec:foma:sem:io.fsm-write-binary-fn]
> New public API (no C counterpart): generic stream binary write. Wraps `out` in a
> GzEncoder (Compression::default()), writes the net with
> `[spec:foma:sem:io.foma-net-print-fn]`, finishes the gzip stream, and returns
> `Ok(())`. Mirrors `[spec:foma:sem:io.fsm-write-binary-file-fn]`'s gzip behavior
> but to an arbitrary writer; round-trips with
> `[spec:foma:sem:io.fsm-read-binary-fn]`.

> [spec:foma:def:io.io-buf-handle]
> struct io_buf_handle {
>   char *io_buf;
>   char *io_buf_ptr;
> }

> [spec:foma:def:io.io-free-fn]
> void io_free(struct io_buf_handle *iobh)

> [spec:foma:sem:io.io-free-fn]
> Frees an io_buf_handle: if iobh->io_buf is non-NULL, free it and set it to NULL; then
> free the handle struct itself. io_buf_ptr (an interior pointer into io_buf) is not
> freed separately.

> [spec:foma:def:io.io-get-file-size-fn]
> static size_t io_get_file_size(char *filename)

> [spec:foma:sem:io.io-get-file-size-fn]
> Returns the number of bytes `filename` will yield after gz decompression, or 0 if
> gzopen fails. Opens with gzopen(filename, "r"): if gzdirect() == 1 (the file is not
> gzip data and will be read raw), close and return the plain on-disk size via
> `[spec:foma:sem:io.io-get-regular-file-size-fn]`; otherwise close and return the gzip
> trailer size via `[spec:foma:sem:io.io-get-gz-file-size-fn]`.

> [spec:foma:def:io.io-get-gz-file-size-fn]
> static size_t io_get_gz_file_size(char *filename)

> [spec:foma:sem:io.io-get-gz-file-size-fn]
> Returns the uncompressed data size recorded in a gzip file's trailer: fopen, fseek to 4
> bytes before EOF, read the last 4 bytes, fclose, and assemble them little-endian as
> b0 | b1<<8 | b2<<16 | b3<<24.
> Caveats: this is the gzip ISIZE field — the uncompressed length mod 2^32 of only the
> last gzip member — so it is wrong for data >= 4 GB and for multi-member files; fopen's
> result is not checked (NULL dereference if the file vanished; in practice only called
> after a successful gzopen of the same path).

> [spec:foma:def:io.io-get-regular-file-size-fn]
> static size_t io_get_regular_file_size(char *filename)

> [spec:foma:sem:io.io-get-regular-file-size-fn]
> Returns the on-disk size of `filename` in bytes: fopen, fseek(0, SEEK_END), ftell,
> fclose. fopen's result is not checked (NULL dereference on failure; only reached after
> a successful gzopen of the same path in `[spec:foma:sem:io.io-get-file-size-fn]`).

> [spec:foma:def:io.io-gets-fn]
> static int io_gets(struct io_buf_handle *iobh, char *target)

> [spec:foma:sem:io.io-gets-fn]
> In-memory line reader: copies bytes from iobh->io_buf_ptr into `target` up to but
> excluding the next '\n' or '\0', NUL-terminates target, then advances io_buf_ptr past
> the '\n', or onto the '\0' at end of buffer (so at end-of-buffer every subsequent call
> returns 0 with an empty target). Returns the number of bytes copied (the line length);
> 0 means either an empty line or end of buffer — callers cannot distinguish the two.
> No bounds check on target: callers pass a READ_BUF_SIZE (4096) byte buffer, and a
> longer line overruns it (latent bug).

> [spec:foma:def:io.io-gz-file-to-mem-fn]
> size_t io_gz_file_to_mem(struct io_buf_handle *iobh, char *filename)

> [spec:foma:sem:io.io-gz-file-to-mem-fn]
> Loads the (possibly gzip-compressed) file into the handle's buffer: size =
> `[spec:foma:sem:io.io-get-file-size-fn]`; if 0, return 0. malloc(size+1) into
> iobh->io_buf, gzopen(filename, "rb"), gzread(size bytes) — zlib transparently
> decompresses gzip data and passes plain files through unchanged — gzclose, store '\0'
> at buf[size], set io_buf_ptr to the buffer start, return size. gzread's return value is
> unchecked: a corrupt gzip body silently leaves the tail of the buffer uninitialized.

> [spec:foma:def:io.io-init-fn]
> struct io_buf_handle *io_init()

> [spec:foma:sem:io.io-init-fn]
> Allocates a new struct io_buf_handle with malloc, sets both io_buf and io_buf_ptr to
> NULL, and returns it. The malloc result is not checked.

> [spec:foma:def:io.io-net-read-fn]
> struct fsm *io_net_read(struct io_buf_handle *iobh, char **net_name)

> [spec:foma:sem:io.io-net-read-fn+3]
> Parses one network in the foma text format from the handle's in-memory buffer (format
> as written by `[spec:foma:sem:io.foma-net-print-fn]`). All lines are read with
> `[spec:foma:sem:io.io-gets-fn]` into a stack buffer of READ_BUF_SIZE (4096) bytes.
> Returns the new struct fsm and stores a strdup of the net name into *net_name; returns
> NULL at end of buffer or on format error (errors print/perror a diagnostic; the fsm is
> destroyed on the header/props/sigma-header errors but leaked on later ones, and
> *net_name is left unset on errors before the props line).
> 1. Read a line; if io_gets returned 0, return NULL (normal end of a multi-net stream).
> Create the net with fsm_create(""). The line must equal "##foma-net 1.0##", else
> perror("File format error foma!\n"), destroy, NULL.
> 2. The next line must be "##props##" (else perror, destroy, NULL). The line after it is
> parsed with sscanf format "%i %i %i %i %i %lld %i %i %i %i %i %i %s" into arity,
> arccount, statecount, linecount, finalcount, pathcount, is_deterministic, is_pruned,
> is_minimized, is_epsilon_free, is_loop_free, extras, name; name (empty when the field is
> absent) is capped at FSM_NAME_LEN (40) into net->name and strdup'd into *net_name. C's sscanf
> left the buffer holding the whole props line when the name field was absent, so that line
> became the net name. extras is unpacked as:
> is_completed = extras & 3; arcs_sorted_in = (extras & 12) >> 2; arcs_sorted_out =
> (extras & 48) >> 4.
> 3. Lines are then skipped until one equals "##sigma##" (future-expansion room); hitting
> an empty line / end of buffer first prints "File format error at sigma definition!\n",
> destroys the net, returns NULL.
> 4. Sigma lines are read until a line starting with '#': each is "<number> <string>",
> split at the first space; a line with no space is a format error ("File format error in
> sigma section!", net destroyed, NULL returned) rather than the C source's strstr NULL-deref.
> An empty remainder means the symbol is a literal newline
> "\n" (how a newline symbol survives the line-oriented format); each calls
> sigma_add_number(net->sigma, string, number). Truly empty lines are skipped, but the read
> cursor is checked for progress first: at end-of-buffer io_gets yields empty lines without
> advancing, so a file truncated inside the sigma section is detected ("File format error in
> sigma section!"), the net destroyed and NULL returned. The C source had no such check and
> looped forever on a truncated sigma section.
> 5. The '#' line must be "##states##" (else message + NULL). malloc linecount *
> sizeof(struct fsm_state) entries. Read lines until one starts with '#', parsing each
> with `[spec:foma:sem:io.explode-line-fn]` into 2–5 ints, maintaining laststate (init
> -1) and last_final (a char, init '1' i.e. 49 — only consumed if the first line has 2 or
> 3 fields, which well-formed files never produce). Field meanings:
> 2 fields: in target — state_no = laststate, out = in, final_state = last_final;
> 3 fields: in out target — state_no = laststate, final_state = last_final;
> 4 fields: state_no in target final_state (out = in), updating laststate/last_final;
> 5 fields: state_no in out target final_state, updating laststate/last_final;
> any other count: "File format error\n", return NULL. Per entry, start_state is 1 if
> laststate == 0, 0 if laststate > 0, and -1 if laststate == -1; the writer's sentinel
> line "-1 -1 -1 -1 -1" flows through the 5-field path and becomes the terminating
> state_no == -1 entry.
> 6. If the '#' line is "##cmatrix##": cmatrix_init(net), then read one integer per line
> into net->medlookup->confusion_matrix (sequentially, no bounds check) until a '#' line.
> 7. The final line must be "##end##" (else message + NULL). Return the net.

> [spec:foma:def:io.load-defined-fn]
> int load_defined(struct defined_networks *def, char *filename)

> [spec:foma:sem:io.load-defined-fn]
> Loads saved definitions from `filename` into the collection `def`: create a handle with
> `[spec:foma:sem:io.io-init-fn]`, print "Loading definitions from <f>.\n", then
> `[spec:foma:sem:io.io-gz-file-to-mem-fn]`; on 0 print "File error.\n" to stderr, free
> the handle, return 0. Otherwise repeatedly `[spec:foma:sem:io.io-net-read-fn]` and call
> add_defined(def, net, net_name) for each network (the stored net name is the definition
> name) until NULL. Free the handle, return 1. add_defined copies the name, so each
> strdup'd net_name is leaked.

> [spec:foma:def:io.net-print-att-fn]
> int net_print_att(struct fsm *net, FILE *outfile)

> [spec:foma:sem:io.net-print-att-fn+1]
> Writes `net` to the open sink in AT&T tab-separated format, returning `Ok(())` on
> success and propagating the first write failure as its `io::Error` (the C returned a
> vestigial `1` no caller inspected; a write error was silently dropped). Builds a
> symbol-number → string table with sigma_to_list; if sigma_max >= 0, entry 0 (epsilon)
> is replaced by the global g_att_epsilon (default "@0@", user-settable via the
> "att-epsilon" variable).
> First pass over the state array: every line with target != -1 prints
> "state_no\ttarget\tinsym\toutsym\n" (symbols looked up by number, so UNKNOWN/IDENTITY
> print as their sigma strings "@_UNKNOWN_SYMBOL_@"/"@_IDENTITY_SYMBOL_@").
> Second pass: for the first line of each state (state_no differs from the previous
> line's), if final_state == 1 print "state_no\n". Arcs therefore all precede final-state
> lines; no weights are ever written. Frees the symbol table.

> [spec:foma:def:io.read-att-fn]
> struct fsm *read_att(char *filename)

> [spec:foma:sem:io.read-att-fn]
> Reads an AT&T tab-separated file into a new fsm; returns NULL if fopen fails. Uses the
> fsm_construct API with `filename` as the net name.
> Per fgets line (1024-byte buffer): strip a single trailing '\n'; tokenize on '\t' with
> strtok, collecting at most 6 tokens (any further fields ignored). 0 tokens (blank line)
> → skip. 4 or more tokens: an arc — source = atoi(tok0), target = atoi(tok1), in-symbol
> = tok2, out-symbol = tok3 (tok4/tok5, e.g. weights, are discarded); a symbol string
> equal to g_att_epsilon (default "@0@") is replaced by "@_EPSILON_SYMBOL_@"; call
> fsm_construct_add_arc. 1–3 tokens: a final-state declaration —
> fsm_construct_set_final(atoi(tok0)) (so "state<TAB>weight" lines work; a malformed
> 3-field line is also silently treated as a final state). Symbols are not unescaped.
> After EOF: set state 0 initial, fsm_construct_done, fsm_count, net = fsm_topsort(net),
> return the net.

> [spec:foma:def:io.save-defined-fn]
> int save_defined(struct defined_networks *def, char *filename)

> [spec:foma:sem:io.save-defined-fn]
> Saves all defined networks into one gzipped foma file. If def == NULL, print "No
> defined networks.\n" to stderr and return 0. If gzopen(filename, "wb") fails, print
> "Error opening file <f> for writing.\n" and return -1. Print "Writing definitions to
> file <f>.\n", then walk the defined_networks list: entries with a NULL net print
> "Skipping definition without network.\n" and are skipped; otherwise the definition name
> is strncpy'd into the net's name field (FSM_NAME_LEN = 40 bytes; no forced NUL
> terminator if the name is that long or longer) and the net is appended with
> `[spec:foma:sem:io.foma-net-print-fn]`. gzclose; return 1. Reload with
> `[spec:foma:sem:io.load-defined-fn]` recovers the names from the stored nets.

