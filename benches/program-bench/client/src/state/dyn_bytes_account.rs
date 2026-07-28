use wincode::{SchemaWrite, SchemaRead};
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use std::mem::MaybeUninit;
use solana_address::Address;
use quasar_lang::client::{DynVec};

pub const DYN_BYTES_ACCOUNT_ACCOUNT_DISCRIMINATOR: &[u8] = &[15];

#[derive(Clone)]
pub struct DynBytesAccount {
    pub authority: Address,
    pub data: DynVec<u8, u16>,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for DynBytesAccount
where
    Address: SchemaWrite<C, Src = Address>,
    u8: SchemaWrite<C, Src = u8>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1
            + <Address as SchemaWrite<C>>::size_of(&src.authority)?
            + 2
            + {
                let mut s = 0usize;
                for item in src.data.iter() {
                    s += <u8 as SchemaWrite<C>>::size_of(item)?;
                }
                s
            })
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(DYN_BYTES_ACCOUNT_ACCOUNT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.authority)?;
        writer.write(&(src.data.len() as u64).to_le_bytes()[..2])?;
        for item in src.data.iter() {
            <u8 as SchemaWrite<C>>::write(writer.by_ref(), item)?;
        }
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for DynBytesAccount
where
    Address: SchemaRead<'de, C, Dst = Address>,
    u8: SchemaRead<'de, C, Dst = u8>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 15 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        let authority = <Address as SchemaRead<'de, C>>::get(reader.by_ref())?;
        let data_len = {
            let mut buf = [0u8; 8];
            let pfx_bytes = reader.take_scoped(2)?;
            buf[..2].copy_from_slice(pfx_bytes);
            usize::try_from(u64::from_le_bytes(buf))
                .map_err(|_| ReadError::PointerSizedReadError)?
        };
        let data: DynVec<u8, u16> = {
            const MAX_DECODE_ELEMENTS: usize = 10 * 1024 * 1024;
            if data_len > MAX_DECODE_ELEMENTS {
                return Err(ReadError::PreallocationSizeLimit { needed: data_len, limit: MAX_DECODE_ELEMENTS });
            }
            let mut items = Vec::with_capacity(data_len.min(4096));
            for _ in 0..data_len {
                items.push(<u8 as SchemaRead<'de, C>>::get(reader.by_ref())?);
            }
            items.into()
        };
        dst.write(Self {
            authority,
            data,
        });
        Ok(())
    }
}
