use core::mem::MaybeUninit;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PodOption<T: Copy> {
    tag: u8,
    value: MaybeUninit<T>,
}

const _: () = assert!(core::mem::align_of::<PodOption<u8>>() == 1);

impl<T: Copy> PodOption<T> {
    #[inline(always)]
    pub fn none() -> Self {
        Self {
            tag: 0,
            value: MaybeUninit::zeroed(),
        }
    }
    #[inline(always)]
    pub fn some(value: T) -> Self {
        Self {
            tag: 1,
            value: MaybeUninit::new(value),
        }
    }
    #[inline(always)]
    pub fn is_some(&self) -> bool {
        self.tag == 1
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
    #[inline(always)]
    pub fn set(&mut self, value: Option<T>) {
        match value {
            Some(v) => {
                self.tag = 1;
                self.value = MaybeUninit::new(v);
            }
            None => {
                self.tag = 0;
                self.value = MaybeUninit::zeroed();
            }
        }
    }
    pub fn raw_tag(&self) -> u8 {
        self.tag
    }

    /// # Safety
    /// Caller must ensure tag == 1 (Some).
    #[inline(always)]
    pub unsafe fn assume_init_ref(&self) -> &T {
        self.value.assume_init_ref()
    }

    pub fn take(&mut self) -> Option<T> {
        let result = self.get();
        self.tag = 0;
        self.value = MaybeUninit::zeroed();
        result
    }

    pub fn replace(&mut self, value: T) -> Option<T> {
        let old = self.get();
        self.tag = 1;
        self.value = MaybeUninit::new(value);
        old
    }

    pub fn clear(&mut self) {
        self.tag = 0;
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

impl<T: Copy> Default for PodOption<T> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T: Copy + PartialEq> PartialEq for PodOption<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self.get(), other.get()) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Copy + Eq> Eq for PodOption<T> {}

impl<T: Copy + PartialEq> PartialEq<Option<T>> for PodOption<T> {
    fn eq(&self, other: &Option<T>) -> bool {
        match (self.get(), other) {
            (Some(a), Some(b)) => a == *b,
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Copy + core::fmt::Debug> core::fmt::Debug for PodOption<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.get() {
            Some(v) => write!(f, "Some({:?})", v),
            None => write!(f, "None"),
        }
    }
}
