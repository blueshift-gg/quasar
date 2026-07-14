use {
    crate::helpers::mollusk_for_program, mollusk_svm::Mollusk, quasar_test_misc::cpi::*,
    solana_account::Account, solana_address::Address, solana_instruction::Instruction,
};

const TWO_DYN_ARGS_DISC: u8 = 22;
// disc(1) + tag u64(8) + a_len u8(1) + a u64(8) + b_len u8(1) + b u64(8)
const TWO_DYN_ARGS_SIZE: usize = 1 + 8 + 1 + 8 + 1 + 8;

fn setup() -> Mollusk {
    mollusk_for_program(&quasar_test_misc::ID, "quasar_test_misc")
}

fn scratch_account() -> (Address, Account) {
    let address = Address::new_unique();
    let mut data = vec![0u8; TWO_DYN_ARGS_SIZE];
    data[0] = TWO_DYN_ARGS_DISC;
    (
        address,
        Account {
            lamports: 1_000_000,
            data,
            owner: quasar_test_misc::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
}

/// With two dynamic args the generated client emits the compact wire layout
/// `[tag][a_len][b_len][a][b]`. Before A1 the `#[derive(Accounts)]` extractor
/// walked an interleaved layout (`[tag][a_len][a][b_len][b]`), so it read `b`'s
/// length prefix out of `a`'s payload and rejected the instruction. The
/// accounts-side constraint and the handler must both agree with the client:
/// layout-discriminating payloads (`a = "aa"`, `b = "zz"`) round-trip only when
/// every decoder reads the same bytes.
#[test]
fn two_dynamic_args_decode_matches_client() {
    let mollusk = setup();
    let (account, account_data) = scratch_account();

    let ix: Instruction = TwoDynInstruction {
        account,
        tag: 7,
        a: quasar_lang::client::DynString::from("aa"),
        b: quasar_lang::client::DynString::from("zz"),
    }
    .into();

    let result = mollusk.process_instruction(&ix, &[(account, account_data)]);
    assert!(
        result.program_result.is_ok(),
        "two dynamic args must decode via the shared compact layout: {:?}",
        result.program_result
    );

    // The handler packs the decoded `a`/`b` bytes into the account; reading them
    // back proves the on-chain decode saw exactly what the client emitted.
    let stored = &result.resulting_accounts[0].1.data;
    assert_eq!(stored[0], TWO_DYN_ARGS_DISC, "discriminator");
    assert_eq!(&stored[1..9], &7u64.to_le_bytes(), "tag");
    assert_eq!(stored[9], 2, "a_len");
    assert_eq!(&stored[10..12], b"aa", "a bytes");
    assert_eq!(stored[18], 2, "b_len");
    assert_eq!(&stored[19..21], b"zz", "b bytes");
}
