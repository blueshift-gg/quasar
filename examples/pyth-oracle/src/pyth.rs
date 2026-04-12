// Pyth PriceUpdateV2 reader.
//
// PriceUpdateV2 layout: https://github.com/pyth-network/pyth-crosschain/blob/dac6a0b0ed8816d96b44b7793f99100a9b7c7caa/target_chains/solana/pyth_solana_receiver_sdk/src/price_update.rs
// PriceFeedMessage fields: https://docs.rs/pythnet-sdk/2.4.1/pythnet_sdk/messages/struct.PriceFeedMessage.html

use {crate::errors::PythError, quasar_lang::prelude::*};

pub const PYTH_RECEIVER_PROGRAM: Address = address!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");

pub const SOL_USD_FEED: [u8; 32] = [
    0xef, 0x0d, 0x8b, 0x6f, 0xda, 0x2c, 0xeb, 0xa4, 0x1d, 0xa1, 0x5d, 0x40, 0x95, 0xd1, 0xda, 0x39,
    0x2a, 0x0d, 0x2f, 0x8e, 0xd0, 0xc6, 0xc7, 0xbc, 0x0f, 0x4c, 0xfa, 0xc8, 0xc2, 0x80, 0xb5, 0x6d,
];

// SHA256("account:PriceUpdateV2")[..8]
pub const DISCRIMINATOR: [u8; 8] = [0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd];

const MIN_LEN: usize = 134;

// VerificationLevel is a Borsh enum at byte 40.
// Partial(u8) serializes as tag + 1 byte payload = 2 bytes.
// Full serializes as tag only = 1 byte.
// See: https://github.com/pyth-network/pyth-crosschain/blob/dac6a0b0ed8816d96b44b7793f99100a9b7c7caa/target_chains/solana/pyth_solana_receiver_sdk/src/price_update.rs
#[inline(always)]
fn msg_offset(data: &[u8]) -> usize {
    40 + if data[40] == 0 { 2 } else { 1 }
}

#[derive(Debug, Clone, Copy)]
pub struct PythPrice {
    pub feed_id: [u8; 32],
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
    pub ema_price: i64,
}

impl PythPrice {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < MIN_LEN {
            return Err(PythError::AccountTooSmall.into());
        }
        if data[0..8] != DISCRIMINATOR {
            return Err(PythError::InvalidDiscriminator.into());
        }
        Self::parse(data)
    }

    pub fn from_account(view: &AccountView) -> Result<Self, ProgramError> {
        if *view.owner() != PYTH_RECEIVER_PROGRAM {
            return Err(PythError::InvalidOwner.into());
        }
        let data = unsafe { view.borrow_unchecked() };
        Self::from_bytes(data)
    }

    fn parse(data: &[u8]) -> Result<Self, ProgramError> {
        let o = msg_offset(data);
        let mut feed_id = [0u8; 32];
        feed_id.copy_from_slice(&data[o..o + 32]);

        Ok(Self {
            feed_id,
            price: read_i64(data, o + 32),
            conf: read_u64(data, o + 40),
            exponent: read_i32(data, o + 48),
            publish_time: read_i64(data, o + 52),
            ema_price: read_i64(data, o + 68),
        })
    }

    pub fn validate(
        &self,
        expected_feed: &[u8; 32],
        max_age: u64,
        clock_ts: i64,
    ) -> Result<(), ProgramError> {
        if self.feed_id != *expected_feed {
            return Err(PythError::FeedMismatch.into());
        }
        if max_age > 0 {
            let age = clock_ts.saturating_sub(self.publish_time);
            if age < 0 || age as u64 > max_age {
                return Err(PythError::StalePrice.into());
            }
        }
        Ok(())
    }
}

#[inline(always)]
fn read_i64(data: &[u8], off: usize) -> i64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[off..off + 8]);
    i64::from_le_bytes(buf)
}

#[inline(always)]
fn read_u64(data: &[u8], off: usize) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data[off..off + 8]);
    u64::from_le_bytes(buf)
}

#[inline(always)]
fn read_i32(data: &[u8], off: usize) -> i32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(&data[off..off + 4]);
    i32::from_le_bytes(buf)
}
