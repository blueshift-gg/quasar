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

pub const ESCROW_ACCOUNT_DISCRIMINATOR: &[u8] = &[1];

#[derive(Clone, Copy)]
pub struct Escrow {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
    pub bump: u8,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes written
// below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for Escrow
where
    Address: SchemaWrite<C, Src = Address>,
    u64: SchemaWrite<C, Src = u64>,
    u8: SchemaWrite<C, Src = u8>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1 + <Address as SchemaWrite<C>>::size_of(&src.maker)?
            + <Address as SchemaWrite<C>>::size_of(&src.mint_a)?
            + <Address as SchemaWrite<C>>::size_of(&src.mint_b)?
            + <Address as SchemaWrite<C>>::size_of(&src.maker_ta_b)?
            + <u64 as SchemaWrite<C>>::size_of(&src.receive)?
            + <u8 as SchemaWrite<C>>::size_of(&src.bump)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(ESCROW_ACCOUNT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.maker)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.mint_a)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.mint_b)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.maker_ta_b)?;
        <u64 as SchemaWrite<C>>::write(writer.by_ref(), &src.receive)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.bump)?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly once,
// only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for Escrow
where
    Address: SchemaRead<'de, C, Dst = Address>,
    u64: SchemaRead<'de, C, Dst = u64>,
    u8: SchemaRead<'de, C, Dst = u8>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 1 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        dst.write(Self {
            maker: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            mint_a: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            mint_b: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            maker_ta_b: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            receive: <u64 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            bump: <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
