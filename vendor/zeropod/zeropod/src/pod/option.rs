use core::mem::MaybeUninit;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PodOption<T: Copy, const PFX: usize = 1> {
    tag: [u8; PFX],
    value: MaybeUninit<T>,
}

const _: () = assert!(core::mem::align_of::<PodOption<u8>>() == 1);
const _: () = assert!(core::mem::align_of::<PodOption<u8, 4>>() == 1);
const _: () = assert!(core::mem::size_of::<PodOption<[u8; 32], 4>>() == 36);

impl<T: Copy, const PFX: usize> PodOption<T, PFX> {
    const _PFX_CHECK: () = assert!(
        PFX == 1 || PFX == 2 || PFX == 4,
        "PodOption<T, PFX>: PFX must be 1, 2, or 4"
    );

    #[inline(always)]
    fn decode_tag(&self) -> u32 {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_PFX_CHECK;
        match PFX {
            1 => self.tag[0] as u32,
            2 => u16::from_le_bytes([self.tag[0], self.tag[1]]) as u32,
            _ => u32::from_le_bytes([self.tag[0], self.tag[1], self.tag[2], self.tag[3]]),
        }
    }

    #[inline(always)]
    fn encode_tag(v: u32) -> [u8; PFX] {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_PFX_CHECK;
        let mut buf = [0u8; PFX];
        match PFX {
            1 => buf[0] = v as u8,
            2 => {
                let bytes = (v as u16).to_le_bytes();
                buf[0] = bytes[0];
                buf[1] = bytes[1];
            }
            _ => {
                let bytes = v.to_le_bytes();
                buf[..4].copy_from_slice(&bytes);
            }
        }
        buf
    }

    #[inline(always)]
    pub fn none() -> Self {
        Self {
            tag: [0u8; PFX],
            value: MaybeUninit::zeroed(),
        }
    }

    #[inline(always)]
    pub fn some(value: T) -> Self {
        Self {
            tag: Self::encode_tag(1),
            value: MaybeUninit::new(value),
        }
    }

    #[inline(always)]
    pub fn is_some(&self) -> bool {
        self.decode_tag() == 1
    }

    #[inline(always)]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }

    #[inline(always)]
    pub fn get(&self) -> Option<T> {
        if self.is_some() {
            Some(unsafe { self.value.assume_init() })
        } else {
            None
        }
    }

    /// Borrow the inner value if `Some`.
    #[inline(always)]
    pub fn get_ref(&self) -> Option<&T> {
        if self.is_some() {
            Some(unsafe { self.value.assume_init_ref() })
        } else {
            None
        }
    }

    /// Borrow the inner value without checking the tag.
    ///
    /// This is safe to call when `T: ZcElem` (align-1, Copy) because the value
    /// bytes are always initialized (zeroed by `none()`, written by `some()`).
    /// Forming `&T` is sound. However, the `T` value may not pass `ZcValidate`
    /// when tag == 0 — the caller must handle semantic validity.
    #[inline(always)]
    pub fn value_unchecked(&self) -> &T {
        // SAFETY: MaybeUninit is zeroed (none) or written (some). T is Copy
        // with align 1 (ZcElem). Forming &T over initialized memory is sound.
        unsafe { self.value.assume_init_ref() }
    }

    #[inline(always)]
    pub fn set(&mut self, value: Option<T>) {
        match value {
            Some(v) => {
                self.tag = Self::encode_tag(1);
                self.value = MaybeUninit::new(v);
            }
            None => {
                self.tag = [0u8; PFX];
                self.value = MaybeUninit::zeroed();
            }
        }
    }

    pub fn raw_tag(&self) -> u32 {
        self.decode_tag()
    }

    #[inline(always)]
    pub fn tag_valid(&self) -> bool {
        self.raw_tag() <= 1
    }

    /// # Safety
    /// Caller must ensure tag == 1 (Some).
    #[inline(always)]
    pub unsafe fn assume_init_ref(&self) -> &T {
        self.value.assume_init_ref()
    }

    pub fn take(&mut self) -> Option<T> {
        let result = self.get();
        self.tag = [0u8; PFX];
        self.value = MaybeUninit::zeroed();
        result
    }

    pub fn replace(&mut self, value: T) -> Option<T> {
        let old = self.get();
        self.tag = Self::encode_tag(1);
        self.value = MaybeUninit::new(value);
        old
    }

    pub fn clear(&mut self) {
        self.tag = [0u8; PFX];
        self.value = MaybeUninit::zeroed();
    }

    pub fn unwrap_or(self, default: T) -> T {
        match self.get() {
            Some(v) => v,
            None => default,
        }
    }

    pub fn map_or<U>(&self, default: U, f: impl FnOnce(T) -> U) -> U {
        match self.get() {
            Some(v) => f(v),
            None => default,
        }
    }
}

impl<T: Copy, const PFX: usize> Default for PodOption<T, PFX> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T: Copy + PartialEq, const PFX: usize> PartialEq for PodOption<T, PFX> {
    fn eq(&self, other: &Self) -> bool {
        match (self.get(), other.get()) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Copy + Eq, const PFX: usize> Eq for PodOption<T, PFX> {}

impl<T: Copy + PartialEq, const PFX: usize> PartialEq<Option<T>> for PodOption<T, PFX> {
    fn eq(&self, other: &Option<T>) -> bool {
        match (self.get(), other) {
            (Some(a), Some(b)) => a == *b,
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Copy + core::fmt::Debug, const PFX: usize> core::fmt::Debug for PodOption<T, PFX> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.get() {
            Some(v) => write!(f, "Some({:?})", v),
            None => write!(f, "None"),
        }
    }
}

#[cfg(all(kani, feature = "kani"))]
mod kani_proofs {
    use super::*;

    // Macro to generate a proof for each PFX value (1, 2, 4).
    macro_rules! pfx_proofs {
        ($base:ident, $body:expr) => {
            mod $base {
                use super::*;

                #[kani::proof]
                fn pfx1() {
                    const PFX: usize = 1;
                    $body
                }
                #[kani::proof]
                fn pfx2() {
                    const PFX: usize = 2;
                    $body
                }
                #[kani::proof]
                fn pfx4() {
                    const PFX: usize = 4;
                    $body
                }
            }
        };
    }

    pfx_proofs!(some_roundtrip, {
        let v: u8 = kani::any();
        let pod = PodOption::<u8, PFX>::some(v);
        assert!(pod.is_some());
        assert!(!pod.is_none());
        assert!(pod.get() == Some(v), "some roundtrip must preserve value");
    });

    pfx_proofs!(none_roundtrip, {
        let pod = PodOption::<u8, PFX>::none();
        assert!(pod.is_none());
        assert!(!pod.is_some());
        assert!(pod.get() == None, "none must return None");
    });

    pfx_proofs!(set_some_then_get, {
        let v: u8 = kani::any();
        let mut pod = PodOption::<u8, PFX>::none();
        pod.set(Some(v));
        assert!(
            pod.get() == Some(v),
            "set(Some(v)) then get() must return Some(v)"
        );
    });

    pfx_proofs!(set_none_then_get, {
        let v: u8 = kani::any();
        let mut pod = PodOption::<u8, PFX>::some(v);
        pod.set(None);
        assert!(pod.get() == None, "set(None) then get() must return None");
    });

    pfx_proofs!(take_returns_value_and_clears, {
        let v: u8 = kani::any();
        let mut pod = PodOption::<u8, PFX>::some(v);
        let taken = pod.take();
        assert!(taken == Some(v), "take must return the value");
        assert!(pod.is_none(), "take must clear to None");
    });

    pfx_proofs!(replace_returns_old, {
        let old: u8 = kani::any();
        let new: u8 = kani::any();
        let mut pod = PodOption::<u8, PFX>::some(old);
        let returned = pod.replace(new);
        assert!(returned == Some(old), "replace must return old value");
        assert!(pod.get() == Some(new), "replace must set new value");
    });

    pfx_proofs!(replace_on_none_returns_none, {
        let v: u8 = kani::any();
        let mut pod = PodOption::<u8, PFX>::none();
        let returned = pod.replace(v);
        assert!(returned == None, "replace on None must return None");
        assert!(pod.get() == Some(v), "replace must set value");
    });

    pfx_proofs!(unwrap_or_some, {
        let v: u8 = kani::any();
        let default: u8 = kani::any();
        let pod = PodOption::<u8, PFX>::some(v);
        assert!(
            pod.unwrap_or(default) == v,
            "unwrap_or on Some must return value"
        );
    });

    pfx_proofs!(unwrap_or_none, {
        let default: u8 = kani::any();
        let pod = PodOption::<u8, PFX>::none();
        assert!(
            pod.unwrap_or(default) == default,
            "unwrap_or on None must return default"
        );
    });

    pfx_proofs!(default_is_none, {
        let pod = PodOption::<u8, PFX>::default();
        assert!(pod.is_none(), "default must be None");
        assert!(pod.raw_tag() == 0, "default tag must be 0");
    });

    pfx_proofs!(clear_makes_none, {
        let v: u8 = kani::any();
        let mut pod = PodOption::<u8, PFX>::some(v);
        pod.clear();
        assert!(pod.is_none(), "clear must make None");
    });

    pfx_proofs!(get_ref_borrow, {
        let v: u8 = kani::any();
        let pod = PodOption::<u8, PFX>::some(v);
        assert!(pod.get_ref() == Some(&v), "get_ref must borrow the value");
        let none_pod = PodOption::<u8, PFX>::none();
        assert!(none_pod.get_ref().is_none(), "get_ref on None must be None");
    });

    // PFX=1 specific: invalid tag (not 0 or 1) — must not be Some.
    #[kani::proof]
    fn invalid_tag_pfx1() {
        let tag: u8 = kani::any();
        kani::assume(tag != 0 && tag != 1);
        let mut buf = [0u8; 2]; // PodOption<u8, 1>: 1 tag + 1 value
        buf[0] = tag;
        buf[1] = kani::any();
        let pod = unsafe { &*(buf.as_ptr() as *const PodOption<u8, 1>) };
        assert!(!pod.is_some(), "invalid tag must not be Some");
        assert!(pod.get() == None, "invalid tag must return None from get()");
    }

    // PFX=4: any 4-byte tag > 1 rejected.
    #[kani::proof]
    fn tag_rejection_pfx4() {
        let tag: u32 = kani::any();
        kani::assume(tag > 1);
        let mut buf = [0u8; 5]; // PodOption<u8, 4>: 4 tag + 1 value
        buf[..4].copy_from_slice(&tag.to_le_bytes());
        buf[4] = kani::any();
        let pod = unsafe { &*(buf.as_ptr() as *const PodOption<u8, 4>) };
        assert!(!pod.is_some(), "tag > 1 must not be Some");
    }

    // PFX=4: none() produces all-zero payload.
    #[kani::proof]
    fn none_zeroed_pfx4() {
        let pod = PodOption::<u8, 4>::none();
        let bytes = unsafe {
            core::slice::from_raw_parts(&pod as *const _ as *const u8, core::mem::size_of_val(&pod))
        };
        for &b in bytes {
            assert!(b == 0, "none() must produce all-zero bytes");
        }
    }

    // value_unchecked: forming &T is sound when value bytes are initialized.
    #[kani::proof]
    fn value_unchecked_reference_sound() {
        let v: u8 = kani::any();
        let pod = PodOption::<u8, 1>::some(v);
        let r = pod.value_unchecked();
        assert!(*r == v, "value_unchecked on Some must return the value");

        // Also safe on None — bytes are zeroed, u8 is valid for all patterns.
        let none_pod = PodOption::<u8, 1>::none();
        let r2 = none_pod.value_unchecked();
        assert!(*r2 == 0, "value_unchecked on None returns zeroed bytes");
    }
}
