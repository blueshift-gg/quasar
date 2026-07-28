use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;

pub const DATA_ACCOUNT_DISCRIMINATOR: &[u8] = &[1];

#[derive(Clone, Copy)]
pub struct Data {
    pub byte: u8,
    pub bump: u8,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for Data
where
    u8: SchemaWrite<C, Src = u8>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <u8 as SchemaWrite<C>>::size_of(&src.byte)?
            + <u8 as SchemaWrite<C>>::size_of(&src.bump)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(DATA_ACCOUNT_DISCRIMINATOR)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.byte)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.bump)?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for Data
where
    u8: SchemaRead<'de, C, Dst = u8>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 1 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        dst.write(Self {
            byte: <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            bump: <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
