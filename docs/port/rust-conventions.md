# foma → Rust porting conventions (Wave 2: literal, bug-for-bug)

These conventions bind every Wave-2 translation. The goal is a 1:1 port:
a reviewer diffing a C function against its Rust twin must see a
line-by-line correspondence. Idiom is Wave 4's job; fixes land after the
port is green. **Reproduce bugs faithfully** (off-by-ones, overflow,
signed-char hashing, order of side effects) unless memory-unsafe to do
so — see "Deviations" below.

## Layout

- One crate: `crates/foma`. One Rust module per C file:
  `foma/structures.c` → `crates/foma/src/structures.rs`, etc.
- Types declared in the C headers (`foma.h`, `fomalib.h`,
  `fomalibconf.h`, `lexc.h`) live in `crates/foma/src/types.rs`.
  Types declared inside a `.c` file (e.g. determinize's nhash tables)
  live in that file's module.
- CLI binaries: `src/bin/foma.rs`, `src/bin/flookup.rs`,
  `src/bin/cgflookup.rs` (w2-cli concern).

## Type mappings (match C widths exactly — sem rules document truncation)

| C | Rust |
|---|---|
| `int` | `i32` |
| `unsigned int` | `u32` |
| `short int` | `i16` |
| `unsigned short` | `u16` |
| `char` (numeric/flag) | `i8` (or `u8` when the C treats it as a byte) |
| `long long` | `i64` |
| `size_t` | `usize` |
| `_Bool` | `bool` (watch documented `_Bool` truncation quirks: reproduce via `!= 0` on the same expression the C truncates) |
| `char *` (owned string) | `String` / `Option<String>` |
| `char *` (byte buffer) | `Vec<u8>` |
| `char name[40]` | `String` capped at 40 bytes; reproduce the "no NUL when ≥ 40" quirk as truncation to 40 bytes |

Hashing/char arithmetic: where a sem rule documents signed-char
sign-extension (`sh_hashf`, `trie_hashf`, `lexc_symbol_hash`,
`fsm_construct_hash_sym`), iterate `s.as_bytes()` and cast each byte
`as i8 as i32` to reproduce it. Where a rule documents wrapping overflow
(e.g. `triplethash_hashf`), use `wrapping_mul`/`wrapping_add` on `i32`.

## Memory model

- Malloc'd arrays → `Vec<T>`. Keep the C's explicit capacity/length
  bookkeeping fields when the algorithm reads them (don't substitute
  `vec.len()` where the C tracks a separate counter — that's idiom).
- The fsm line table stays a sentinel-terminated `Vec<FsmState>` with a
  final `state_no == -1` sentinel line, exactly as in C. Iteration is by
  index, mirroring pointer walks.
- Owned singly/doubly linked lists → `Option<Box<Node>>` chains with the
  same insert/delete order. Where the C keeps a dummy head, keep it.
- Handles (`apply_handle`, `fsm_construct_handle`, …) → owned structs
  passed `&mut`.
- C functions that *consume* (free) their `struct fsm *` arguments take
  `Fsm` by value; functions that borrow take `&Fsm`/`&mut Fsm`. The
  per-function sem rule states which convention each function uses —
  follow it exactly. (`Fsm` is passed unboxed: a move is a shallow copy of
  the struct header, the `Vec`/`LineTable` heap buffers transfer in place,
  and there is no per-net heap allocation. `Box` is reserved for the
  recursive linked-list nodes above and for nullable owning slots that
  genuinely need pointer indirection, not for `Fsm` itself.)
- File-static mutable globals → module-level
  `thread_local! { static NAME: RefCell<T> = ... }`. Keep the C names
  (upper-cased). Non-reentrancy is part of the contract; do not
  redesign into handle-passing where the C didn't (Wave 4).

## Control flow

- Keep loop shapes, early returns, and statement order. `goto` →
  labelled `loop`/`break 'label` with the same targets.
- Keep function names (already snake_case) and parameter names.
- No traits, no iterator chains, no combining functions. One C function
  = one Rust `pub fn` (file-statics become `pub(crate) fn`).

## Deviations (memory-unsafe C behavior)

Safe Rust cannot reproduce use-after-free / double-free / OOB reads.
For each such documented bug (they are flagged in the sem rules):
reproduce the *observable* result the C exhibits in practice where
possible; otherwise implement the nearest safe behavior, add
`// DEVIATION from C (<one line why>)` at the site, and note it in the
concern's commit message. Never silently "fix" logic bugs that are
memory-safe (e.g. `flag_eliminate`'s `|`-for-`&` filter — port it as-is).

## Stubs for not-yet-ported callees

Concerns land in dependency order, but the C has call cycles across
files. When your module calls a function whose concern hasn't landed,
add `pub fn name(...) -> ... { todo!("ported by <concern-id>") }` in
that function's *home module* (create the module file if needed) —
WITHOUT spec annotations. The owning concern replaces the stub and adds
the annotations. `cargo check` must pass after every concern.

## Annotations (what the Wave-2 gate counts)

Above every ported item, carry its manifest ids as line comments:

```rust
// [spec:foma:def:structures.fsm-create-fn]
// [spec:foma:sem:structures.fsm-create-fn]
pub fn fsm_create(name: &str) -> Fsm { ... }
```

Header prototypes got their own rule ids (they did NOT dedup with the
per-file impl ids). A function declared in a header carries BOTH id
families at its single Rust site, e.g. `apply_init` carries
`apply.apply-init-fn` (def+sem) AND `fomalib.apply-init-fn` (def+sem).
Before annotating, grep `docs/spec/port/foma/{fomalib,foma,fomalibconf,lexc}.md`
for the function's name to find its header-layer ids. Types from headers
carry the header id (e.g. `fomalib.fsm`) plus any duplicate per-file id.

## Verification per concern

1. `cargo check` clean (warnings acceptable in Wave 2 if C-faithful).
2. Every symbol in the concern shows `tgt_impl: true` in
   `nplan port status` (annotation present in target source).
3. No Wave-4 idiom crept in.

## Wave 3: tests

- Unit tests live in a `#[cfg(test)] mod tests` block at the bottom of the
  module under test (access to `pub(crate)` internals; thread-local state is
  isolated per test thread automatically).
- CLI/binary behavior (iface printed output, foma REPL, flookup/cgflookup) is
  tested as integration tests in `crates/foma/tests/`, spawning the built
  binaries and asserting on stdout/stderr bytes.
- Every function symbol needs a `/test` facet: put
  `// [spec:foma:sem:<id>/test]` directly above the `#[test]` fn (or the
  assertion site) that verifies it. Header-duplicate ids get their facet
  line at the same site (e.g. `apply.apply-init-fn/test` AND
  `fomalib.apply-init-fn/test`).
- The test asserts the SEM RULE's behavior — including the documented bugs
  (a test that "fixes" a bug is wrong for Wave 3; pin the literal behavior
  so Wave 4 diffs against it). DEVIATION sites assert the deviated-but-safe
  behavior (`#[should_panic]` where the C had UB and the port panics).
- Dead prototypes (fsm_find_ambiguous, fsm_mark_ambiguous, save_stack_att,
  int_stack_status): `#[should_panic]` tests pinning the never-callable
  contract.
- The C foma at /opt/homebrew/bin/foma may be used while WRITING tests to
  derive expected values, but tests must not invoke it at runtime.
- Never annotate aspirationally: the facet goes in only once the test exists
  and passes (`cargo test` green is part of every Wave-3 concern's gate).

## Wave 4: idiomatize

The sem rules are the behavioral spec; the 546 ported tests are the
oracle. Refactor freely while (a) every sem rule stays covered on the
target side and (b) tests stay green. Intended divergence = update the
rule body, bump `+N` in the rule header AND the target annotation, and
update the test deliberately. The C-side annotation goes stale — that is
recorded history, not an error.

### API model

- **Reentrancy**: library modules lose their `thread_local!` state.
  - dynarray's `fsm_state_*` builder statics → an owned `FsmBuilder`
    struct (`fsm_state_init` → `FsmBuilder::new`, methods take `&mut self`;
    old free functions become thin deprecated-free wrappers only if a
    caller still needs them — otherwise update the callers).
  - determinize/minimize/constructions scratch pools → locals or
    algorithm structs; nothing survives a call.
  - `define`'s `G_DEFINES`/`G_DEFINES_F` → a caller-owned `Definitions`
    registry passed `&mut` (regex already takes it as a param — make
    every path explicit).
  - CLI state (network stack, prompt/apply mode, g_* option globals)
    moves into a `Session` struct owned by the binaries; `iface_*`
    functions become methods or take `&mut Session`.
- **Errors**: library code never calls `exit()` and never panics on
  user input. Introduce `FomaError` (thiserror-style enum, hand-rolled —
  no new deps) + `Result<T, FomaError>`. `exit(1)` sites in the library
  (int_stack overflow, io fatal paths) → `Result` with rule bumps.
  Binaries translate errors to exit codes and messages.
- **Apply API**: `apply_down`/`apply_up`/`apply_words` etc. gain
  iterator front-ends (`ApplyHandle::down(word) -> impl Iterator<Item=String>`)
  wrapping the resume protocol; the C-shaped functions stay as thin
  wrappers so annotations and tests keep a stable home.
- **Names**: keep the C snake_case names for the public surface;
  idiomatic signatures (`&str`, `Result`, iterators, `Option`). No trait
  hierarchies; no generics beyond `Read`/`Write` in io.
- **Representation**: the sentinel-terminated `Vec<FsmState>` line table
  and `Sigma` list stay in Wave 4 (changing the core representation is
  redesign, not idiomatization — record as future work).

### Module split map (annotations move with their functions)

- `constructions.rs` → `constructions/` submodules: `helpers`,
  `merge_sigma`, `triplet_hash`, `products`, `boolean`, `closure`,
  `derived` (quotients/priority unions/ignore/…); `constructions.rs`
  re-exports the public surface so callers don't churn.
- `iface.rs` → `iface/` submodules by command family (`stack_ops`,
  `unary`, `binary`, `apply_cmds`, `print`, `io_cmds`, `tests_cmds`,
  `variables`); public surface re-exported.
- Other modules stay single files.

### Documented-bug policy (per concern, every flagged bug gets ONE of)

1. **Fix** — when the bug is a genuine defect (wrong result, crash,
   corruption): fix code, rewrite the sem rule body, bump `+N`, update
   tests. Canonical fixes: `stack_turn` infinite loop (implement the
   evident intent: reverse the stack); `fsm_isuniversal` unsatisfiable
   condition (implement the evident universality test); `flag_eliminate`
   `|`-for-`&` filter (use `&`; masked in practice); `iface_conc` stray
   "dd" debug print (delete); `iface_print_defined` stray `)` (fix
   format); `iface_show_variable` BOOL-formatter (print by type);
   `iface_random_pairs` wrong limit global (use g_list_random_limit);
   `iface_extract_number` "-5"→5 (parse sign); `sigma_sort`
   uninitialized replacearray (error on absent numbers); lexc
   `#`-numbering collision; `fsm_letter_machine` utf8skip(in) on output
   side; io `check_BOM` NUL false-matches; prolog writer's out-side `?`
   escape typo; `iface_words_file` sticky static applyer;
   `utf8strlen`/`decode_quoted`/`streqrep` non-termination on malformed
   input (return error/lossy instead); buffer-size quirks that panic
   (grow instead).
2. **Keep** — when C compatibility outweighs (wire formats, hash
   functions, tokenization, `fsm_lenient_compose`'s actual `.P. A`
   semantics, sigma numbering conventions): keep behavior, delete the
   obsolete DEVIATION/bug commentary only where it no longer applies,
   and leave the rule unbumped.
3. **Obsolete in Rust** (leaks, double-frees, uninit reads that safe
   Rust already neutralized): prune the DEVIATION comments that
   documented pure memory-management hazards; no rule change.

`exit(1)`-on-overflow stacks, "Implementation pending" stubs
(`fsm_sequentialize`, `fsm_bimachine`) and dead prototypes stay as-is
(honest unimplemented errors now via `FomaError::Unimplemented`, bumped).

### Per-concern gate

`cargo test -p foma` green + `nplan spec uncovered` empty on the target
side + every intended divergence carries a `+N` bump in both the rule
header and the target annotation.
