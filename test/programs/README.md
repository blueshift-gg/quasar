# Classic SPL Token fixture

`spl_token.so` is copied unchanged from `src/elf/token.so` in
[`mollusk-svm-programs-token` 0.10.3](https://crates.io/crates/mollusk-svm-programs-token/0.10.3).
The source distribution is pinned at Mollusk commit
`9746e75dd5b6c9beb96e3c794dcf49b0bf902b6e` (`programs/token`).
Its Apache-2.0 license is included in `LICENSE`.

SHA-256: `7b44674b9704fa7640fbebfac68389f338b500b8c2bc29175c27ed66838b5a3c`

This fixture preserves classic SPL Token coverage. Later Mollusk releases bundle
p-token at the same Tokenkeg address. Benchmarks use this ELF for Escrow and AMM;
the separate p-token benchmark builds its pinned revision through `bench/rivals.sh`.
