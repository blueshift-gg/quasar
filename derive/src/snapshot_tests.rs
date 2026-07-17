//! Expansion goldens (Workstream B2).
//!
//! Each test drives one macro's `*_inner` entry point with a fixed input token
//! stream and compares the pretty-printed expansion against a committed golden
//! under the versioned compatibility baseline. These goldens are the reviewable
//! spec of what the compiler emits: a diff here is a codegen change and must be
//! reviewed as one, never blessed blindly (regenerate deliberately with
//! `make bless-proc-macro-baselines`).
//!
//! File-like expansions must parse as `syn::File` and are normalized by
//! `prettyplease`; expression-like expansions have a separate normalizer.

#[cfg(feature = "declare-program")]
use crate::declare_program::declare_program_inner;
use {
    crate::{
        account::account_inner, accounts::derive_accounts_inner, emit_cpi_inner,
        error_code::error_code_inner, event::event_inner, instruction::instruction_inner,
        program::program_inner, seeds::derive_seeds_inner,
        serialize::derive_quasar_serialize_inner,
    },
    quote::quote,
};

// `expect-test` resolves relative paths against Cargo's workspace directory,
// which points at the primary checkout when this crate is tested from a linked
// worktree. Shadow its path macro locally so every golden is anchored to this
// crate's manifest directory instead.
mod expect_test {
    macro_rules! expect_file {
        ($path:literal) => {
            ::expect_test::expect_file![crate::snapshot_tests::baseline_path($path)]
        };
    }

    pub(crate) use expect_file;
}

fn baseline_path(path: &str) -> std::path::PathBuf {
    let root = std::env::var_os("QUASAR_PROC_MACRO_BASELINE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../compatibility-baselines/v0.1.0/proc-macros")
        });
    root.join(path)
}

/// Pretty-print a file-like macro expansion. An invalid expansion is a test
/// failure, never text embedded in the compatibility snapshot.
fn expand_pretty(ts: proc_macro2::TokenStream) -> String {
    let file = syn::parse2::<syn::File>(ts).expect("macro emits a valid Rust file");
    prettyplease::unparse(&file)
}

/// Normalize an expression-like macro expansion without embedding compiler
/// output or source locations in the golden.
fn expand_expression(ts: proc_macro2::TokenStream) -> String {
    let expression = syn::parse2::<syn::Expr>(ts).expect("macro emits a valid Rust expression");
    format!("{}\n", quote! { #expression })
}

// ---------------------------------------------------------------------------
// Function-like macros.
// ---------------------------------------------------------------------------

/// `emit_cpi!` delegates to the typed event CPI implementation on `self`.
#[test]
fn emit_cpi_basic() {
    expect_test::expect_file!["expansions/emit_cpi_basic.rs"]
        .assert_eq(&expand_expression(emit_cpi_inner(quote! { event })));
}

/// `declare_program!` emits a fixed-layout typed CPI module from a checked-in
/// IDL fixture. The feature gate matches the real macro's implementation.
#[cfg(feature = "declare-program")]
#[test]
fn declare_program_fixed_cpi() {
    let fixture = baseline_path("fixtures/external_vault.idl.json");
    let fixture = syn::LitStr::new(
        fixture.to_str().expect("fixture path is valid UTF-8"),
        proc_macro2::Span::call_site(),
    );
    let input = quote! {
        external_vault,
        #fixture
    };
    expect_test::expect_file!["expansions/declare_program_fixed_cpi.rs"]
        .assert_eq(&expand_pretty(declare_program_inner(input)));
}

// ---------------------------------------------------------------------------
// #[derive(Accounts)] — one fixture per high-consequence axis.
// ---------------------------------------------------------------------------

/// Header masks: a `mut` signer, a plain account, and a program field. The
/// writable/signer/executable bits emitted here are what the B2 canary flips.
#[test]
fn accounts_basic_mut_signer() {
    let input = quote! {
        pub struct BasicAccounts {
            #[account(mut)]
            pub payer: Signer,
            pub config: Account<TestConfig>,
            pub system_program: Program<SystemProgram>,
            pub rent: Sysvar<Rent>,
        }
    };
    expect_test::expect_file!["expansions/accounts_basic_mut_signer.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Rent plan: `init` with an explicit `payer` and a typed-seeds address.
#[test]
fn accounts_init_payer() {
    let input = quote! {
        pub struct InitEscrow {
            #[account(mut)]
            pub payer: Signer,
            #[account(init, payer = payer, address = Escrow::seeds(payer.address()))]
            pub escrow: Account<Escrow>,
            pub system_program: Program<SystemProgram>,
        }
    };
    expect_test::expect_file!["expansions/accounts_init_payer.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Close capability: `close(dest = ...)` scheduled at the epilogue.
#[test]
fn accounts_close() {
    let input = quote! {
        pub struct CloseAccounts {
            #[account(mut)]
            pub authority: Signer,
            #[account(mut, close(dest = authority))]
            pub old_data: Account<OldData>,
        }
    };
    expect_test::expect_file!["expansions/accounts_close.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Realloc capability: `realloc = <expr>`.
#[test]
fn accounts_realloc() {
    let input = quote! {
        pub struct ReallocAccounts {
            #[account(mut)]
            pub payer: Signer,
            #[account(mut, realloc = 200)]
            pub data: Account<MyData>,
            pub system_program: Program<SystemProgram>,
        }
    };
    expect_test::expect_file!["expansions/accounts_realloc.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Optional account (`Option<Account<..>>`) carrying a `has_one` user check.
#[test]
fn accounts_optional() {
    let input = quote! {
        pub struct OptionalAccounts {
            pub authority: Signer,
            #[account(has_one(authority))]
            pub config: Option<Account<Config>>,
        }
    };
    expect_test::expect_file!["expansions/accounts_optional.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Duplicate readonly alias (`dup`) on an `UncheckedAccount`, with the
/// conventional `/// CHECK:` justification doc comment.
#[test]
fn accounts_dup() {
    let input = quote! {
        pub struct HeaderDupReadonly {
            pub source: Signer,
            /// CHECK: test-only unchecked account used to validate duplicate readonly aliases.
            #[account(dup)]
            pub destination: UncheckedAccount,
        }
    };
    expect_test::expect_file!["expansions/accounts_dup.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Composite via `AccountsArray<T, N>` — exercises the bounded-group offset
/// math (`fixed + N * T::COUNT`).
#[test]
fn accounts_composite() {
    let input = quote! {
        pub struct UsesAccountArray {
            pub payer: Signer,
            pub pairs: AccountsArray<SignerPair, 2>,
        }
    };
    expect_test::expect_file!["expansions/accounts_composite.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Custom behavior group: builder chain + const-assert scaffold emitted for a
/// plugin behavior (`min_value(min = ..)`).
#[test]
fn accounts_behavior_group() {
    let input = quote! {
        pub struct UseCustomBehavior {
            #[account(min_value(min = 10u64))]
            pub data: Account<MyData>,
        }
    };
    expect_test::expect_file!["expansions/accounts_behavior_group.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Struct-level `#[instruction(...)]` with fixed-only args referenced in a
/// constraint.
#[test]
fn accounts_ix_args_fixed() {
    let input = quote! {
        #[instruction(amount: u64, flag: bool)]
        pub struct IxArgsFixed {
            #[account(mut, constraints(amount > 0 && flag))]
            pub account: Account<SimpleAccount>,
        }
    };
    expect_test::expect_file!["expansions/accounts_ix_args_fixed.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

/// Struct-level `#[instruction(...)]` with TWO dynamic args — snapshots the A1
/// compact-layout extraction fix (the old interleaved walker misread the
/// second dynamic arg's length prefix).
#[test]
fn accounts_ix_args_dynamic() {
    let input = quote! {
        #[instruction(tag: u64, a: String<8>, b: String<8>)]
        pub struct TwoDyn {
            #[account(mut, constraints(tag != 0 && a.len() == b.len()))]
            pub account: Account<TwoDynArgsAccount>,
        }
    };
    expect_test::expect_file!["expansions/accounts_ix_args_dynamic.rs"]
        .assert_eq(&expand_pretty(derive_accounts_inner(input)));
}

// ---------------------------------------------------------------------------
// #[instruction] handler macro.
// ---------------------------------------------------------------------------

/// Handler with a fixed arg (`u64`) and a dynamic arg (`String<8>`): body fn +
/// `__quasar_direct_*` direct-parse fn with the compact decode.
#[test]
fn instruction_fixed_and_dynamic() {
    let attr = quote! {};
    let item = quote! {
        pub fn transfer(ctx: Ctx<Transfer>, amount: u64, memo: String<8>) -> Result<(), ProgramError> {
            ctx.accounts.handler(amount, memo)
        }
    };
    expect_test::expect_file!["expansions/instruction_fixed_and_dynamic.rs"]
        .assert_eq(&expand_pretty(instruction_inner(attr, item)));
}

/// Bounded typed remaining accounts stay raw through argument decoding and
/// become typed immediately before the user body. This avoids generating drop
/// cleanup for instruction-data failures while keeping the typed handler API.
#[test]
fn instruction_bounded_remaining() {
    let attr = quote! {};
    let item = quote! {
        pub fn approve(
            ctx: CtxWithRemaining<Approve, Signer, 10>,
            amount: u64,
        ) -> Result<(), ProgramError> {
            ctx.accounts.handler(amount, ctx.remaining)
        }
    };
    expect_test::expect_file!["expansions/instruction_bounded_remaining.rs"]
        .assert_eq(&expand_pretty(instruction_inner(attr, item)));
}

// ---------------------------------------------------------------------------
// #[program] module macro.
// ---------------------------------------------------------------------------

/// Two-instruction program: dispatch table, EventAuthority PDA, entrypoint.
#[test]
fn program_dispatch_two_ix() {
    let attr = quote! {};
    let item = quote! {
        mod quasar_demo {
            use super::*;

            #[instruction(discriminator = 0)]
            pub fn initialize(ctx: Ctx<Initialize>, amount: u64) -> Result<(), ProgramError> {
                ctx.accounts.handler(amount)
            }

            #[instruction(discriminator = 1)]
            pub fn update(ctx: Ctx<Update>) -> Result<(), ProgramError> {
                ctx.accounts.handler()
            }
        }
    };
    expect_test::expect_file!["expansions/program_dispatch_two_ix.rs"]
        .assert_eq(&expand_pretty(program_inner(attr, item)));
}

// ---------------------------------------------------------------------------
// #[account] type macro.
// ---------------------------------------------------------------------------

/// Fixed-layout account: discriminator + `ZeroPodFixed` companion.
#[test]
fn account_fixed() {
    let attr = quote! { discriminator = 6 };
    let item = quote! {
        pub struct MixedAccount {
            pub authority: Address,
            pub value: u64,
        }
    };
    expect_test::expect_file!["expansions/account_fixed.rs"]
        .assert_eq(&expand_pretty(account_inner(attr, item)));
}

/// Dynamic-layout account: `String<N>` + `Vec<T, N>` compact schema with
/// `set_inner`.
#[test]
fn account_dynamic() {
    let attr = quote! { discriminator = 5, set_inner };
    let item = quote! {
        pub struct DynamicAccount {
            pub name: String<8>,
            pub tags: Vec<Address, 2>,
        }
    };
    expect_test::expect_file!["expansions/account_dynamic.rs"]
        .assert_eq(&expand_pretty(account_inner(attr, item)));
}

// ---------------------------------------------------------------------------
// #[event], #[error_code], #[derive(Seeds)] basics.
// ---------------------------------------------------------------------------

/// Event with a fixed byte discriminator: `emit_log` + IDL fragment.
/// (Non-zero: all-zero event discriminators are rejected at expansion.)
#[test]
fn event_basic() {
    let attr = quote! { discriminator = 1 };
    let item = quote! {
        pub struct MakeEvent {
            pub escrow: Address,
            pub maker: Address,
            pub deposit: u64,
            pub receive: u64,
        }
    };
    expect_test::expect_file!["expansions/event_basic.rs"]
        .assert_eq(&expand_pretty(event_inner(attr, item)));
}

/// Error enum: auto-assigned codes from the 6000 offset + `TryFrom` arms.
#[test]
fn error_code_basic() {
    let attr = quote! {};
    let item = quote! {
        pub enum TestError {
            Unauthorized,
            InvalidAddress,
            CustomConstraint,
        }
    };
    expect_test::expect_file!["expansions/error_code_basic.rs"]
        .assert_eq(&expand_pretty(error_code_inner(attr, item)));
}

/// `#[derive(Seeds)]` on a unit struct with a prefix + `Address` seed.
#[test]
fn seeds_basic() {
    let input = quote! {
        #[seeds(b"vault", authority: Address)]
        pub struct VaultPda;
    };
    expect_test::expect_file!["expansions/seeds_basic.rs"]
        .assert_eq(&expand_pretty(derive_seeds_inner(input)));
}

/// `#[derive(QuasarSerialize)]` on a fixed struct — the instruction-arg
/// serializer path.
#[test]
fn serialize_fixed() {
    let input = quote! {
        pub struct Payload {
            pub amount: u64,
            pub flag: bool,
        }
    };
    expect_test::expect_file!["expansions/serialize_fixed.rs"]
        .assert_eq(&expand_pretty(derive_quasar_serialize_inner(input)));
}
