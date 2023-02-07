#![no_std]

pub mod elf;

#[cfg(feature = "tls")]
mod tls;

#[cfg(feature = "unwinding")]
pub mod unwinding;

#[cfg(feature = "embedded-phdrs")]
mod embedded;

#[cfg(feature = "injected-phdrs")]
mod injected;

#[cfg(feature = "embedded-phdrs")]
pub use embedded::EmbeddedProgramHeaders;

#[cfg(feature = "injected-phdrs")]
pub use injected::InjectedProgramHeaders;

use elf::ProgramHeader;

pub trait InnerProgramHeadersFinder {
    fn find_phdrs(&self) -> &[ProgramHeader];
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProgramHeadersFinder<T>(T);

impl<T> ProgramHeadersFinder<T> {
    pub const fn new(inner: T) -> Self {
        Self(inner)
    }
}

impl<T: InnerProgramHeadersFinder> ProgramHeadersFinder<T> {
    pub fn find_phdrs(&self) -> &[ProgramHeader] {
        self.0.find_phdrs()
    }
}