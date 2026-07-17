# quasar-profile

Static profiling support for Quasar SBF programs. The library reads program ELF
and DWARF data, attributes estimated compute-unit cost, and produces data for
the Quasar CLI and flamegraph viewer.

The estimates are derived from the compiled instruction graph. They are useful
for deterministic code-generation and binary-size budgets, but they do not
replace transaction runtime CU measurements, which include input-dependent
execution, syscalls, CPI, and validator behavior.

- [Profiling guide](https://quasar-lang.com/docs/profiling/cu-profiler)
- [API documentation](https://docs.rs/quasar-profile/0.1.0)

Licensed under Apache-2.0 or MIT, at your option.
