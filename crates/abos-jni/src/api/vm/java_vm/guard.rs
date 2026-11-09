#[cfg(doc)]
use crate::{EnvUnowned, errors::Error, vm::JavaVM};

use super::{Env, JThrowable, Result, THREAD_ATTACHMENT, THREAD_GUARD_NEST_LEVEL, sys, sys_detach_current_thread};

/// Represents a JNI attachment of the current thread to a Java VM, which is
/// required before you can access the [`Env`] API.
///
/// [`AttachGuard`] is an `unsafe`, low-level building block for the safe thread
/// attachment APIs provided by [`JavaVM`]. Before considering this API you
/// should look at:
///
///   - [`JavaVM::attach_current_thread()`]
///   - [`JavaVM::attach_current_thread_for_scope()`]
///   - [`JavaVM::attach_current_thread_with_config()`]
///   - [`JavaVM::with_local_frame()`]
///   - [`JavaVM::with_top_local_frame()`]
///
/// More-specifically, an [`AttachGuard`] only represents an attachment for a
/// single JNI stack frame or local reference frame.
///
/// The lifetime for an [`AttachGuard`] effectively names the scope of a single
/// JNI stack frame. The lifetime of the guard is used to ensure that any local
/// references created via the [`Env`] API are only accessible within the
/// lifetime of the guard, which ensures that local references can't be used
/// outside of the JNI stack frame that owns them.
///
/// If the [`AttachGuard`] "owns" the underlying JNI thread attachment, that
/// means the guard will automatically detach the current thread from the Java
/// VM when the guard is dropped.
///
/// If you're implementing a JNI native method which is passed a raw
/// [`crate::sys::JNIEnv`] pointer, then you can get a corresponding guard via
/// [`AttachGuard::from_unowned`].
///
/// If you're implementing some JNI utility code that doesn't already have a raw
/// [`crate::sys::JNIEnv`] pointer you could use
/// [`JavaVM::attach_current_thread_guard`] to get an attachment guard.
///
/// # `JavaVM::singleton()` guarantee
///
/// If you know that at least one [`AttachGuard`] has ever existed (which is
/// implied if you have a [`Env`] reference) you can assume that
/// [`JavaVM::singleton()`] will return `Ok(JavaVM)`.
///
/// This can be useful if you need to access JNI from something like a Drop
/// implementation, without needing to explicitly copy `JavaVM` pointers. In
/// these cases you can often prove that at least one `AttachGuard` must have
/// existed in order to construct the object that will later be dropped.
///
/// # Safety
///
/// [`AttachGuard`] management is considered `unsafe` because there are some
/// safety rules that can't be automatically guaranteed through the Rust type
/// system alone...
///
/// 1. You must treat a guard as an immovable type that needs to live on the
///    stack and can't be re-ordered relative to other guards on the stack.
///
/// 2. Following from (1), an [`AttachGuard`] _MUST NOT_ be given a `'static`
///    lifetime (e.g. by boxing or moving into a `static` variable).
///
/// 3. Any low-level JNI code that creates new local references must
///    runtime-assert that it has exclusive access to the top-most
///    [`AttachGuard`] on the stack.
///
///    If you're only using the safe [`Env`] API then this is handled for you,
///    but if you're implementing low-level JNI code that somehow creates local
///    references without involving a checked `Env` API you must add fail-safe
///    runtime assertions. (See [`Env::assert_top`])
///
/// # Safety Guidance
///
/// If you do need to use an [`AttachGuard`] directly, then it's recommended
/// that the guard should be kept private to a function that uses so it isn't
/// ever exposed to safe code.
///
/// It's very important that [`AttachGuard`]s are never given a `'static`
/// lifetime or re-ordered relative to other guards on the stack and keeping the
/// guard private to a function is a good way to ensure that it remains fixed on
/// the stack.
///
/// Beware of making it too easy to repeatedly materialize access to a mutable
/// [`Env`] and increasing the risk that safe code could attempt to create new
/// local references with an [`Env`] reference that is not at the top of the
/// stack. If this happens the API will panic at runtime to avoid undefined
/// behavior.
///
/// # Panics
///
///    The `Drop` implementation will `panic` if a guard is not dropped in the
///    same order that it was created, relative to other guards (LIFO order).
#[derive(Debug)]
pub struct AttachGuard<'local> {
	pub(super) env: Env<'local>,
}

/// Increments the thread guard level, returning the new level.
fn thread_guard_level_push(env: *mut abos_jni_sys::JNIEnv) -> usize {
	THREAD_GUARD_NEST_LEVEL.with(|cell| {
		let level: usize = cell.get();
		if level == 0 {
			THREAD_ATTACHMENT.set(env);
		}
		cell.set(level + 1);
		level + 1
	})
}

/// Decrements the thread guard level, returning the new level.
fn thread_guard_level_pop() -> usize {
	let level: usize = THREAD_GUARD_NEST_LEVEL.with(|cell| {
		let level: usize = cell.get();
		assert_ne!(
			level, 0,
			"Spuriously dropped more AttachGuards than were known to exist"
		);
		cell.set(level - 1);
		level - 1
	});

	if level == 0 {
		THREAD_ATTACHMENT.set(std::ptr::null_mut());
	}

	level
}

impl<'local> AttachGuard<'local> {
	/// Wrap a raw [`sys::JNIEnv`] pointer in an `AttachGuard` that will detach
	/// the current thread on drop.
	///
	/// An owned `AttachGuard` can be used to implement "`for_scope`" thread
	/// attachments.
	///
	/// # Safety
	///
	/// The pointer must be non-null and correspond to a valid [`Env`] pointer
	/// that is attached to the current thread.
	///
	/// The returned guard must be managed according to the general
	/// [`AttachGuard`] "Safety" rules.
	///
	/// **IMPORTANTLY:** Never give the returned guard a `'static` lifetime; the
	/// guard must be treated as an immovable value on the stack and not be
	/// re-ordered relative to other guards on the stack.
	pub(super) unsafe fn from_owned(env: *mut sys::JNIEnv) -> Self {
		let level: usize = thread_guard_level_push(env);

		let guard: AttachGuard<'_> = Self {
			// SAFETY: The caller supplies the current thread's valid JNI environment, and the newly pushed level tracks this guard.
			env: unsafe { Env::new(env, level, true) },
		};

		// Guarantee that if you have an `AttachGuard` then
		// `JavaVM::singleton()` will always return `Ok(JavaVM)`
		let _vm: std::result::Result<crate::JavaVM, crate::errors::Error> = guard.env.get_java_vm();

		guard
	}

	/// Wrap a raw [`sys::JNIEnv`] pointer in an [`AttachGuard`] that does not
	/// own the underlying thread attachment and so it will **NOT** detach the
	/// current thread on drop.
	///
	/// This can be use when implementing native JNI methods (that are passed an
	/// attached [`sys::JNIEnv`] pointer) as a way to access the [`Env`] API.
	///
	/// It is recommended to use [`EnvUnowned`] instead of directly using this API
	/// since that will make it clearer that the guard is associated with the
	/// JNI stack frame lifetime that was passed to the native method.
	///
	/// # Safety
	///
	/// The pointer must be non-null and correspond to a valid [`Env`]
	/// pointer that is attached to the current thread.
	///
	/// The returned guard must be managed according to the general
	/// [`AttachGuard`] "Safety" rules.
	///
	/// **IMPORTANTLY:** Never give the returned guard a `'static` lifetime; the
	/// guard must be treated as an immovable value on the stack and not be
	/// re-ordered relative to other guards on the stack.
	pub unsafe fn from_unowned(env: *mut sys::JNIEnv) -> Self {
		let level: usize = thread_guard_level_push(env);
		let guard: AttachGuard<'_> = Self {
			// SAFETY: The caller supplies the current thread's valid JNI environment, and the newly pushed level tracks this guard.
			env: unsafe { Env::new(env, level, false) },
		};

		// Guarantee that if you have an `AttachGuard` then
		// `JavaVM::singleton()` will always return `Ok(JavaVM)`
		let _vm: std::result::Result<crate::JavaVM, crate::errors::Error> = guard.env.get_java_vm();

		guard
	}

	/// Returns true if the guard represents a scoped attachment that will also
	/// detach the thread when it is dropped.
	///
	/// Note that not all scoped guards from
	/// [`JavaVM::attach_current_thread_for_scope`] will own the attachment,
	/// since the scope may be nested under some other guard, lower on the stack
	/// that has already attached the thread.
	#[must_use]
	pub const fn owns_attachment(&self) -> bool {
		self.env.owns_attachment()
	}

	/// Borrows a mutable reference to the [`Env`] associated with this
	/// [`AttachGuard`], after asserting that this is the top-most
	/// guard on the stack.
	///
	/// # Panic
	///
	/// This will panic if the `AttachGuard` does not currently represent the
	/// top JNI stack frame.
	pub fn borrow_env_mut(&mut self) -> &mut Env<'local> {
		self.env.assert_top();
		&mut self.env
	}

	/// Handles detaching the current thread if the guards owns the attachment
	///
	/// # Safety
	///
	/// Since this is used as part of the `Drop` implementation then you must
	/// not allow the `Drop` implementation to run if this is called explicitly
	///
	/// Even though this only takes a reference, the implementation assumes that
	/// the guard is going to be dropped.
	unsafe fn detach_impl(&mut self, catch_exceptions: bool, throw: Option<JThrowable<'local>>) -> Result<()> {
		let caught_exception: std::result::Result<(), crate::errors::Error> = {
			let env: &mut Env<'_> = self.borrow_env_mut();
			let caught_exception: std::result::Result<(), crate::errors::Error> = if catch_exceptions {
				// note: we don't want to return errors here immediately because it's more important
				// that we do the book-keeping for the guard level and possibly detaching the thread
				env.exception_catch()
			} else {
				Ok(())
			};

			if let Some(throwable) = throw {
				drop(env.throw(throwable));
			}
			caught_exception
		};

		let new_level: usize = thread_guard_level_pop();
		assert_eq!(
			new_level + 1,
			self.env.level,
			"AttachGuard was dropped out-of-order with respect to other guards"
		);

		let detach_result: std::result::Result<(), crate::errors::Error> = if self.owns_attachment() {
			assert_eq!(
				new_level, 0,
				"Spurious AttachGuard that owns its attachment but is nested under another guard"
			);
			// SAFETY: This outermost owning guard has popped its level and will not use the environment after detaching.
			unsafe { sys_detach_current_thread(Some(self.env.raw), &std::thread::current()) }
		} else {
			Ok(())
		};

		match detach_result {
			Err(err) => Err(err),
			Ok(()) => caught_exception,
		}
	}

	/// Drop a guard explicitly and detach the current thread if the guard owns
	/// the current attachment.
	///
	/// Unlike [`AttachGuard::drop`] this returns a `Result` that can indicate
	/// potential JNI errors from attempting to detach the thread.
	///
	/// # Panics
	///
	/// This will panic if a guard is dropped out-of-order, with respect to
	/// other guards. Each [`AttachGuard`] created may be nested with respect
	/// to other guards and must be dropped or detached in LIFO order.
	///
	/// # Errors
	///
	/// Returns an error if JNI fails to detach an attachment owned by this guard.
	pub fn detach(mut self) -> Result<()> {
		// Safety: we're going to 'forget' the guard afterwards to ensure the
		// `Drop` implementation isn't run too.
		let res: std::result::Result<(), crate::errors::Error> = unsafe { self.detach_impl(false, None) };

		// We've effectively dropped the guard manually (so we can also get a `Result`)
		// but that means we shouldn't allow the `Drop` implementation to run too.
		std::mem::forget(self);

		res
	}

	/// Catch any exceptions and then Drop the guard + detach thread (if owned).
	///
	/// This is similar to [`AttachGuard::detach`], but will also catch (clear)
	/// any pending Java exceptions and return them as an
	/// [`Error::CaughtJavaException`].
	///
	/// Pending exceptions are caught, regardless of whether this is an "owned"
	/// attachment or not.
	///
	/// # Panics
	///
	/// This will panic if a guard is dropped out-of-order, with respect to
	/// other guards. Each [`AttachGuard`] created may be nested with respect
	/// to other guards and must be dropped or detached in LIFO order.
	///
	/// # Errors
	///
	/// Returns a caught Java exception or an error from detaching the owned attachment.
	pub fn detach_with_catch(mut self) -> Result<()> {
		// Safety: we're going to 'forget' the guard afterwards to ensure the
		// `Drop` implementation isn't run too.
		let res: std::result::Result<(), crate::errors::Error> = unsafe { self.detach_impl(true, None) };

		// We've effectively dropped the guard manually (so we can also get a `Result`)
		// but that means we shouldn't allow the `Drop` implementation to run too.
		std::mem::forget(self);

		res
	}

	/// Catch any exceptions, throw `throwable` and then Drop the guard + detach
	/// thread (if owned).
	///
	/// This is similar to [`AttachGuard::detach`], but will also catch (clear)
	/// any pending Java exceptions and then throw the provided `throwable`. Any
	/// caught exception will be returned as an [`Error::CaughtJavaException`].
	///
	/// This API can be used in case you have temporarily caught/cleared an
	/// exception to run JNI code and you then want to catch any exceptions for
	/// the run code before re-throwing the exception saved earlier.
	///
	/// # Panics
	///
	/// This will panic if a guard is dropped out-of-order, with respect to
	/// other guards. Each [`AttachGuard`] created may be nested with respect
	/// to other guards and must be dropped or detached in LIFO order.
	///
	/// # Errors
	///
	/// Returns a caught Java exception or an error from detaching the owned attachment.
	pub fn detach_with_catch_and_throw(mut self, throwable: Option<JThrowable<'local>>) -> Result<()> {
		// Safety: we're going to 'forget' the guard afterwards to ensure the
		// `Drop` implementation isn't run too.
		let res: std::result::Result<(), crate::errors::Error> = unsafe { self.detach_impl(true, throwable) };

		// We've effectively dropped the guard manually (so we can also get a `Result`)
		// but that means we shouldn't allow the `Drop` implementation to run too.
		std::mem::forget(self);

		res
	}
}

impl Drop for AttachGuard<'_> {
	fn drop(&mut self) {
		// SAFETY: Drop invokes the detach path once and does not use the environment afterward.
		if let Err(err) = unsafe { self.detach_impl(false, None) } {
			// This probably means that something `unsafe` happened to detach the thread already
			log::error!("Failed to detach current JNI thread: {err}");
		}
	}
}
