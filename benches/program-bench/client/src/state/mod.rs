pub mod clock_snapshot;
pub mod data;
pub mod dyn_bytes_account;
pub mod dyn_str_account;
pub mod dynamic_account;
pub mod error_test_account;
pub mod rent_calc_snapshot;
pub mod rent_snapshot;
pub mod simple_account;
pub mod space_test_account;
pub mod two_dyn_args_account;

pub use clock_snapshot::*;
pub use data::*;
pub use dyn_bytes_account::*;
pub use dyn_str_account::*;
pub use dynamic_account::*;
pub use error_test_account::*;
pub use rent_calc_snapshot::*;
pub use rent_snapshot::*;
pub use simple_account::*;
pub use space_test_account::*;
pub use two_dyn_args_account::*;

pub enum ProgramAccount {
    ClockSnapshot(ClockSnapshot),
    Data(Data),
    DynBytesAccount(DynBytesAccount),
    DynStrAccount(DynStrAccount),
    DynamicAccount(DynamicAccount),
    ErrorTestAccount(ErrorTestAccount),
    RentCalcSnapshot(RentCalcSnapshot),
    RentSnapshot(RentSnapshot),
    SimpleAccount(SimpleAccount),
    SpaceTestAccount(SpaceTestAccount),
    TwoDynArgsAccount(TwoDynArgsAccount),
}

pub fn decode_account(data: &[u8]) -> Option<ProgramAccount> {
    if data.starts_with(CLOCK_SNAPSHOT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<ClockSnapshot>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::ClockSnapshot(value));
    }
    if data.starts_with(DATA_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<Data>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::Data(value));
    }
    if data.starts_with(DYN_BYTES_ACCOUNT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<DynBytesAccount>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::DynBytesAccount(value));
    }
    if data.starts_with(DYN_STR_ACCOUNT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<DynStrAccount>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::DynStrAccount(value));
    }
    if data.starts_with(DYNAMIC_ACCOUNT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<DynamicAccount>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::DynamicAccount(value));
    }
    if data.starts_with(ERROR_TEST_ACCOUNT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<ErrorTestAccount>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::ErrorTestAccount(value));
    }
    if data.starts_with(RENT_CALC_SNAPSHOT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<RentCalcSnapshot>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::RentCalcSnapshot(value));
    }
    if data.starts_with(RENT_SNAPSHOT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<RentSnapshot>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::RentSnapshot(value));
    }
    if data.starts_with(SIMPLE_ACCOUNT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<SimpleAccount>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::SimpleAccount(value));
    }
    if data.starts_with(SPACE_TEST_ACCOUNT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<SpaceTestAccount>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::SpaceTestAccount(value));
    }
    if data.starts_with(TWO_DYN_ARGS_ACCOUNT_ACCOUNT_DISCRIMINATOR) {
        let value = wincode::deserialize::<TwoDynArgsAccount>(data).ok()?;
        if usize::try_from(wincode::serialized_size(&value).ok()?).ok()? != data.len() { return None; }
        return Some(ProgramAccount::TwoDynArgsAccount(value));
    }
    None
}
