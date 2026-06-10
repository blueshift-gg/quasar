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

pub const MAKE_EVENT_DISCRIMINATOR: &[u8] = &[0];

#[derive(Clone, Copy)]
pub struct MakeEvent {
    pub escrow: Address,
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub deposit: u64,
    pub receive: u64,
}

unsafe impl<C: ConfigCore> SchemaWrite<C> for MakeEvent
where
    Address: SchemaWrite<C, Src = Address>,
    u64: SchemaWrite<C, Src = u64>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1 + <Address as SchemaWrite<C>>::size_of(&src.escrow)?
            + <Address as SchemaWrite<C>>::size_of(&src.maker)?
            + <Address as SchemaWrite<C>>::size_of(&src.mint_a)?
            + <Address as SchemaWrite<C>>::size_of(&src.mint_b)?
            + <u64 as SchemaWrite<C>>::size_of(&src.deposit)?
            + <u64 as SchemaWrite<C>>::size_of(&src.receive)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(MAKE_EVENT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.escrow)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.maker)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.mint_a)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.mint_b)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.deposit)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.receive)?;
        Ok(())
    }
}

unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for MakeEvent
where
    Address: SchemaRead<'de, C, Dst = Address>,
    u64: SchemaRead<'de, C, Dst = u64>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 0 {
            return Err(ReadError::InvalidValue("invalid event discriminator"));
        }
        dst.write(Self {
            escrow: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            maker: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            mint_a: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            mint_b: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            deposit: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            receive: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
