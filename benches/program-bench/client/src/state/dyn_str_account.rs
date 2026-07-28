use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;
use solana_address::Address;
use quasar_lang::client::{DynString};

pub const DYN_STR_ACCOUNT_ACCOUNT_DISCRIMINATOR: &[u8] = &[14];

#[derive(Clone)]
pub struct DynStrAccount {
    pub authority: Address,
    pub label: DynString<u8>,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for DynStrAccount
where
    Address: SchemaWrite<C, Src = Address>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <Address as SchemaWrite<C>>::size_of(&src.authority)?
            + 1
            + src.label.len())
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(DYN_STR_ACCOUNT_ACCOUNT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.authority)?;
        writer.write(&(src.label.len() as u64).to_le_bytes()[..1])?;
        writer.write(src.label.as_bytes())?;
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for DynStrAccount
where
    Address: SchemaRead<'de, C, Dst = Address>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 14 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        let authority = <Address as SchemaRead<'de, C>>::get(reader.by_ref())?;
        let label_len = {
            let mut buf = [0u8; 8];
            let pfx_bytes = reader.take_scoped(1)?;
            buf[..1].copy_from_slice(pfx_bytes);
            usize::try_from(u64::from_le_bytes(buf))
                .map_err(|_| ReadError::PointerSizedReadError)?
        };
        let label: DynString<u8> = {
            let bytes = reader.take_scoped(label_len)?;
            core::str::from_utf8(bytes)?;
            bytes.to_vec().into()
        };
        dst.write(Self {
            authority,
            label,
        });
        Ok(())
    }
}
