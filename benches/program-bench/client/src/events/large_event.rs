use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;
use solana_address::Address;

pub const LARGE_EVENT_DISCRIMINATOR: &[u8] = &[6];

#[derive(Clone, Copy)]
pub struct LargeEvent {
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
    pub e: Address,
    pub f: Address,
    pub g: u128,
    pub h: u128,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for LargeEvent
where
    Address: SchemaWrite<C, Src = Address>,
    u128: SchemaWrite<C, Src = u128>,
    u64: SchemaWrite<C, Src = u64>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <u64 as SchemaWrite<C>>::size_of(&src.a)?
            + <u64 as SchemaWrite<C>>::size_of(&src.b)?
            + <u64 as SchemaWrite<C>>::size_of(&src.c)?
            + <u64 as SchemaWrite<C>>::size_of(&src.d)?
            + <Address as SchemaWrite<C>>::size_of(&src.e)?
            + <Address as SchemaWrite<C>>::size_of(&src.f)?
            + <u128 as SchemaWrite<C>>::size_of(&src.g)?
            + <u128 as SchemaWrite<C>>::size_of(&src.h)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(LARGE_EVENT_DISCRIMINATOR)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.a)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.b)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.c)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.d)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.e)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.f)?;
        <u128 as SchemaWrite<C>>::write(writer.by_ref(), &src.g)?;
        <u128 as SchemaWrite<C>>::write(writer.by_ref(), &src.h)?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for LargeEvent
where
    Address: SchemaRead<'de, C, Dst = Address>,
    u128: SchemaRead<'de, C, Dst = u128>,
    u64: SchemaRead<'de, C, Dst = u64>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 6 {
            return Err(ReadError::InvalidValue("invalid event discriminator"));
        }
        dst.write(Self {
            a: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            b: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            c: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            d: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            e: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            f: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            g: <u128 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            h: <u128 as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
