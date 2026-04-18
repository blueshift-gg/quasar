//! Manual `SchemaWrite` / `SchemaRead` impls for pod types that cannot use
//! derive (generic params, `MaybeUninit` fields).  Each impl serializes the
//! full fixed-size byte representation via raw pointer cast — matching the
//! zero-copy layout used on-chain.

use {
    super::{option::PodOption, string::PodString, vec::PodVec},
    crate::traits::ZcElem,
    wincode::config::ConfigCore,
};

// ---------------------------------------------------------------------------
// PodString
// ---------------------------------------------------------------------------

unsafe impl<const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodString<N, PFX>
{
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        let __bytes = unsafe {
            core::slice::from_raw_parts(
                src as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        };
        __writer.write(__bytes)?;
        Ok(())
    }
}

unsafe impl<'__de, const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaRead<'__de, C>
    for PodString<N, PFX>
{
    type Dst = Self;

    fn read(
        mut __reader: impl wincode::io::Reader<'__de>,
        __dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::error::ReadResult<()> {
        let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
        let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
        <Self as crate::ZcValidate>::validate_ref(&__val)
            .map_err(|_| wincode::error::ReadError::InvalidValue("PodString validation failed"))?;
        __dst.write(__val);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PodVec
// ---------------------------------------------------------------------------

unsafe impl<T: ZcElem, const N: usize, const PFX: usize, C: ConfigCore> wincode::SchemaWrite<C>
    for PodVec<T, N, PFX>
{
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        let __bytes = unsafe {
            core::slice::from_raw_parts(
                src as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        };
        __writer.write(__bytes)?;
        Ok(())
    }
}

unsafe impl<'__de, T: ZcElem, const N: usize, const PFX: usize, C: ConfigCore>
    wincode::SchemaRead<'__de, C> for PodVec<T, N, PFX>
{
    type Dst = Self;

    fn read(
        mut __reader: impl wincode::io::Reader<'__de>,
        __dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::error::ReadResult<()> {
        let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
        let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
        <Self as crate::ZcValidate>::validate_ref(&__val)
            .map_err(|_| wincode::error::ReadError::InvalidValue("PodVec validation failed"))?;
        __dst.write(__val);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PodOption
// ---------------------------------------------------------------------------

unsafe impl<T: Copy, C: ConfigCore> wincode::SchemaWrite<C> for PodOption<T> {
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
        Ok(core::mem::size_of::<Self>())
    }

    fn write(
        mut __writer: impl wincode::io::Writer,
        src: &Self,
    ) -> wincode::error::WriteResult<()> {
        let __bytes = unsafe {
            core::slice::from_raw_parts(
                src as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        };
        __writer.write(__bytes)?;
        Ok(())
    }
}

unsafe impl<'__de, T: ZcElem, C: ConfigCore> wincode::SchemaRead<'__de, C> for PodOption<T> {
    type Dst = Self;

    fn read(
        mut __reader: impl wincode::io::Reader<'__de>,
        __dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::error::ReadResult<()> {
        let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
        let __val = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
        <Self as crate::ZcValidate>::validate_ref(&__val)
            .map_err(|_| wincode::error::ReadError::InvalidValue("PodOption validation failed"))?;
        __dst.write(__val);
        Ok(())
    }
}
