# foma/cgflookup.c

> [spec:foma:def:cgflookup.app-print-fn]
> void app_print(char *result)

> [spec:foma:sem:cgflookup.app-print-fn]
> Emits one lookup result in constraint-grammar (CG) cohort format for the current global
> input `line`, to stdout.
> result == NULL (a word with no analyses): print only the cohort header, `"\"<%s>\"\n"`
> with the input line (i.e. `"<word>"` on its own line, no readings). Note
> `[spec:foma:sem:cgflookup.handle-line-fn]` prints this same header itself before the
> first real result, so the NULL branch is reached only via main's no-results fallback.
> result != NULL: print one reading line: indent ("\t", fixed) + result + "\n". With -u
> (mark_uppercase), the first character of the input line is converted with
> mbstowcs(dest, line, 1) (locale-dependent; locale is set at option-parse time) and if
> iswupper() reports it uppercase, the reading is printed as indent + result + " <*>\n" —
> tagging readings of capitalized wordforms.

> [spec:foma:def:cgflookup.applyer-fn]
> static char *(*applyer)(struct apply_handle *h, char *word) = &apply_up

> [spec:foma:sem:cgflookup.applyer-fn]
> File-static function pointer through which every lookup is made, defaulting to
> apply_up; the -i option repoints it to apply_down. Apply-API calling convention: the
> first call for a word passes the word string; each subsequent call passes NULL to fetch
> the next result for the same word; a NULL return means no (more) results.

> [spec:foma:def:cgflookup.get-next-line-fn]
> char *get_next_line()

> [spec:foma:sem:cgflookup.get-next-line-fn]
> Reads the next input line from the global FILE* INFILE into the global `line` buffer
> with fgets(line, LINE_LIMIT = 262144, INFILE); on success truncates the line at its
> first '\n' or '\r' (strcspn over "\n\r") and returns `line`; returns NULL at EOF.
> A physical line of LINE_LIMIT-1 chars or more is silently split into multiple logical
> lines (fgets chunking); no error is raised.

> [spec:foma:def:cgflookup.handle-line-fn]
> void handle_line(char *s)

> [spec:foma:sem:cgflookup.handle-line-fn]
> Same chain traversal as `[spec:foma:sem:flookup.handle-line-fn]` — alternates mode
> (-a): head-to-tail, the first net that yields a result has all its results printed and
> the rest of the chain skipped; default cascade mode: depth-first composition where each
> node's result feeds the next node, tail results are printed and drained, and NULL
> results trigger backtracking via applyer(prev, NULL) until the head is exhausted — with
> CG output framing added: `results` is reset to 0 on entry (redundantly with main), and
> immediately before printing the first result of the word (when results reaches 1) the
> cohort header `"\"<line>\"\n"` is printed to stdout; each result is then printed as an
> indented reading via `[spec:foma:sem:cgflookup.app-print-fn]`. A word with no results
> prints nothing here; main's fallback app_print(NULL) supplies the bare header.

> [spec:foma:def:cgflookup.lookup-chain]
> struct lookup_chain {
>   struct fsm *net;
>   struct apply_handle *ah;
>   struct lookup_chain *next;
>   struct lookup_chain *prev;
> }

> [spec:foma:def:cgflookup.main-fn]
> int main(int argc, char *argv[])

> [spec:foma:sem:cgflookup.main-fn+1]
> Like `[spec:foma:sem:flookup.main-fn]` but stdin-only (no UDP server mode, no Windows
> socket setup) and with CG cohort output. stdout is set to full buffering over a static
> 2048-byte buffer (setvbuf _IOFBF).
> Options (getopt string "abhHiI:qs:uw:vx"): -a alternates mode; -b unbuffered output
> (flush after every word); -h print usage + help, exit 0; -i apply down (applyer =
> apply_down, direction = DIR_DOWN); -q don't sort arcs; -I <arg> arc indexing parsed
> exactly as in flookup, so "4k"/"4M" set a memory limit (the C source required both letter
> cases, 'k' and 'K' or 'm' and 'M', in the same arg, so those fell through to the digit
> branch); -s <sep> sets separator (default "\t"; note it is never used — there is no echo
> mode); -u sets mark_uppercase and calls setlocale(LC_CTYPE, "") (on failure prints "Check
> uppercase flag is on, but can't set locale!" to stderr and continues); -w <sep> word
> separator (default "" — empty, unlike flookup's "\n"); -v print "cgflookup 1.03 (foma
> library version <v>)" and exit 0; -x (advertised in the usage text; disables echo, a no-op
> since cgflookup does not echo) is accepted and ignored. The C source had no case for -x, so
> it printed the usage string to stderr and exit(EXIT_FAILURE). 'H' is in the optstring but
> has no switch case, so it (like any unknown option, or a missing file operand optind == argc)
> prints usage to stderr and exits with failure.
> Net loading, per-net arc sorting (fsm_sort_arcs 1/2 by direction unless -q or already
> sorted), apply_init, optional apply_index (APPLY_INDEX_INPUT for down /
> APPLY_INDEX_OUTPUT for up with cutoff, mem limit, flag-only), and chain construction
> (append for down/alternates, prepend for the default up direction so up-mode runs nets
> in reverse file order) are identical to flookup; init failure → perror("File error")
> and exit; zero nets → "File error: <name>" to stderr and exit failure.
> Main loop: line = calloc(LINE_LIMIT = 262144); INFILE = stdin; for each
> `[spec:foma:sem:cgflookup.get-next-line-fn]` line: results = 0;
> `[spec:foma:sem:cgflookup.handle-line-fn]`; if results == 0,
> `[spec:foma:sem:cgflookup.app-print-fn]` (NULL) prints the bare cohort header
> `"<word>"`; then print wordseparator (default empty) and fflush(stdout) if -b.
> Cleanup: apply_clear + fsm_destroy per chain node, free the nodes and the line buffer,
> exit(0).
