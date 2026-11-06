#[macro_use]
pub mod macros;
#[cfg(windows)]
pub(crate) mod windows_sys;

#[cfg(use_fls_attach_guard)]
pub(crate) mod fls_attach_guard;
#[cfg(use_tls_attach_guard)]
pub mod tls_attach_guard;
