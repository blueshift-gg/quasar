use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;

pub const TWO_DYN_ARGS_ACCOUNT_ACCOUNT_DISCRIMINATOR: &[u8] = &[13];

#[derive(Clone, Copy)]
pub struct TwoDynArgsAccount {
    pub tag: u64,
    pub a_len: u8,
    pub a: u64,
    pub b_len: u8,
    pub b: u64,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for TwoDynArgsAccount
where
    u64: SchemaWrite<C, Src = u64>,
    u8: SchemaWrite<C, Src = u8>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <u64 as SchemaWrite<C>>::size_of(&src.tag)?
            + <u8 as SchemaWrite<C>>::size_of(&src.a_len)?
            + <u64 as SchemaWrite<C>>::size_of(&src.a)?
            + <u8 as SchemaWrite<C>>::size_of(&src.b_len)?
            + <u64 as SchemaWrite<C>>::size_of(&src.b)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(TWO_DYN_ARGS_ACCOUNT_ACCOUNT_DISCRIMINATOR)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.tag)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.a_len)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.a)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.b_len)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.b)?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for TwoDynArgsAccount
where
    u64: SchemaRead<'de, C, Dst = u64>,
    u8: SchemaRead<'de, C, Dst = u8>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 13 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        dst.write(Self {
            tag: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            a_len: <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            a: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            b_len: <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            b: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
