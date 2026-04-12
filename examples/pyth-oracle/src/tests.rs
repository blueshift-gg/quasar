extern crate std;
use {
    crate::pyth::{DISCRIMINATOR, PYTH_RECEIVER_PROGRAM, SOL_USD_FEED},
    quasar_pyth_oracle_client::*,
    quasar_svm::{Account, Instruction, Pubkey, QuasarSvm},
    std::{println, vec},
};

fn setup() -> QuasarSvm {
    let elf = std::fs::read("../../target/deploy/quasar_pyth_oracle.so").unwrap();
    QuasarSvm::new().with_program(&crate::ID, &elf)
}

fn signer(address: Pubkey) -> Account {
    quasar_svm::token::create_keyed_system_account(&address, 1_000_000_000)
}

fn pyth_price_account(
    address: Pubkey,
    feed_id: &[u8; 32],
    price: i64,
    exponent: i32,
    publish_time: i64,
) -> Account {
    let mut data = vec![0u8; 134];
    data[0..8].copy_from_slice(&DISCRIMINATOR);
    data[40] = 0; // Partial verification
    let o = 42;
    data[o..o + 32].copy_from_slice(feed_id);
    data[o + 32..o + 40].copy_from_slice(&price.to_le_bytes());
    data[o + 48..o + 52].copy_from_slice(&exponent.to_le_bytes());
    data[o + 52..o + 60].copy_from_slice(&publish_time.to_le_bytes());
    Account {
        address,
        lamports: 1_000_000,
        data,
        owner: PYTH_RECEIVER_PROGRAM,
        executable: false,
    }
}

#[test]
fn test_check_price_cu() {
    let mut svm = setup();

    let user = Pubkey::new_unique();
    let price_feed = Pubkey::new_unique();
    let clock = quasar_svm::solana_sdk_ids::sysvar::clock::ID;

    let instruction: Instruction = CheckPriceInstruction {
        user,
        price_feed,
        clock,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(user),
            pyth_price_account(price_feed, &SOL_USD_FEED, 15_000_000_000, -8, 0),
        ],
    );

    assert!(
        result.is_ok(),
        "check_price failed: {:?}",
        result.raw_result
    );
    println!("  CHECK_PRICE CU: {}", result.compute_units_consumed);
}

// Fetches the real SOL/USD price feed from mainnet and parses it.
// Run with: cargo test -p quasar-pyth-oracle -- --ignored --nocapture
#[test]
#[ignore]
fn test_live_mainnet_sol_usd() {
    use {
        crate::pyth::PythPrice,
        std::{eprintln, process::Command},
    };

    let output = Command::new("curl")
        .args([
            "-s",
            "https://api.mainnet-beta.solana.com",
            "-X", "POST",
            "-H", "Content-Type: application/json",
            "-d", r#"{"jsonrpc":"2.0","id":1,"method":"getAccountInfo","params":["7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE",{"encoding":"base64"}]}"#,
        ])
        .output()
        .expect("curl failed");

    let body = std::string::String::from_utf8(output.stdout).unwrap();
    let marker = r#""data":[""#;
    let start = body.find(marker).expect("no data in response") + marker.len();
    let end = start + body[start..].find('"').unwrap();
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD.decode(&body[start..end]).expect("bad base64");

    let price = PythPrice::from_bytes(&bytes).expect("failed to parse PriceUpdateV2");

    assert_eq!(price.feed_id, SOL_USD_FEED);
    assert_eq!(price.exponent, -8);
    assert!(price.price > 0);

    let usd = price.price as f64 * 10f64.powi(price.exponent);
    assert!(usd > 1.0 && usd < 10_000.0);

    eprintln!("  SOL/USD: ${:.4}", usd);
}
