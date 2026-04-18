use core::fmt;

#[repr(transparent)]
#[derive(Copy, Clone, Default)]
#[cfg_attr(feature = "wincode", derive(wincode::SchemaWrite, wincode::SchemaRead))]
pub struct PodBool([u8; 1]);

impl PodBool {
    /// Returns the contained [`bool`] value. Any non-zero byte yields `true`.
    #[inline(always)]
    pub fn get(&self) -> bool {
        self.0[0] != 0
    }

    #[inline(always)]
    pub fn is_true(&self) -> bool {
        self.get()
    }

    #[inline(always)]
    pub fn is_false(&self) -> bool {
        !self.get()
    }

    #[inline(always)]
    pub fn set(&mut self, value: bool) {
        self.0 = [value as u8];
    }
}

impl From<bool> for PodBool {
    #[inline(always)]
    fn from(v: bool) -> Self {
        Self([v as u8])
    }
}

impl From<PodBool> for bool {
    #[inline(always)]
    fn from(v: PodBool) -> Self {
        v.get()
    }
}

impl PartialEq for PodBool {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}
impl Eq for PodBool {}

impl PartialEq<bool> for PodBool {
    #[inline(always)]
    fn eq(&self, other: &bool) -> bool {
        self.get() == *other
    }
}

impl core::ops::Not for PodBool {
    type Output = Self;
    #[inline(always)]
    fn not(self) -> Self {
        Self::from(!self.get())
    }
}

impl core::hash::Hash for PodBool {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.get().hash(state);
    }
}

impl core::ops::BitAnd<bool> for PodBool {
    type Output = PodBool;
    fn bitand(self, rhs: bool) -> PodBool {
        PodBool::from(self.get() & rhs)
    }
}

impl core::ops::BitOr<bool> for PodBool {
    type Output = PodBool;
    fn bitor(self, rhs: bool) -> PodBool {
        PodBool::from(self.get() | rhs)
    }
}

impl PartialEq<PodBool> for bool {
    fn eq(&self, other: &PodBool) -> bool {
        *self == other.get()
    }
}

impl fmt::Display for PodBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl fmt::Debug for PodBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.get(), f)
    }
}

const _: () = assert!(core::mem::align_of::<PodBool>() == 1);
const _: () = assert!(core::mem::size_of::<PodBool>() == 1);

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn bool_roundtrip() {
        let v: bool = kani::any();
        let pod = PodBool::from(v);
        assert!(pod.get() == v, "bool roundtrip must preserve value");
    }

    #[kani::proof]
    fn any_nonzero_is_true() {
        let byte: u8 = kani::any();
        // Construct PodBool from a raw byte. PodBool is #[repr(transparent)]
        // over [u8; 1], so this transmute is sound for any bit pattern.
        let pod: PodBool = unsafe { core::mem::transmute([byte]) };
        assert!(pod.get() == (byte != 0), "any nonzero byte must be true");
    }

    #[kani::proof]
    fn not_involution() {
        let v: bool = kani::any();
        let pod = PodBool::from(v);
        assert!(!!pod == pod, "double negation must be identity");
    }
}
