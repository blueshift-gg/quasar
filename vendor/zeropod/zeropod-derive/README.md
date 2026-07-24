# zeropod-derive

Derive macros for [zeropod](https://crates.io/crates/zeropod).

This crate provides `#[derive(ZeroPod)]` which generates zero-copy companion types, validation logic, and accessor methods for structs and enums.

You should not depend on this crate directly. It is re-exported through `zeropod`:

```toml
[dependencies]
zeropod = "0.1"
```

```rust
use zeropod::ZeroPod;

#[derive(ZeroPod)]
struct MyAccount {
    pub value: u64,
    pub flag: bool,
}
```

## What it generates

### For fixed structs

- `{Name}Zc` — `#[repr(C)]` zero-copy companion with pod field types
- `ZeroPodFixed` impl — `from_bytes()`, `from_bytes_mut()`, `validate()`
- `ZcValidate` impl — per-field validation

### For compact structs (`#[zeropod(compact)]`)

- `{Name}Header` — `#[repr(C)]` header containing fixed fields + length prefixes
- `{Name}Ref` — zero-copy read accessor with tail field methods (returns `&str`, `&[T]`)
- `{Name}Mut` — mutable accessor with `set_{field}()` + `commit()`
- `ZeroPodCompact` impl — `header()`, `validate()`

### For `#[repr(u8)]` enums

- `{Name}Zc` — `#[repr(transparent)]` wrapper over `u8`
- `ZeroPodFixed` impl — validates discriminant is in range
- `is()`, `Display`, `Debug`, `PartialEq` with the native enum

## License

Apache-2.0
