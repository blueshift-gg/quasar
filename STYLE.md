# Code style

How runtime code is written here — the unsafe core especially. The lineage is
[Pinocchio](https://github.com/anza-xyz/pinocchio): code that reads as if it
were effortless, because every invariant is established exactly once and then
trusted. TESTING.md owns test style; this document owns the code.

The mechanically checkable subset is enforced by `make check-unsafe-policy`.
Everything else is review judgment, and reviewers hold changes to it.

## The feel

- **One altitude per function.** A function reads top-to-bottom as a single
  narrative. When the abstraction level must change, it changes file or
  function, not paragraph.
- **Comments carry intent and invariants; code carries mechanics.** Never
  swap them. A comment that restates its line is deleted on sight. When
  nothing needs saying, nothing is said.
- **Names encode the invariant.** `to_process_plus_one`, `DUP_ENTRY_SIZE`,
  `MAX_SEEDS_WITH_BUMP` — when names carry the design choice, comments are
  freed to carry only judgment.
- **Zero defensiveness.** Establish an invariant once — a const, a
  `const { assert!() }`, a module header, a `# Safety` contract — then trust
  it without re-checking or re-arguing. No speculative generality, no trait
  indirection "just in case".
- **Visual calm.** Early returns over else-ladders, shallow nesting, one
  concept per file, a consistent vertical rhythm of doc / const / struct /
  impl.

## Unsafe discipline

- **Every `unsafe` block, `unsafe impl`, and `unsafe trait` carries a
  `// SAFETY:` comment** naming, in one to three lines, the concrete
  precondition or invariant that makes it sound — the validated length, the
  runtime layout, the guarding branch. Never what the code does.
- **Every `unsafe fn` carries a `# Safety` doc section** stating caller
  obligations in prose ("The caller must ensure …"). `# Safety` headings are
  reserved for unsafe fns.
- **Established invariants are referenced, not re-argued.** Once the module
  header or type contract states the trust model, later SAFETY comments cite
  it tersely: `// SAFETY: index < len, checked by cache_has_capacity above.`
- **`unsafe fn` means a memory/aliasing contract.** A function that merely
  skips a semantic check (overflow validation, a compile-time budget) stays
  safe; if it is named `_unchecked`, its doc says in one line which check is
  skipped and that the function is safe.
- **Modules that construct wrappers by pointer cast state the trust model in
  their `//!` header**: when validation runs, what layout guarantee
  (`#[repr(transparent)]`, `StaticView`) later access relies on.

## Layout and constants

- **No magic numbers in layout code.** Offsets and sizes come from
  `size_of::<T>()` chains and named consts with one-line docs; relations
  between consts are derived, not restated (`MAX_PDA_SLICES =
  MAX_SEEDS_NO_BUMP + 2`).
- **Layout assumptions are compile-time asserted** (`const _: () =
  assert!(...)`, `offset_of!` cross-checks) so documentation and layout
  cannot silently drift.
- **Wire formats are first-class documentation.** Modules that parse a
  serialized format diagram it in the `//!` header. Every CPI builder
  documents `### Accounts:` (numbered, `[WRITE]`/`[SIGNER]` tags) and
  `### Instruction data (N bytes):` as a byte map derived from the code.

## Compute-driven choices

CU is the currency; these are deliberate and exempt from DRY instincts:

- `#[inline(always)]` on hot accessors and builders; `#[inline]` on large
  derivation functions (forced inlining bloats the `.so`); `#[cold]`
  (+ `#[inline(never)]`) free functions for error construction;
  `unlikely()` on rare guards; bitwise `|` over `&&` for independent cheap
  checks.
- **Duplication is correct where abstraction costs branches.** Fixed-layout
  serializers are written out per instruction. When identical logic must be
  shared without cost, use a private `#[inline(always)]` helper with a
  monomorphized closure (`pda::search_bump`): one source of truth, same
  straight-line code per caller.
- Off-chain-only code takes no inline annotations — they are noise there.

## Change discipline

- A comment whose claim you cannot verify against the code is a bug: fix the
  claim or fix the code, never leave it.
- Behavior-neutral refactors of the unsafe core are verified like behavior:
  full gate plus the tracked CU/size comparison.
- `#[allow]` attributes carry a trailing reason.
