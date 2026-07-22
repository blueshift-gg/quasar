use quasar_lang::client::{DynString, DynVec};
use solana_address::Address;
use std::mem::MaybeUninit;
use wincode::config::ConfigCore;
use wincode::error::{ReadError, ReadResult, WriteResult};
use wincode::io::{Reader, Writer};
use wincode::{SchemaRead, SchemaWrite};

pub const MULTISIG_CONFIG_ACCOUNT_DISCRIMINATOR: &[u8] = &[1];

#[derive(Clone)]
pub struct MultisigConfig {
    pub creator: Address,
    pub threshold: u8,
    pub bump: u8,
    pub label: DynString<u8>,
    pub signers: DynVec<Address, u16>,
}

// SAFETY: TYPE_META remains dynamic and size_of counts exactly the
// discriminator, fixed fields, length prefixes, and payload bytes
// written below.
unsafe impl<C: ConfigCore> SchemaWrite<C> for MultisigConfig
where
    Address: SchemaWrite<C, Src = Address>,
    u8: SchemaWrite<C, Src = u8>,
{
    type Src = Self;

    fn size_of(src: &Self) -> WriteResult<usize> {
        Ok(1 + <Address as SchemaWrite<C>>::size_of(&src.creator)?
            + <u8 as SchemaWrite<C>>::size_of(&src.threshold)?
            + <u8 as SchemaWrite<C>>::size_of(&src.bump)?
            + 1
            + 2
            + src.label.len()
            + {
                let mut s = 0usize;
                for item in src.signers.iter() {
                    s += <Address as SchemaWrite<C>>::size_of(item)?;
                }
                s
            })
    }

    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {
        writer.write(MULTISIG_CONFIG_ACCOUNT_DISCRIMINATOR)?;
        <Address as SchemaWrite<C>>::write(writer.by_ref(), &src.creator)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.threshold)?;
        <u8 as SchemaWrite<C>>::write(writer.by_ref(), &src.bump)?;
        writer.write(&(src.label.len() as u64).to_le_bytes()[..1])?;
        writer.write(&(src.signers.len() as u64).to_le_bytes()[..2])?;
        writer.write(src.label.as_bytes())?;
        for item in src.signers.iter() {
            <Address as SchemaWrite<C>>::write(writer.by_ref(), item)?;
        }
        Ok(())
    }
}

// SAFETY: TYPE_META remains dynamic and read initializes dst exactly
// once, only after every discriminator, field, and payload validates.
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for MultisigConfig
where
    Address: SchemaRead<'de, C, Dst = Address>,
    u8: SchemaRead<'de, C, Dst = u8>,
{
    type Dst = Self;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> {
        let disc = reader.take_byte()?;
        if disc != 1 {
            return Err(ReadError::InvalidValue("invalid account discriminator"));
        }
        let creator = <Address as SchemaRead<'de, C>>::get(reader.by_ref())?;
        let threshold = <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?;
        let bump = <u8 as SchemaRead<'de, C>>::get(reader.by_ref())?;
        let label_len = {
            let mut buf = [0u8; 8];
            let pfx_bytes = reader.take_scoped(1)?;
            buf[..1].copy_from_slice(pfx_bytes);
            usize::try_from(u64::from_le_bytes(buf))
                .map_err(|_| ReadError::PointerSizedReadError)?
        };
        let signers_len = {
            let mut buf = [0u8; 8];
            let pfx_bytes = reader.take_scoped(2)?;
            buf[..2].copy_from_slice(pfx_bytes);
            usize::try_from(u64::from_le_bytes(buf))
                .map_err(|_| ReadError::PointerSizedReadError)?
        };
        let label: DynString<u8> = {
            let bytes = reader.take_scoped(label_len)?;
            core::str::from_utf8(bytes)?;
            bytes.to_vec().into()
        };
        let signers: DynVec<Address, u16> = {
            const MAX_DECODE_ELEMENTS: usize = 10 * 1024 * 1024;
            if signers_len > MAX_DECODE_ELEMENTS {
                return Err(ReadError::PreallocationSizeLimit {
                    needed: signers_len,
                    limit: MAX_DECODE_ELEMENTS,
                });
            }
            let mut items = Vec::with_capacity(signers_len.min(4096));
            for _ in 0..signers_len {
                items.push(<Address as SchemaRead<'de, C>>::get(reader.by_ref())?);
            }
            items.into()
        };
        dst.write(Self {
            creator,
            threshold,
            bump,
            label,
            signers,
        });
        Ok(())
    }
}
