**Quasar is not accepting pull requests during beta.**

The API is unstable and the internals are performance-critical — small changes can have outsized CU impact. Please open an issue instead:

- **Bug?** → [Open a bug report](https://github.com/blueshift-gg/quasar/issues/new?template=bug.yml)
- **Feature idea?** → [Open a feature request](https://github.com/blueshift-gg/quasar/issues/new?template=feature.yml)

See [CONTRIBUTING.md](../CONTRIBUTING.md) for details. This PR will be closed.

---

## Maintainer checklist

<!-- What does this PR do, and why? Reference the issue it closes.
     Tick what applies and delete what does not. -->

- [ ] Every new rejection path asserts the exact typed error or diagnostic
- [ ] Bug fix ships a regression test that fails before the fix and
      references the issue in its name or a comment
- [ ] Intentional diagnostic/codegen/IDL/client changes: goldens regenerated
      through their owning crate and every diff hunk reviewed like code
      (CONTRIBUTING.md; never blessed blindly)
- [ ] CU-relevant changes checked against `make compare-tracked`
- [ ] Unsafe changes explain their local safety contract and add a targeted
      Miri, Kani, or fuzz case when that tool owns a unique failure mode
- [ ] Compatibility impact stated for a stable Rust item, macro expansion,
      IDL field, wire layout, or generated client change, with the relevant
      owner-local fixture diff (VERSIONING.md)
