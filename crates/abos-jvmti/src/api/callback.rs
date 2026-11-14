use std::panic::{AssertUnwindSafe, catch_unwind};

/// Runs a native callback body, returning an explicit fallback if it unwinds.
///
/// This boundary does not attach threads, access JNI/JVMTI, throw Java exceptions, log,
/// or install a panic hook. It can wrap restricted callbacks, but the body and its
/// destructors must still obey that callback's execution-context rules.
///
/// Choose the fallback for the particular callback: `()` for a notification,
/// [`sys::JVMTI_VISIT_ABORT`](crate::sys::JVMTI_VISIT_ABORT) for a modern heap visitor,
/// [`sys::JVMTI_ITERATION_ABORT`](crate::sys::JVMTI_ITERATION_ABORT) for a legacy heap
/// visitor, or [`sys::JNI_ERR`](crate::sys::JNI_ERR) for agent initialization. There is
/// deliberately no generic `Default` policy. Ordinary errors are not panics; handle
/// them inside `body` before returning its ABI result.
///
/// # Panic containment
///
/// Caught panic payloads are **intentionally leaked**, without formatting or dropping
/// them. An arbitrary payload destructor could panic again or perform operations
/// forbidden in a restricted callback. Repeated panics can therefore leak memory;
/// this is a last-resort boundary, not a normal error-handling mechanism.
///
/// This catches unwinding Rust panics only. It cannot recover from `panic = "abort"`,
/// a panic during unwinding, an aborting/panicking panic hook, or foreign exceptions.
/// The existing process-wide panic hook runs **before** the unwind is caught and must
/// itself be suitable for the execution context. The boundary does not promise an
/// allocation-free or lock-free panic path.
///
/// Catching a panic does not roll back mutations or repair shared state. The body is
/// treated as unwind-safe at this outer FFI boundary; the caller must arrange that
/// partially completed work cannot corrupt subsequent callbacks. In particular,
/// publish output pointers and lengths only after all fallible work succeeds.
///
/// Only `Copy` results are accepted so discarding a fallback or result cannot run a
/// user destructor outside the boundary. Construct closure captures and fallback
/// values without panicking, and keep any other potentially panicking work inside
/// the boundary rather than before or after it in the native entry point.
///
/// # Examples
///
/// ```
/// use abos_jvmti::{catch_callback, sys};
///
/// // A heap callback must abort traversal on panic, not choose a generic zero value.
/// let flags = catch_callback(sys::JVMTI_VISIT_ABORT, || {
///     // Inspect callback metadata here, without attaching or invoking Java.
///     sys::JVMTI_VISIT_OBJECTS
/// });
/// assert_eq!(flags, sys::JVMTI_VISIT_OBJECTS);
/// ```
pub fn catch_callback<R: Copy>(fallback: R, body: impl FnOnce() -> R) -> R {
	match catch_unwind(AssertUnwindSafe(body)) {
		Ok(result) => result,
		Err(payload) => {
			// Even dropping the original payload may execute arbitrary, context-unsafe code.
			std::mem::forget(payload);
			fallback
		}
	}
}

/// Runs a callback with a protected panic handler and an explicit fallback result.
///
/// On a body panic, the payload is leaked and `on_panic` is called once. Whether the
/// handler returns or panics, the boundary returns `fallback`. The handler receives
/// no payload, preventing accidental formatting or destruction of arbitrary payloads.
/// Dropping unused handler captures is also protected: if that drop panics after a
/// successful body, the boundary returns `fallback` without calling the handler.
///
/// All restrictions and limitations of [`catch_callback`] apply. The handler runs
/// on the **same thread and in the same restricted context** as the body. It must
/// not assume it can attach a thread, throw a Java exception, acquire a lock, or log.
/// A simple atomic failure flag can defer reporting to a permitted context.
///
/// # Examples
///
/// ```
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use abos_jvmti::catch_callback_with;
///
/// let failed = AtomicBool::new(false);
/// catch_callback_with((), || {
///     // Notification callback work.
/// }, || {
///     failed.store(true, Ordering::Relaxed);
/// });
/// assert!(!failed.load(Ordering::Relaxed));
/// ```
pub fn catch_callback_with<R: Copy>(fallback: R, body: impl FnOnce() -> R, on_panic: impl FnOnce()) -> R {
	// The outer boundary also contains handler execution and destruction of unused captures.
	catch_callback(fallback, || match catch_unwind(AssertUnwindSafe(body)) {
		Ok(result) => result,
		Err(payload) => {
			std::mem::forget(payload);
			on_panic();
			fallback
		}
	})
}
