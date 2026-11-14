use std::{borrow::Cow, ops::Deref};

use abos_jni_sys::jobject;
use log::{debug, warn};

use crate::{
	AttachConfig, Env, JavaVM,
	errors::{Error, Result},
	objects::{Global, JClass, JObject, LoaderContext},
	strings::JNIStr,
	sys,
};

use super::Reference;

// Weak must not expose its hidden object wrapper through AsRef or Deref: doing so
// would allow object access without first preventing collection. Into<JObject> would
// also allow Auto to release the weak reference with the wrong JNI deletion function.

/// An owned weak global reference that allows its Java object to be garbage collected.
///
/// # Upgrading
///
/// Before accessing the object, use [`Weak::upgrade_local`] or [`Weak::upgrade_global`].
/// They return `Ok(Some(reference))` if the object is still alive, `Ok(None)` if it
/// was collected, or an error if reference creation fails. Upgrading does not consume
/// the weak reference; it may be upgraded repeatedly.
///
/// `Weak` implements [`Reference`] for reference management, but not [`Deref`] or
/// [`AsRef`] to an object wrapper. Reference creation and [`Env::is_same_object`]
/// accept it directly; class, field, method, and content access require an upgrade.
///
/// # Creating and deleting
///
/// Use [`Env::new_weak_ref`] to create a weak reference or [`Weak::clone_in_jvm`] to
/// create another weak reference to the same object. Drop deletes the JNI reference.
/// Prefer dropping on an attached thread: otherwise cleanup temporarily attaches
/// the thread and logs a warning, adding attachment overhead.
#[repr(transparent)]
#[derive(Debug)]
pub struct Weak<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync,
{
	obj: T,
}

/// A temporary type alias to sign post that `WeakRef` has been renamed to `Weak`.
#[deprecated(since = "0.22.0", note = r#"Since 0.22, `WeakRef` has been renamed to `Weak`."#)]
pub type WeakRef<T> = Weak<T>;

// SAFETY: JNI global references are usable across threads; the wrapped type is Send + Sync and JNI access requires an Env.
unsafe impl<T> Send for Weak<T> where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync
{
}

// SAFETY: JNI global references are usable across threads; the wrapped type is Send + Sync and JNI access requires an Env.
unsafe impl<T> Sync for Weak<T> where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync
{
}

impl<T> Default for Weak<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync,
{
	fn default() -> Self {
		Self::null()
	}
}

impl<T> Weak<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync,
{
	/// Creates a new auto-delete wrapper for the `'static` weak global reference
	///
	/// Note: It's more likely that you want to look at the [`Env::new_weak_ref`] API instead
	/// of this, since you can't get `'static` reference types through safe APIs.
	///
	/// The [`Env`] reference here serves as proof that the current thread is attached, which
	/// implies [`JavaVM::singleton()`] is initialized, which is required by the `Drop`
	/// implementation.
	///
	/// # Safety
	///
	/// If the given reference is non-null, it must represent a weak global JNI reference.
	pub(crate) const unsafe fn new(_env: &Env, obj: T) -> Self {
		Self { obj }
	}

	/// Creates a [`Weak`] wrapper for a `null` reference
	///
	/// This is equivalent [`Weak::default()`]
	///
	/// A `null` [`Weak`] acts as-if the object has been garbage collected:
	/// [`Self::is_garbage_collected()`] returns `true`, and both
	/// [`Self::upgrade_local`] and [`Self::upgrade_global`] return `None`.
	#[must_use]
	pub fn null() -> Self {
		Self { obj: T::default() }
	}

	/// Returns the raw JNI weak reference.
	pub fn as_raw(&self) -> sys::jweak {
		self.obj.as_raw()
	}

	/// Creates a new local reference to this object.
	///
	/// This returns `None` if this weak reference is null or the object has already been garbage
	/// collected, otherwise it returns `Some(new_local_reference)`.
	///
	/// If this method returns `Some(r)`, it is guaranteed that the object will not be garbage
	/// collected at least until `r` is deleted or becomes invalid.
	///
	/// # Errors
	///
	/// Returns an error if a local reference cannot be allocated or a Java exception is pending.
	pub fn upgrade_local<'local>(&self, env: &mut Env<'local>) -> Result<Option<T::Kind<'local>>> {
		match env.new_local_ref(self) {
			Ok(local_ref) if local_ref.is_null() => Ok(None),
			Ok(local_ref) => Ok(Some(local_ref)),
			Err(Error::ObjectFreed) => Ok(None),
			Err(err) => Err(err),
		}
	}

	/// Creates a new strong global reference to this object.
	///
	/// This returns `None` if this weak reference is null or the object has already been garbage
	/// collected, otherwise it returns `Some(new_global_reference)`.
	///
	/// If this method returns `Some(r)`, it is guaranteed that the object will not be garbage
	/// collected at least until `r` is deleted or becomes invalid.
	///
	/// # Errors
	///
	/// Returns an error if a global reference cannot be allocated or a Java exception is pending.
	pub fn upgrade_global(&self, env: &Env) -> Result<Option<Global<T::GlobalKind>>> {
		match env.new_global_ref(self) {
			Ok(global_ref) if global_ref.is_null() => Ok(None),
			Ok(global_ref) => Ok(Some(global_ref)),
			Err(Error::ObjectFreed) => Ok(None),
			Err(err) => Err(err),
		}
	}

	/// Checks if the object referred to by this `Weak` has been garbage collected.
	///
	/// Note that garbage collection can happen at any moment, so a return of `Ok(false)` from this
	/// method does not guarantee that [`Weak::upgrade_local`] or [`Weak::upgrade_global`]
	/// will succeed.
	///
	/// This is equivalent to
	/// <code>self.[is_same_object][Weak::is_same_object](env, [JObject::null]\())</code>.
	///
	/// This may return [`Error::JavaException`] if called while there is a pending exception.
	///
	/// # Errors
	///
	/// Returns an error if a Java exception is pending while comparing the references.
	pub fn is_garbage_collected(&self, env: &Env) -> Result<bool> {
		env.is_same_object(self, JObject::null())
	}

	/// Returns true if this weak reference refers to the given object. Otherwise returns false.
	///
	/// If `object` is [null][JObject::null], then this method is equivalent to
	/// [`Weak::is_garbage_collected`]: it returns true if the object referred to by this
	/// `Weak` has been garbage collected, or false if the object has not yet been garbage
	/// collected.
	#[deprecated = "Use Env::is_same_object"]
	///
	/// # Errors
	///
	/// Returns an error if a Java exception is pending while comparing the references.
	pub fn is_same_object<'local, O>(&self, env: &Env<'local>, object: O) -> Result<bool>
	where
		O: AsRef<JObject<'local>>,
	{
		env.is_same_object(self, object.as_ref())
	}

	/// Returns true if this weak reference refers to the same object as another weak reference.
	/// Otherwise returns false.
	///
	/// This method will also return true if both weak references refer to an object that has been
	/// garbage collected.
	#[deprecated = "Use Env::is_same_object"]
	///
	/// # Errors
	///
	/// Returns an error if a Java exception is pending while comparing the references.
	pub fn is_weak_ref_to_same_object(&self, env: &Env, other: &Self) -> Result<bool> {
		env.is_same_object(self, other)
	}

	/// Creates a new weak reference to the same object that this one refers to.
	///
	/// This method returns `None` if the object has already been garbage collected.
	///
	/// # Errors
	///
	/// Returns an error if a new weak reference cannot be allocated or a Java exception is pending.
	pub fn clone_in_jvm(&self, env: &mut Env<'_>) -> Result<Option<Weak<T::GlobalKind>>> {
		match env.new_weak_ref(self) {
			Err(Error::ObjectFreed) => Ok(None),
			Err(err) => Err(err),
			Ok(weak_ref) => Ok(Some(weak_ref)),
		}
	}
}

impl<T> Drop for Weak<T>
where
	T: Into<JObject<'static>> + AsRef<JObject<'static>> + Default + Reference + Send + Sync,
{
	fn drop(&mut self) {
		let obj: T = std::mem::take(&mut self.obj);

		// It's redundant to explicitly call DeleteWeakGlobalRef with a null pointer and we don't
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
					"Dropping a Weak in a detached thread. Fix your code if this message appears frequently (see the Weak docs)."
				);
			}

			// Safety: This method is safe to call in case of pending exceptions (see chapter 2 of the spec)
			// jni-rs requires JNI_VERSION > 1.2
			unsafe {
				ex_safe_jni_call_no_post_check_ex!(env, v1_2, DeleteWeakGlobalRef, obj.as_raw());
			}
			Ok(())
		};

		if let Err(err) = exception_safe_delete() {
			debug!("error dropping weak ref: {err:#?}");
		}
	}
}

// SAFETY: Kind and GlobalKind are implicitly transparent wrappers if T is
// implemented correctly / safely.
unsafe impl<T> Reference for Weak<T>
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
		unsafe { T::kind_from_raw(local_ref) }
	}

	unsafe fn global_kind_from_raw(global_ref: jobject) -> Self::GlobalKind {
		// SAFETY: The caller guarantees the global reference type required by the delegated constructor.
		unsafe { T::global_kind_from_raw(global_ref) }
	}
}

#[cfg(test)]
#[path = "../../../tests/unit/api/refs/weak.rs"]
mod tests;
