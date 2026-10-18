//! Bounded Borsh encoding; all writes go through checked slices.
use quasar_lang::{Address, ProgramError, Result};

/// The basic DataV2 fields. These builders encode creators, collection and
/// uses as `None`, matching the original integration.
#[derive(Clone, Copy)]
pub struct Data<'a> {
    pub name: &'a str,
    pub symbol: &'a str,
    pub uri: &'a str,
    pub seller_fee_basis_points: u16,
}

pub(crate) struct Encoded {
    pub bytes: [u8; 512],
    pub len: usize,
}

impl Encoded {
    fn new(discriminator: u8) -> Self {
        let mut bytes = [0; 512];
        bytes[0] = discriminator;
        Self { bytes, len: 1 }
    }

    fn push(&mut self, bytes: &[u8]) -> Result {
        let end = self.len.checked_add(bytes.len()).ok_or(ProgramError::InvalidInstructionData)?;
        self.bytes
            .get_mut(self.len..end)
            .ok_or(ProgramError::InvalidInstructionData)?
            .copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }

    fn string(&mut self, text: &str, max: usize) -> Result {
        if text.len() > max {
            return Err(ProgramError::InvalidInstructionData);
        }
        self.push(&(text.len() as u32).to_le_bytes())?;
        self.push(text.as_bytes())
    }

    fn data(&mut self, data: Data<'_>) -> Result {
        self.string(data.name, 32)?;
        self.string(data.symbol, 10)?;
        self.string(data.uri, 200)?;
        if data.seller_fee_basis_points > 10_000 {
            return Err(ProgramError::InvalidInstructionData);
        }
        self.push(&data.seller_fee_basis_points.to_le_bytes())?;
        self.push(&[0, 0, 0])
    }

    fn boolean(&mut self, value: Option<bool>) -> Result {
        self.push(&[u8::from(value.is_some())])?;
        if let Some(value) = value {
            self.push(&[u8::from(value)])?;
        }
        Ok(())
    }
}

pub(crate) fn create(data: Data<'_>, is_mutable: bool) -> Result<Encoded> {
    let mut out = Encoded::new(33);
    out.data(data)?;
    out.push(&[u8::from(is_mutable), 0])?;
    Ok(out)
}

pub(crate) fn update(
    data: Option<Data<'_>>,
    authority: Option<&Address>,
    sale: Option<bool>,
    mutable: Option<bool>,
) -> Result<Encoded> {
    let mut out = Encoded::new(15);
    out.push(&[u8::from(data.is_some())])?;
    if let Some(data) = data {
        out.data(data)?;
    }
    out.push(&[u8::from(authority.is_some())])?;
    if let Some(authority) = authority {
        out.push(authority.as_ref())?;
    }
    out.boolean(sale)?;
    out.boolean(mutable)?;
    Ok(out)
}

pub(crate) fn u64_data<const TAG: u8>(value: u64) -> [u8; 9] {
    let mut bytes = [0; 9];
    bytes[0] = TAG;
    bytes[1..].copy_from_slice(&value.to_le_bytes());
    bytes
}

pub(crate) fn option_u64_data(value: Option<u64>) -> ([u8; 10], usize) {
    let mut bytes = [0; 10];
    bytes[0] = 17;
    if let Some(value) = value {
        bytes[1] = 1;
        bytes[2..].copy_from_slice(&value.to_le_bytes());
        (bytes, 10)
    } else {
        (bytes, 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximum_strings_fit_and_oversized_fields_are_rejected() {
        let name = "n".repeat(32);
        let symbol = "s".repeat(10);
        let uri = "u".repeat(200);
        let data =
            Data { name: &name, symbol: &symbol, uri: &uri, seller_fee_basis_points: 10_000 };
        assert_eq!(create(data, true).unwrap().len, 262);
        assert_eq!(
            update(Some(data), Some(&Address::default()), Some(true), Some(false)).unwrap().len,
            298
        );
        assert!(create(Data { name: &"n".repeat(33), ..data }, true).is_err());
        assert!(create(Data { symbol: &"s".repeat(11), ..data }, true).is_err());
        assert!(create(Data { uri: &"u".repeat(201), ..data }, true).is_err());
        assert!(create(Data { name: &"é".repeat(17), ..data }, true).is_err());
        assert!(create(Data { seller_fee_basis_points: 10_001, ..data }, true).is_err());
    }

    #[test]
    fn bounded_writer_does_not_overwrite_on_failure() {
        let mut out = Encoded::new(33);
        out.len = 511;
        assert!(out.push(&[1, 2]).is_err());
        assert_eq!(out.len, 511);
        assert_eq!(out.bytes[511], 0);
        out.push(&[1]).unwrap();
        assert_eq!(out.len, 512);
        assert!(out.push(&[2]).is_err());
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn integer_encodings_preserve_every_value() {
        let value: u64 = kani::any();
        let bytes = u64_data::<11>(value);
        assert_eq!(bytes[0], 11);
        assert_eq!(u64::from_le_bytes(bytes[1..].try_into().unwrap()), value);
        let value: Option<u64> = kani::any();
        let (bytes, len) = option_u64_data(value);
        assert_eq!(bytes[0], 17);
        assert_eq!(bytes[1], u8::from(value.is_some()));
        if let Some(value) = value {
            assert_eq!(len, 10);
            assert_eq!(u64::from_le_bytes(bytes[2..].try_into().unwrap()), value);
        } else {
            assert_eq!(len, 2);
        }
    }

    #[kani::proof]
    fn writer_rejects_overflow_without_changing_its_length() {
        let mut out = Encoded::new(33);
        let len: usize = kani::any();
        kani::assume(len <= 512);
        out.len = len;
        let result = out.push(&[7, 8]);
        assert_eq!(result.is_ok(), len <= 510);
        assert_eq!(out.len, if len <= 510 { len + 2 } else { len });
    }
}
