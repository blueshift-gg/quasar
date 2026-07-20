# Architecture

Quasar 0.1.0 is organized around one product journey:

```text
install -> init -> build -> test -> deploy -> verify -> debug
```

The repository keeps product boundaries small even when packaging requires
supporting crates.

## Runtime products

`quasar-lang` owns the program model, account views, validation traits,
zero-copy contracts, dispatch, CPI primitives, and user-facing macros through
its derive dependency. `quasar-spl` implements SPL-specific account behavior
and CPI helpers through those extension points. The derive must not contain
token-program product policy that can be expressed structurally by SPL types.

`quasar-test` is the Rust testing product. It owns project-aware QuasarSVM
fixtures, instruction/account assertions, and testing macros. JavaScript client
generation is independent from the testing product.

## IDL and clients

The program model flows through one typed IDL/codegen representation. IDL wire
behavior and ABI hashing are stable. Rust generation is part of the
program/IDL build path. Kit and Web3 are separate stable targets, allowing
their dependencies, compilation, and failures to be tested independently.

Python, Go, and C consume the same model as explicit preview targets. Shared
model changes exercise every backend; backend-only changes exercise their
owner.

Compatibility fixtures live with the derive or IDL implementation that owns
the contract. Root-level duplicates are intentionally avoided.

## CLI

The CLI is an executable product, not a Rust library framework. It exposes one
narrow entrypoint to `main.rs`; command modules and argument structs remain
private.

`Quasar.toml` is represented by one typed `Serialize + Deserialize` model.
Commands are structured `CommandSpec` values and client targets are a closed
enum. Unknown fields and removed values fail with a supported replacement;
the CLI does not rewrite pre-release configuration.

The stable top-level command set owns the primary journey. Validation-plan and
assembly inspection are grouped below `inspect` and labeled preview. Profiling
is stable CLI behavior whose implementation lives inside the CLI; sharing and
diff-server behavior is preview.

## Release graph

Cargo metadata is the only package inventory. A non-publishable Rust helper
derives publishable members, exact internal pins, dependency edges, topological
tiers, package archives, and publication order. CI, Docker rehearsal, and
release publishing consume its machine-readable manifest rather than
maintaining crate lists or numeric archive counts.

The package rehearsal is the strongest integration test: it installs immutable
archives in a clean Linux environment with no source checkout or credentials,
then runs the same init-to-verification journey users run.

## Assurance

Tests are located by failure owner. Compiler diagnostics belong to derives,
wire and client fixtures to IDL, semantic validation to host/SBF suites, and
undefined-behavior questions to focused Miri/Kani/fuzz targets.

CI jobs are named for promises: Rust quality, core host behavior, SBF behavior,
contracts, package journey, dependency safety, unsafe-boundary assurance, and
performance. Scheduled coverage, fuzz soaks, and full proofs deepen assurance
without turning preview or research machinery into additional products.
