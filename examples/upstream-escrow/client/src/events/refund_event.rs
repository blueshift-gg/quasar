use {
    solana_address::Address,
    std::mem::MaybeUninit,
    wincode::{
        config::ConfigCore,
        error::{ReadError, ReadResult, WriteResult},
        io::{Reader, Writer},
        SchemaRead, SchemaWrite,
    },
};

pub const REFUND_EVENT_DISCRIMINATOR: &[u8] = &[2];

#[derive(Clone, Copy)]
pub struct RefundEvent {
    pub escrow: Address,
}

unsafe impl<C: ConfigCore> SchemaWrite<C> for RefundEvent
where
    Address: SchemaWrite<C, Src = Address>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1 + <Address as SchemaWrite<C>>::size_of(&src.escrow)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(REFUND_EVENT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.escrow)?;
        Ok(())
    }
}

unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for RefundEvent
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
