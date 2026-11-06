#[cfg(feature = "invocation")]
mod init_args;
#[cfg(feature = "invocation")]
pub use self::init_args::*;

mod java_vm;
pub use self::java_vm::{
	AttachConfig, AttachGuard, AttachmentExceptionPolicy, DEFAULT_LOCAL_FRAME_CAPACITY, JavaVM, ScopeToken,
};
pub(crate) use self::java_vm::{sys_attach_current_thread, sys_detach_current_thread};
