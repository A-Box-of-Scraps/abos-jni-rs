use std::{borrow::Cow, ops::Deref};

use abos_jni_sys::jobject;
use log::{debug, warn};

use crate::{
	AttachConfig, Env, JavaVM,
	objects::{JClass, JObject, LoaderContext},
	strings::JNIStr,
	sys,
};

#[cfg(doc)]
use crate::objects::Weak;

use super::Reference;

// Note: `Global` must not implement `Into<JObject>`! If it did, then it would be possible to
// wrap it in `Auto`, which would cause undefined behavior upon drop as a result of calling
// the wrong JNI function to delete the reference.

/// An owned global reference that keeps a Java object alive until dropped.
///
/// Global references outlive local JNI frames and implement [`Send`] and [`Sync`].
/// Use them to retain objects across native calls or share references between threads.
/// Each thread still needs an attachment to call JNI; sharing a reference does not
/// make the Java object's operations thread-safe. See [`Weak`] for a reference that
/// permits garbage collection.
///
/// Create one with [`Env::new_global_ref`] or adopt an owned raw global reference
/// with [`Env::global_from_raw`]. For a borrowed raw reference, use [`Env::as_cast_raw`]
/// instead so the wrapper does not delete a reference it does not own.
///
/// # Drop and thread attachment
///
/// Drop deletes the JNI reference. Prefer dropping on an attached thread: otherwise
/// cleanup temporarily attaches the thread and logs a warning, adding attachment overhead.
#[repr(transparent)]
#[derive(Debug)]
pub struct Global<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static,
{
	obj: T,
}

/// A temporary type alias to sign post that `GlobalRef` has been renamed to `Global`.
#[deprecated(since = "0.22.0", note = r#"Since 0.22, `GlobalRef` has been renamed to `Global`."#)]
pub type GlobalRef<T> = Global<T>;

// SAFETY: JNI global references are usable across threads; the wrapped type is Send + Sync and JNI access requires an Env.
unsafe impl<T> Send for Global<T> where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static
{
}

// SAFETY: JNI global references are usable across threads; the wrapped type is Send + Sync and JNI access requires an Env.
unsafe impl<T> Sync for Global<T> where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static
{
}

impl<T> Default for Global<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static,
{
	fn default() -> Self {
		Self::null()
	}
}

impl<T, U> AsRef<U> for Global<T>
where
	T: AsRef<U> + Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync,
{
	fn as_ref(&self) -> &U {
		self.obj.as_ref()
	}
}

impl<T> Deref for Global<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.obj
	}
}

impl<T> Global<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static,
{
	/// Creates a new auto-delete wrapper for the `'static` global reference
	///
	/// Note: It's more likely that you want to look at the [`Env::new_global_ref`] API instead
	/// of this, since you can't get `'static` reference types through safe APIs.
	///
	/// The [`Env`] reference here serves as proof that the current thread is attached, which
	/// implies [`JavaVM::singleton()`] is initialized, which is required by the `Drop`
	/// implementation.
	///
	/// # Safety
	///
	/// If the given reference is non-null, it must represent a global JNI reference.
	pub(crate) const unsafe fn new(_env: &Env, obj: T) -> Self {
		Self { obj }
	}

	/// Creates a [`Global`] wrapper for a `null` reference
	///
	/// This is equivalent [`Global::default()`]
	#[must_use]
	pub fn null() -> Self {
		Self { obj: T::default() }
	}

	/// Unwrap to the auto-delete global reference into a raw global `jobject`.
	///
	/// This prevents the global reference from being automatically deleted when
	/// this guard is dropped.
	///
	/// # Leaking References
	///
	/// When unwrapping a [`Global`] you should consider how else you will
	/// ensure that the reference will get deleted.
	///
	/// The global reference may end up leaking unless a new [`Global`] wrapper
	/// is create later, or you find some way to call the JNI `DeleteGlobalRef`
	/// API on the raw reference.
	///
	/// **Note:** You should not find yourself unwrapping a raw global `jobject`
	/// that you do not logically own. If you have a raw global `jobject` that
	/// you do not own, you should instead use [`Env::as_cast_raw`] to create a
	/// temporary wrapper.
	pub fn into_raw(mut self) -> sys::jobject {
		let obj: T = std::mem::take(&mut self.obj); // Leave a `Default/null`` reference in self.obj that doesn't need to be deleted
		std::mem::forget(self); // Skip redundant Drop for `Default/null` reference
		let obj: JObject = obj.into();
		obj.into_raw()
	}

	#[doc(hidden)]
	#[deprecated(since = "0.22.0", note = r#"Replaced with `into_raw` since it is infallible"#)]
	pub fn try_into_raw(self) -> std::result::Result<sys::jobject, std::convert::Infallible> {
		Ok(self.into_raw())
	}

	/// Borrows a `JObject` referring to the same Java object as this
	/// `Global`.
	///
	/// This method is zero-cost and does not create a new local reference.
	///
	/// `Global` also implements <code>[AsRef]&lt;[JObject]&gt;</code>.
	/// That trait's `as_ref` method does the same thing as this method.
	pub fn as_obj(&self) -> &JObject<'static> {
		self.as_ref()
	}
}

impl<T> Drop for Global<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync + 'static,
{
	fn drop(&mut self) {
		let obj: T = std::mem::take(&mut self.obj);

		// It's redundant to explicitly call DeleteGlobalRef with a null pointer and we don't
		// assume that a JavaVM has been initialized if we only wrap a 'static null pointer
		if obj.is_null() {
			return;
		}
		// Panic: If we have a non-null reference, we know JavaVM::singleton() must have been
		// initialized (and can't return an error) because ::new() takes a Env reference.
		let vm: JavaVM = JavaVM::singleton().expect("JavaVM singleton uninitialized");

		// Use a raw attachment guard so cleanup leaves pending exceptions untouched.
		let exception_safe_delete = || -> abos_jni::errors::Result<()> {
			let mut scope: crate::ScopeToken = abos_jni::ScopeToken::default();
			let mut guard: crate::AttachGuard<'_> =
				// SAFETY: The scope token and guard remain on this stack until the reference has been released.
				unsafe { vm.attach_current_thread_guard(|| AttachConfig::new().scoped(true), &mut scope)? };
			let env: &mut Env<'_> = guard.borrow_env_mut();
			// If the Env is borrowing from an AttachGuard that owns the current thread
			// attachment that means the thread was not already attached
			if env.owns_attachment() {
				warn!(
					"A JNI global reference was dropped on a thread that is not attached. This will cause a performance problem if it happens frequently. For more information, see the documentation for `abos_jni::objects::Global`."
				);
			}
			// Safety: This method is safe to call in case of pending exceptions (see chapter 2 of the spec)
			unsafe {
				ex_safe_jni_call_no_post_check_ex!(env, v1_1, DeleteGlobalRef, obj.as_raw());
			}
			Ok(())
		};

		if let Err(err) = exception_safe_delete() {
			debug!("error dropping global ref: {err:#?}");
		}
	}
}

// SAFETY: Kind and GlobalKind are implicitly transparent wrappers if T is
// implemented correctly / safely.
unsafe impl<T> Reference for Global<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync,
{
	type Kind<'env> = T::Kind<'env>;
	type GlobalKind = T::GlobalKind;

	fn as_raw(&self) -> jobject {
		self.obj.as_raw()
	}

	fn class_name() -> Cow<'static, JNIStr> {
		T::class_name()
	}

	fn lookup_class<'caller>(
		env: &Env<'_>,
		loader_context: &LoaderContext,
	) -> crate::errors::Result<impl Deref<Target = Global<JClass<'static>>> + 'caller> {
		T::lookup_class(env, loader_context)
	}

	unsafe fn kind_from_raw<'env>(local_ref: jobject) -> Self::Kind<'env> {
		// SAFETY: The caller guarantees the local reference type and lifetime required by the delegated constructor.
		unsafe { T::kind_from_raw::<'env>(local_ref) }
	}

	unsafe fn global_kind_from_raw(global_ref: jobject) -> Self::GlobalKind {
		// SAFETY: The caller guarantees the global reference type required by the delegated constructor.
		unsafe { T::global_kind_from_raw(global_ref) }
	}
}

#[cfg(test)]
#[path = "../../../tests/unit/api/refs/global.rs"]
mod tests;
