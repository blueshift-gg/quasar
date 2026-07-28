use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;
use solana_address::Address;

pub const ERROR_TEST_ACCOUNT_ACCOUNT_DISCRIMINATOR: &[u8] = &[16];

#[derive(Clone, Copy)]
pub struct ErrorTestAccount {
    pub authority: Address,
    pub value: u64,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for ErrorTestAccount
where
    Address: SchemaWrite<C, Src = Address>,
    u64: SchemaWrite<C, Src = u64>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <Address as SchemaWrite<C>>::size_of(&src.authority)?
            + <u64 as SchemaWrite<C>>::size_of(&src.value)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(ERROR_TEST_ACCOUNT_ACCOUNT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.authority)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.value)?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for ErrorTestAccount
where
    Address: SchemaRead<'de, C, Dst = Address>,
    u64: SchemaRead<'de, C, Dst = u64>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 16 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        dst.write(Self {
            authority: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            value: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
