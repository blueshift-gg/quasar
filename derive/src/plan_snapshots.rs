//! Plan-IR dump snapshots (Workstream B3).
//!
//! For each `#[derive(Accounts)]` fixture, lower the fields to `FieldSemantics`
//! and build the typed `AccountsPlanTyped`, then snapshot the compact dump of
//! both IRs. This is the audit surface the accounts-IR workstream re-blesses:
//! a reviewer reads the dump and sees every structural fact and every
//! phase-ordered step, without touching generated tokens. A dump diff is a
//! resolve/plan change; regenerate with `UPDATE_EXPECT=1` and review each hunk.

use {
    crate::accounts::{
        parse_struct_instruction_args,
        resolve::{
            dump::{dump_plan, dump_semantics},
            lower_semantics,
            planner::build_plan,
        },
    },
    quote::quote,
    syn::{Data, DeriveInput, Fields},
};

// See `snapshot_tests`: relative expect-test paths otherwise resolve against
// the primary checkout when Cargo runs this crate from a linked worktree.
mod expect_test {
    macro_rules! expect_file {
        ($path:literal) => {
            ::expect_test::expect_file![concat!(env!("CARGO_MANIFEST_DIR"), "/src/", $path)]
        };
    }

    pub(crate) use expect_file;
}

/// Lower + plan a fixture and render both IRs into one dump. Instruction args
/// are threaded exactly as the real derive does, so typed-seed classification
/// matches production.
fn dump_ir(input: proc_macro2::TokenStream) -> String {
    let derive_input: DeriveInput = syn::parse2(input).expect("fixture is a valid struct");
    let ix_args = parse_struct_instruction_args(&derive_input)
        .expect("instruction args parse")
        .unwrap_or_default();
    let fields = match derive_input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(named) => named.named,
            _ => Default::default(),
        },
        _ => Default::default(),
    };
    let sems = lower_semantics(&fields, &ix_args).expect("lower_semantics succeeds");
    let plan = build_plan(&sems, &ix_args, !ix_args.is_empty()).expect("build_plan succeeds");
    format!("{}\n{}", dump_semantics(&sems), dump_plan(&plan))
}

#[test]
fn plan_basic_mut_signer() {
    let input = quote! {
        pub struct BasicAccounts {
            #[account(mut)]
            pub payer: Signer,
            pub config: Account<TestConfig>,
            pub system_program: Program<SystemProgram>,
        }
    };
    expect_test::expect_file!["snapshots/plan_basic_mut_signer.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_init_payer() {
    let input = quote! {
        pub struct InitEscrow {
            #[account(mut)]
            pub payer: Signer,
            #[account(init, payer = payer, address = Escrow::seeds(payer.address()))]
            pub escrow: Account<Escrow>,
            pub system_program: Program<SystemProgram>,
        }
    };
    expect_test::expect_file!["snapshots/plan_init_payer.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_seed_kinds() {
    // C2 evidence: the seeds expression is wrapped in parentheses (which the old
    // signer-helper detector stripped but the IDL resolver did not — they
    // disagreed). It is now classified once, so both agree, and every `SeedRef`
    // variant is exercised: account address, account field, instruction arg, and
    // an opaque const.
    let input = quote! {
        #[instruction(index: u64)]
        pub struct SeedKinds {
            pub authority: Signer,
            pub config: Account<Config>,
            #[account(address = (Item::seeds(authority.address(), config.namespace.into(), index, SIDE_A)))]
            pub item: Account<Item>,
        }
    };
    expect_test::expect_file!["snapshots/plan_seed_kinds.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_close() {
    let input = quote! {
        pub struct CloseAccounts {
            #[account(mut)]
            pub authority: Signer,
            #[account(mut, close(dest = authority))]
            pub old_data: Account<OldData>,
        }
    };
    expect_test::expect_file!["snapshots/plan_close.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_realloc() {
    let input = quote! {
        pub struct ReallocAccounts {
            #[account(mut)]
            pub payer: Signer,
            #[account(mut, realloc = 200)]
            pub data: Account<MyData>,
            pub system_program: Program<SystemProgram>,
        }
    };
    expect_test::expect_file!["snapshots/plan_realloc.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_realloc_implies_mut() {
    // C4 realloc-implies-mut: `realloc` without an explicit `mut` is now accepted
    // (previously a `realloc = ... requires mut` error). `declared_mut=false` but
    // the derived `writable=true`, and the realloc step is still planned.
    let input = quote! {
        pub struct ReallocNoMut {
            #[account(mut)]
            pub payer: Signer,
            #[account(realloc = 200)]
            pub data: Account<MyData>,
            pub system_program: Program<SystemProgram>,
        }
    };
    expect_test::expect_file!["snapshots/plan_realloc_implies_mut.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_optional() {
    let input = quote! {
        pub struct OptionalAccounts {
            pub authority: Signer,
            #[account(has_one(authority))]
            pub config: Option<Account<Config>>,
        }
    };
    expect_test::expect_file!["snapshots/plan_optional.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_dup() {
    let input = quote! {
        pub struct HeaderDupReadonly {
            pub source: Signer,
            /// CHECK: test-only unchecked account used to validate duplicate readonly aliases.
            #[account(dup)]
            pub destination: UncheckedAccount,
        }
    };
    expect_test::expect_file!["snapshots/plan_dup.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_composite() {
    let input = quote! {
        pub struct UsesAccountArray {
            pub payer: Signer,
            pub pairs: AccountsArray<SignerPair, 2>,
        }
    };
    expect_test::expect_file!["snapshots/plan_composite.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_behavior_group() {
    let input = quote! {
        pub struct UseCustomBehavior {
            #[account(min_value(min = 10u64))]
            pub data: Account<MyData>,
        }
    };
    expect_test::expect_file!["snapshots/plan_behavior_group.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_ix_args_fixed() {
    let input = quote! {
        #[instruction(amount: u64, flag: bool)]
        pub struct IxArgsFixed {
            #[account(mut, constraints(amount > 0 && flag))]
            pub account: Account<SimpleAccount>,
        }
    };
    expect_test::expect_file!["snapshots/plan_ix_args_fixed.txt"].assert_eq(&dump_ir(input));
}

#[test]
fn plan_ix_args_dynamic() {
    let input = quote! {
        #[instruction(tag: u64, a: String<8>, b: String<8>)]
        pub struct TwoDyn {
            #[account(mut, constraints(tag != 0 && a.len() == b.len()))]
            pub account: Account<TwoDynArgsAccount>,
        }
    };
    expect_test::expect_file!["snapshots/plan_ix_args_dynamic.txt"].assert_eq(&dump_ir(input));
}
