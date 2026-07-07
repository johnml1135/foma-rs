# foma/flookup.c

> [spec:foma:def:flookup.app-print-fn]
> void app_print(char *result)

> [spec:foma:sem:flookup.app-print-fn+1]
> Emits one lookup result — or the no-result marker when result == NULL — for the current
> global input `line`.
> stdin mode (mode_server == 0): if echo is on (default; disabled by -x), print the input
> line followed by `separator` (default "\t"); then print result + "\n", or "+?\n" if
> result is NULL. Output goes to stdout (fully buffered; flushing is the caller's job).
> Server mode: instead of printing, append the same content to the UDP reply buffer
> serverstring (calloc'd UDP_MAX+1 = 65536 bytes): line + separator when echoing, then
> result + "\n", or "+?\n" for a NULL result — the same failure marker as stdin mode. The C
> source emitted "?+" in server mode (the reverse), an inconsistency between the two paths.
> Each append is strncat(serverstring + udpsize, src, UDP_MAX - udpsize), after which
> udpsize is advanced by the full strlen(src) even when strncat truncated; once the
> buffer fills, udpsize can exceed UDP_MAX so later length arguments go negative and are
> converted to a huge size_t, making strncat unbounded (latent buffer-overflow bug near
> the 64 kB reply limit).

> [spec:foma:def:flookup.applyer-fn]
> static char *(*applyer)(struct apply_handle *h, char *word) = &apply_up

> [spec:foma:sem:flookup.applyer-fn]
> File-static function pointer through which every lookup is made, defaulting to
> apply_up; the -i option repoints it to apply_down. Apply-API calling convention: the
> first call for a word passes the word string; each subsequent call passes NULL to fetch
> the next result for the same word; a NULL return means no (more) results.

> [spec:foma:def:flookup.get-next-line-fn]
> char *get_next_line()

> [spec:foma:sem:flookup.get-next-line-fn]
> Reads the next input line from the global FILE* INFILE into the global `line` buffer
> with fgets(line, LINE_LIMIT = 262144, INFILE); on success truncates the line at its
> first '\n' or '\r' (strcspn over "\n\r") and returns `line`; returns NULL at EOF.
> A physical line of LINE_LIMIT-1 chars or more is silently split into multiple logical
> lines (fgets chunking); no error is raised.

> [spec:foma:def:flookup.handle-line-fn]
> void handle_line(char *s)

> [spec:foma:sem:flookup.handle-line-fn]
> Runs one input word `s` through the loaded chain of nets, calling
> `[spec:foma:sem:flookup.app-print-fn]` for every output and incrementing the global
> `results` counter (the caller resets it beforehand and prints the failure marker itself
> if it remains 0).
> Alternates mode (apply_alternates == 1, the -a flag): walk the chain head → tail; for
> each net call applyer(ah, s); the first net returning non-NULL has that result printed,
> then all its remaining results drained (applyer(ah, NULL) until NULL) and printed, and
> the loop breaks — priority-union semantics; if a net yields nothing, the next is tried,
> giving up silently after the tail.
> Cascade mode (default): simulates composition by depth-first search. Start at the head
> with the input word as the current string. At each node call applyer(node, current):
> if a result is returned and the node is not the tail, the result becomes the input to
> the next node (descend); if the node is the tail, print the result and drain/print all
> further tail results via applyer(tail, NULL). Whenever a node returns NULL (including
> after the tail is drained), backtrack: walk prev-wards asking each earlier node for its
> next result with applyer(prev, NULL); the first node that yields one supplies the new
> current string and the search resumes downward from its successor; if backtracking
> walks off the head (position becomes NULL) the search ends. Every result of every path
> through the cascade is printed; duplicates are not removed and nothing is sorted.

> [spec:foma:def:flookup.lookup-chain]
> struct lookup_chain {
>   struct fsm *net;
>   struct apply_handle *ah;
>   struct lookup_chain *next;
>   struct lookup_chain *prev;
> }

> [spec:foma:def:flookup.main-fn]
> int main(int argc, char *argv[])

> [spec:foma:sem:flookup.main-fn+1]
> flookup applies words from stdin (or from UDP datagrams in server mode) to one or more
> nets read from a foma binary file and writes results to stdout. On Windows, Winsock is
> initialized first (WSAStartup 2.2; failure → "WSAStartup failed" to stderr, return 1),
> WSACleanup is called before the final chain cleanup, and close() on sockets maps to
> closesocket(). stdout is set to full buffering over a static 2048-byte buffer (setvbuf
> _IOFBF).
> Options (getopt string "abhHiI:qs:SA:P:w:vx"): -a alternates mode; -b unbuffered output
> (flush after every input word); -h print usage + help and exit 0; -i apply down instead
> of up (direction = DIR_DOWN, applyer = apply_down); -q don't sort arcs; -s <sep>
> input/output separator (default "\t"); -S UDP server mode; -A <addr> server bind
> address; -P <port> server port (default FLOOKUP_PORT = 6062); -w <sep> word separator
> (default "\n"); -v print "flookup 1.03 (foma library version <v>)" and exit 0; -x don't
> echo the input string. -I <arg> arc indexing: arg "f" → index only flag-containing
> states; arg containing both 'k' and 'K' → memory limit 1024*atoi(arg); both 'm' and 'M'
> → 1024*1024*atoi(arg); else if arg starts with a digit → index states with >=
> atoi(arg) arcs. The k/m branches match when the arg contains 'k'/'K' (resp. 'm'/'M'), so
> "-I 4k"/"-I 4M" set a memory limit as advertised. The C source required BOTH letter cases
> present in the same arg, so "-I 4k" fell through to the digit branch and set an arc-count
> cutoff of 4 (the k/m suffix was silently ignored).
> 'H' is in the optstring but has no case, so it falls to default — usage to stderr,
> exit(EXIT_FAILURE) — like any unknown option; a missing file operand (optind == argc)
> does the same.
> Net loading: argv[optind] is opened with
> `[spec:foma:sem:io.fsm-read-binary-file-multiple-init-fn]` (NULL → perror("File
> error"), exit failure) and nets are read in a loop via
> `[spec:foma:sem:io.fsm-read-binary-file-multiple-fn]`. Per net: unless -q, sort arcs
> with fsm_sort_arcs(net, 1) when applying down and arcs_sorted_in != 1, or
> fsm_sort_arcs(net, 2) when applying up and arcs_sorted_out != 1; create an apply handle
> with apply_init; if -I was given, apply_index(ah, APPLY_INDEX_INPUT (down) or
> APPLY_INDEX_OUTPUT (up), index_cutoff, index_mem_limit, index_flag_states).
> Chain construction: the first net becomes head = tail; each further net is appended at
> the tail when direction is down or alternates mode is on, but prepended at the head for
> the default up direction — an up-mode cascade thus runs nets in reverse file order
> (inverting a composition stored top-to-bottom). Zero nets loaded → "File error:
> <name>" to stderr, exit failure.
> Server mode (-S): `[spec:foma:sem:flookup.server-init-fn]`; serverstring and line are
> each calloc'd UDP_MAX+1 bytes; then loop forever: recvfrom(listen_sd, line, UDP_MAX)
> recording the client address (error → perror("recvfrom() failed, aborting") and break
> out); NUL-terminate at the byte count and truncate at the first '\n'/'\r'; reset
> results = 0, udpsize = 0, serverstring[0] = '\0'; run
> `[spec:foma:sem:flookup.handle-line-fn]`; if results == 0 call app_print(NULL); if
> serverstring is non-empty, sendto() it back to the datagram's source (error →
> perror("sendto() failed"), keep serving). One request datagram holds one word; one
> reply datagram holds all its results, newline-separated (no word separator is added).
> stdin mode: line = calloc(LINE_LIMIT); INFILE = stdin; for each
> `[spec:foma:sem:flookup.get-next-line-fn]` line: results = 0; handle_line(line); if
> results == 0 app_print(NULL); print wordseparator; fflush(stdout) when -b was given.
> Cleanup: walk the chain calling apply_clear on each handle and fsm_destroy on each net,
> free the nodes, free serverstring/line if allocated, exit(0).

> [spec:foma:def:flookup.server-init-fn]
> void server_init(void)

> [spec:foma:sem:flookup.server-init-fn]
> Creates and binds the UDP server socket, stored in the global listen_sd.
> socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP); then setsockopt SO_RCVBUF and SO_SNDBUF are
> both set to 262144 bytes; any failure so far → perror + exit(1).
> serveraddr (sockaddr_in) is zeroed and filled: sin_family = AF_INET, sin_port =
> htons(port_number) (default 6062, overridden by -P); sin_addr from inet_pton(AF_INET,
> server_address) when -A was given — a return of 0 prints "inet_pton() failed: string is
> not a valid address.\n" and exits 1, any other non-1 return perrors and exits — else
> INADDR_ANY.
> bind() failure → perror + exit(1). On success the bound address is formatted with
> inet_ntop (failure also fatal) and "Started flookup server on <addr> port <port>\n" is
> printed and stdout flushed. IPv4/UDP only; single-threaded and synchronous — requests
> are served one at a time by the recvfrom loop in `[spec:foma:sem:flookup.main-fn]`.
