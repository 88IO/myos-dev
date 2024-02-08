use core::ops::Deref;

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
