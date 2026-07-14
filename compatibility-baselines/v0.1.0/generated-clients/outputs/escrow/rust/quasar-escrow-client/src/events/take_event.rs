use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;
use solana_address::Address;

pub const TAKE_EVENT_DISCRIMINATOR: &[u8] = &[2];

#[derive(Clone, Copy)]
pub struct TakeEvent {
    pub escrow: Address,
}

unsafe impl<C: ConfigCore> SchemaWrite<C> for TakeEvent
where
    Address: SchemaWrite<C, Src = Address>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <Address as SchemaWrite<C>>::size_of(&src.escrow)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(TAKE_EVENT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.escrow)?;
        Ok(())
    }
}

unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for TakeEvent
where
    Address: SchemaRead<'de, C, Dst = Address>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 2 {
            return Err(ReadError::InvalidValue("invalid event discriminator"));
        }
        dst.write(Self {
            escrow: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
