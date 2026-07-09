//! Internal compiler error (ICE) helper for the derive macros.
//!
//! `ice!(...)` marks a path that is unreachable for any input that has passed
//! the front-end validation upstream of it. If one fires, it is a bug in
//! `quasar-derive`, not a user error — so it panics with a uniform, greppable
//! message rather than an ad-hoc `expect`/`unwrap`. A proc-macro panic surfaces
//! as `proc macro ... panicked: <message>` at the invocation, which is the best
//! available signal for a compiler-internal invariant break.
//!
//! Prefer a spanned `syn::Error` for anything a user can trigger; reserve
//! `ice!` for genuine invariants (e.g. "named struct fields are validated before
//! codegen reaches here").

/// Panic with a uniform internal-error message. See the module docs. Brought
/// into crate-wide scope by `#[macro_use] mod ice;` in `lib.rs`.
macro_rules! ice {
    ($($arg:tt)*) => {
        ::core::panic!(
            "internal error in quasar-derive: {}. This is a bug; please report it.",
            ::core::format_args!($($arg)*)
        )
    };
}
