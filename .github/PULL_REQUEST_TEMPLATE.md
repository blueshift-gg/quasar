**Quasar is not accepting pull requests during beta.**

The API is unstable and the internals are performance-critical — small changes can have outsized CU impact. Please open an issue instead:

- **Bug?** → [Open a bug report](https://github.com/blueshift-gg/quasar/issues/new?template=bug.yml)
- **Feature idea?** → [Open a feature request](https://github.com/blueshift-gg/quasar/issues/new?template=feature.yml)

See [CONTRIBUTING.md](../CONTRIBUTING.md) for details. This PR will be closed.

---

## Maintainer checklist

<!-- What does this PR do, and why? Reference the issue it closes.
     Every box below maps to an enforced check; tick what applies, delete
     what doesn't. "The check obviously works" is not an argument — the
     matrix and the mutation baseline are (TESTING.md). -->

- [ ] New/changed feature has its `tests/feature-matrix.tsv` row and every
      required cell filled (`make check-test-matrix` — enforced in CI)
- [ ] Every new rejection path asserts the exact error via a named constant
      (`make check-suite-oracles` — enforced in CI)
- [ ] Bug fix ships a regression test that fails before the fix and
      references the issue in its name or a comment
- [ ] Intentional diagnostic/codegen/IDL/client changes: goldens regenerated
      with the bless targets and every diff hunk reviewed like code
      (CONTRIBUTING.md; never blessed blindly)
- [ ] No new `println!` in test code (`make check-test-silence` — enforced
      in CI); CU-relevant changes checked against `make compare-tracked`
- [ ] Ratchets only shrink: any addition to `.ci/mutants-baseline.txt` or
      the oracle allowlist is justified in the description
- [ ] Compatibility impact stated for any published Rust item, macro
      expansion, IDL field, wire layout, or generated client change, with
      the relevant baseline diff (VERSIONING.md)
