//! The in-crate client and the standalone IDL clients decide two things
//! independently, and a comment used to be the only thing holding them
//! together:
//!
//! * which accounts the client resolves for itself, and
//! * what a synthesized account-field seed input is called.
//!
//! A program's `cpi` builder and its generated client are used side by side —
//! in the same test file, often on the same instruction — so a disagreement is
//! an API mismatch a user hits, not an internal detail. These tests run both
//! implementations over the same program shape and require the same answer.

use {
    crate::accounts::derive_accounts_inner,
    quasar_idl::{
        codegen::{
            accounts::account_source,
            model::{account_field_seed_inputs, SeedNameForm},
        },
        types::*,
    },
    quote::quote,
};

/// The synthesized seed-input names the in-crate client emits, read out of the
/// expansion: they appear as `pub {name}: {alias}` inside the generated macro.
fn cpi_seed_input_names(expansion: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (index, _) in expansion.match_indices("__QuasarSeedInput") {
        // The alias follows the field it types: `pub <name> : __QuasarSeedInput..`.
        let head = &expansion[..index];
        let Some(colon) = head.rfind(':') else {
            continue;
        };
        let name: String = head[..colon]
            .trim_end()
            .chars()
            .rev()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names.sort();
    names
}

/// The same names, as the IDL clients would spell them.
fn idl_seed_input_names(ix: &IdlInstruction) -> Vec<String> {
    let mut names: Vec<String> = account_field_seed_inputs(ix)
        .iter()
        .map(|seed| {
            let field = seed.field.replace('.', "_");
            match seed.form {
                SeedNameForm::Field => field,
                SeedNameForm::BaseField => format!("{}_{}", seed.path, field),
                SeedNameForm::BaseFieldSeed => format!("{}_{}_seed", seed.path, field),
            }
        })
        .collect();
    names.sort();
    names
}

fn account(name: &str, resolver: IdlResolver) -> IdlAccountNode {
    IdlAccountNode {
        name: name.to_owned(),
        optional: false,
        writable: AccountFlag::Fixed(false),
        signer: AccountFlag::Fixed(false),
        resolver,
        docs: Vec::new(),
    }
}

/// `escrow`'s address is seeded by a field it stores, so both clients have to
/// synthesize an input for that value — and agree on its name.
fn stored_seed_instruction() -> IdlInstruction {
    IdlInstruction {
        name: "take".to_owned(),
        discriminator: vec![0],
        docs: Vec::new(),
        accounts: vec![
            account("taker", IdlResolver::Input {}),
            account(
                "escrow",
                IdlResolver::Pda {
                    program: IdlPdaProgram::ProgramId {},
                    seeds: vec![IdlPdaSeed::AccountField {
                        path: "escrow".to_owned(),
                        account: "Escrow".to_owned(),
                        field: "seed".to_owned(),
                    }],
                },
            ),
        ],
        args: Vec::new(),
        layout: None,
        remaining_accounts: None,
    }
}

#[test]
fn seed_input_names_match_the_idl_clients() {
    let expansion = derive_accounts_inner(quote! {
        pub struct Take {
            pub taker: Signer,
            #[account(address = Escrow::seeds(escrow.seed))]
            pub escrow: Account<Escrow>,
        }
    })
    .to_string();

    let cpi = cpi_seed_input_names(&expansion);
    let idl = idl_seed_input_names(&stored_seed_instruction());

    assert!(
        !idl.is_empty(),
        "fixture must synthesize a seed input for this test to mean anything"
    );
    assert_eq!(
        cpi, idl,
        "the in-crate client and the IDL clients must name a synthesized seed input the same"
    );
}

/// A second stored-seed account escalates both rules past the bare form; the
/// escalation has to happen in both, or the two clients diverge exactly where
/// collisions make it hardest to notice.
#[test]
fn colliding_seed_inputs_escalate_in_both() {
    let mut ix = stored_seed_instruction();
    ix.accounts.push(account(
        "mirror",
        IdlResolver::Pda {
            program: IdlPdaProgram::ProgramId {},
            seeds: vec![IdlPdaSeed::AccountField {
                path: "mirror".to_owned(),
                account: "Escrow".to_owned(),
                field: "seed".to_owned(),
            }],
        },
    ));

    let expansion = derive_accounts_inner(quote! {
        pub struct Take {
            pub taker: Signer,
            #[account(address = Escrow::seeds(escrow.seed))]
            pub escrow: Account<Escrow>,
            #[account(address = Escrow::seeds(mirror.seed))]
            pub mirror: Account<Escrow>,
        }
    })
    .to_string();

    let idl = idl_seed_input_names(&ix);
    assert!(
        idl.iter().all(|name| name != "seed"),
        "two accounts contributing `seed` must escalate past the bare form: {idl:?}"
    );
    assert_eq!(
        cpi_seed_input_names(&expansion),
        idl,
        "colliding seed inputs must escalate identically in both clients"
    );
}

/// Whether the client resolves an address itself is the other decision made
/// twice. The IDL side is structural; the in-crate side additionally requires
/// the derivation to be expressible. Where they disagree the same program gets
/// two clients with different inputs, so pin the agreement.
#[test]
fn derived_accounts_match_the_idl_clients() {
    let expansion = derive_accounts_inner(quote! {
        pub struct Update {
            pub authority: Signer,
            #[account(address = UserAccount::seeds(authority.address()))]
            pub user: Account<UserAccount>,
        }
    })
    .to_string();

    let ix = IdlInstruction {
        name: "update".to_owned(),
        discriminator: vec![0],
        docs: Vec::new(),
        accounts: vec![
            account("authority", IdlResolver::Input {}),
            account(
                "user",
                IdlResolver::Pda {
                    program: IdlPdaProgram::ProgramId {},
                    seeds: vec![IdlPdaSeed::Account {
                        path: "authority".to_owned(),
                    }],
                },
            ),
        ],
        args: Vec::new(),
        layout: None,
        remaining_accounts: None,
    };

    let idl_derives_user = ix
        .accounts
        .iter()
        .find(|node| node.name == "user")
        .map(|node| {
            account_source(node)
                .expect("validated resolver")
                .is_derived()
        })
        .expect("user account");
    // The in-crate client derives an address when it emits the recipe call
    // rather than reading the field off the builder.
    let cpi_derives_user = expansion.contains("__quasar_pda_user");

    assert!(idl_derives_user, "IDL side must consider `user` derived");
    assert_eq!(
        cpi_derives_user, idl_derives_user,
        "the in-crate client and the IDL clients must agree on which accounts they resolve"
    );
}
