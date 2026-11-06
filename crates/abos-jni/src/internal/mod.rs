#[macro_use]
pub mod macros;
#[cfg(windows)]
#[allow(
	non_snake_case,
	non_upper_case_globals,
	non_camel_case_types,
	dead_code,
	clippy::all,
	reason = "Generated Windows bindings retain the SDK names and declarations."
)]
pub mod windows_sys;

#[cfg(use_fls_attach_guard)]
pub mod fls_attach_guard;
#[cfg(use_tls_attach_guard)]
pub mod tls_attach_guard;
