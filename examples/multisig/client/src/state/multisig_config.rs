use {
    quasar_lang::client::{DynBytes, DynVec},
    solana_address::Address,
    std::mem::MaybeUninit,
    wincode::{
        config::ConfigCore,
        error::{ReadError, ReadResult, WriteResult},
        io::{Reader, Writer},
        SchemaRead, SchemaWrite,
    },
};

pub const MULTISIG_CONFIG_ACCOUNT_DISCRIMINATOR: &[u8] = &[1];

#[derive(Clone)]
pub struct MultisigConfig {
    pub creator: Address,
    pub threshold: u8,
    pub bump: u8,
    pub label: DynBytes<u8>,
    pub signers: DynVec<Address, u16>,
}

unsafe impl<C: ConfigCore> SchemaWrite<C> for MultisigConfig
where
    Address: SchemaWrite<C, Src = Address>,
    DynBytes<u8>: SchemaWrite<C, Src = DynBytes<u8>>,
    DynVec<Address, u16>: SchemaWrite<C, Src = DynVec<Address, u16>>,
    u8: SchemaWrite<C, Src = u8>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1 + <Address as SchemaWrite<C>>::size_of(&src.creator)?
            + <u8 as SchemaWrite<C>>::size_of(&src.threshold)?
            + <u8 as SchemaWrite<C>>::size_of(&src.bump)?
            + <DynBytes<u8> as SchemaWrite<C>>::size_of(&src.label)?
            + <DynVec<Address, u16> as SchemaWrite<C>>::size_of(&src.signers)?)
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(MULTISIG_CONFIG_ACCOUNT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.creator)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.threshold)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.bump)?;
        <DynBytes<u8> as SchemaWrite<C>>::write(writer.by_ref(), &src.label)?;
        <DynVec<Address, u16> as SchemaWrite<C>>::write(writer.by_ref(), &src.signers)?;
        Ok(())
    }
}

unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for MultisigConfig
where
    Address: SchemaRead<'de, C, Dst = Address>,
    DynBytes<u8>: SchemaRead<'de, C, Dst = DynBytes<u8>>,
    DynVec<Address, u16>: SchemaRead<'de, C, Dst = DynVec<Address, u16>>,
    u8: SchemaRead<'de, C, Dst = u8>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 1 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        dst.write(Self {
            creator: <Address as SchemaRead<'de, C>>::get(reader.by_ref())?,
            threshold: <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            bump: <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?,
            label: <DynBytes<u8> as SchemaRead<'de, C>>::get(reader.by_ref())?,
            signers: <DynVec<Address, u16> as SchemaRead<'de, C>>::get(reader.by_ref())?,
        });
        Ok(())
    }
}
