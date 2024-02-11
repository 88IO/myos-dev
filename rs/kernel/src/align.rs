use core::ops::{Deref, DerefMut};

#[repr(align(64))]
pub struct Aligned64<T>(pub T);

impl<T> Aligned64<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for Aligned64<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Aligned64<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
