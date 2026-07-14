//! Resolution of the `quasar-lang` crate path for generated code.
//!
//! Generated code must reference the runtime crate by the name the *consumer*
//! gave it in their `Cargo.toml`, which may be a rename (for example
//! `ql = { package = "quasar-lang" }`). [`lang_path`] resolves that name via
//! `proc-macro-crate` and returns the token path every emitter interpolates as
//! `#krate`.
//!
//! This module is the ONLY place a literal `quasar_lang` path may appear inside
//! a `quote!` body; `derive/tests/deny_lang_path.rs` enforces that boundary so
//! a rename can never be silently defeated by a hard-coded path elsewhere.

use {
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    std::cell::RefCell,
};

/// Resolution of the runtime crate, cached per proc-macro process.
#[derive(Clone)]
enum Resolved {
    /// The consumer *is* `quasar-lang` (its own macro-expanded code).
    Itself,
    /// The consumer depends on `quasar-lang` under this (possibly renamed)
    /// name.
    Named(String),
    /// Resolution failed (docs.rs, vendored trees) — assume the canonical name.
    Fallback,
}

thread_local! {
    // `crate_name` reads `Cargo.toml`; the answer is constant within a single
    // compilation, so resolve once per process and reuse it for every emission.
    static LANG: RefCell<Option<Resolved>> = const { RefCell::new(None) };
}

fn resolve() -> Resolved {
    use proc_macro_crate::{crate_name, FoundCrate};
    match crate_name("quasar-lang") {
        Ok(FoundCrate::Itself) => Resolved::Itself,
        Ok(FoundCrate::Name(name)) => Resolved::Named(name),
        Err(_) => Resolved::Fallback,
    }
}

/// Token path to the `quasar-lang` runtime crate, honoring consumer renames.
///
/// - `FoundCrate::Itself` -> `crate` (quasar-lang's own macro-expanded code).
/// - `FoundCrate::Name` -> `::<name>` (the consumer's dependency name, renamed
///   or not).
/// - resolution error (docs.rs, vendored trees) -> `::quasar_lang` fallback.
///
/// Emitters bind the result once (`let krate = crate::krate::lang_path();`) and
/// interpolate it as `#krate` everywhere a runtime item is referenced.
pub(crate) fn lang_path() -> TokenStream {
    let resolved = LANG.with(|slot| slot.borrow_mut().get_or_insert_with(resolve).clone());
    match resolved {
        Resolved::Itself => quote!(crate),
        Resolved::Named(name) => {
            let ident = format_ident!("{name}");
            quote!(::#ident)
        }
        Resolved::Fallback => quote!(::quasar_lang),
    }
}
