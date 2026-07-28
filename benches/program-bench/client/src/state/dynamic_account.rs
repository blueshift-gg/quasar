use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;
use solana_address::Address;
use quasar_lang::client::{DynString, DynVec};

pub const DYNAMIC_ACCOUNT_ACCOUNT_DISCRIMINATOR: &[u8] = &[12];

#[derive(Clone)]
pub struct DynamicAccount {
    pub name: DynString<u8>,
    pub tags: DynVec<Address, u16>,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for DynamicAccount
where
    Address: SchemaWrite<C, Src = Address>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + 1
            + 2
            + src.name.len()
            + {
                let mut s = 0usize;
                for item in src.tags.iter() {
                    s += <Address as SchemaWrite<C>>::size_of(item)?;
                }
                s
            })
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(DYNAMIC_ACCOUNT_ACCOUNT_DISCRIMINATOR)?;
        writer.write(&(src.name.len() as u64).to_le_bytes()[..1])?;
        writer.write(&(src.tags.len() as u64).to_le_bytes()[..2])?;
        writer.write(src.name.as_bytes())?;
        for item in src.tags.iter() {
            <Address as SchemaWrite<C>>::write(writer.by_ref(), item)?;
        }
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for DynamicAccount
where
    Address: SchemaRead<'de, C, Dst = Address>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 12 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        let name_len = {
            let mut buf = [0u8; 8];
            let pfx_bytes = reader.take_scoped(1)?;
            buf[..1].copy_from_slice(pfx_bytes);
            usize::try_from(u64::from_le_bytes(buf))
                .map_err(|_| ReadError::PointerSizedReadError)?
        };
        let tags_len = {
            let mut buf = [0u8; 8];
            let pfx_bytes = reader.take_scoped(2)?;
            buf[..2].copy_from_slice(pfx_bytes);
            usize::try_from(u64::from_le_bytes(buf))
                .map_err(|_| ReadError::PointerSizedReadError)?
        };
        let name: DynString<u8> = {
            let bytes = reader.take_scoped(name_len)?;
            core::str::from_utf8(bytes)?;
            bytes.to_vec().into()
        };
        let tags: DynVec<Address, u16> = {
            const MAX_DECODE_ELEMENTS: usize = 10 * 1024 * 1024;
            if tags_len > MAX_DECODE_ELEMENTS {
                return Err(ReadError::PreallocationSizeLimit { needed: tags_len, limit: MAX_DECODE_ELEMENTS });
            }
            let mut items = Vec::with_capacity(tags_len.min(4096));
            for _ in 0..tags_len {
                items.push(<Address as SchemaRead<'de, C>>::get(reader.by_ref())?);
            }
            items.into()
        };
        dst.write(Self {
            name,
            tags,
        });
        Ok(())
    }
}
