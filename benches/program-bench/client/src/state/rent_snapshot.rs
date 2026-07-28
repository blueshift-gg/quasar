use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;

pub const RENT_SNAPSHOT_ACCOUNT_DISCRIMINATOR: &[u8] = &[18];

#[derive(Clone, Copy)]
pub struct RentSnapshot {
    pub min_balance_100: u64,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for RentSnapshot
where
    u64: SchemaWrite<C, Src = u64>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <u64 as SchemaWrite<C>>::size_of(&src.min_balance_100)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(RENT_SNAPSHOT_ACCOUNT_DISCRIMINATOR)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.min_balance_100)?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for RentSnapshot
where
    u64: SchemaRead<'de, C, Dst = u64>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 18 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        dst.write(Self {
            min_balance_100: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
