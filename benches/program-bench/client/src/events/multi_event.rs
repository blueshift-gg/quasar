use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;
use solana_address::Address;

pub const MULTI_EVENT_DISCRIMINATOR: &[u8] = &[4];

#[derive(Clone, Copy)]
pub struct MultiEvent {
    pub a: u64,
    pub b: u64,
    pub c: Address,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for MultiEvent
where
    Address: SchemaWrite<C, Src = Address>,
    u64: SchemaWrite<C, Src = u64>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <u64 as SchemaWrite<C>>::size_of(&src.a)?
            + <u64 as SchemaWrite<C>>::size_of(&src.b)?
            + <Address as SchemaWrite<C>>::size_of(&src.c)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(MULTI_EVENT_DISCRIMINATOR)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.a)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.b)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.c)?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for MultiEvent
where
    Address: SchemaRead<'de, C, Dst = Address>,
    u64: SchemaRead<'de, C, Dst = u64>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 4 {
            return Err(ReadError::InvalidValue("invalid event discriminator"));
        }
        dst.write(Self {
            a: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            b: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            c: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
