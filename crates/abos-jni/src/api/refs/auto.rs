use std::{borrow::Cow, marker::PhantomData, mem::ManuallyDrop, ops::Deref, ptr};

use abos_jni_sys::jobject;

use crate::{
	Env, JavaVM,
	objects::{Global, JClass, JObject, LoaderContext},
	strings::JNIStr,
};

use super::Reference;

/// Deletes an owned local reference when dropped, before its JNI frame ends.
///
/// Local references are otherwise released when their frame ends. Use `Auto` for
/// temporary references in long-running loops, or [`Env::with_local_frame`] to
/// release a group of references together. [`Env::delete_local_ref`] deletes one explicitly.
///
/// Wrap a local reference with [`Auto::new`] or the [`super::IntoAuto::auto`] method:
///
/// For example:
///
/// ```rust,no_run
/// # use abos_jni::{objects::Auto, Env};
/// # fn example(env: &mut Env) -> std::result::Result<(), abos_jni::errors::Error> {
/// use abos_jni::objects::IntoAuto as _;
/// for i in 0..1000 {
///     // Ensure we aren't left with a new local for each iteration by
///     // wrapping the reference in an `Auto` wrapper.
///     let auto_delete_string = env.new_string("Hello, world!")?.auto();
/// }
/// # Ok(())
/// # }
/// ```
///
#[derive(Debug)]
pub struct Auto<'local, T>
where
	T: Into<JObject<'local>>,
{
	obj: ManuallyDrop<T>,
	_lifetime: PhantomData<&'local T>,
}

/// Deprecated alias for [`Auto`].
#[deprecated(since = "0.22.0", note = "AutoLocal has been renamed to `Auto`")]
pub type AutoLocal<'local, T> = Auto<'local, T>;

impl<'local, T> Auto<'local, T>
where
	// Note that this bound prevents `Auto` from wrapping a `Global`, which implements
	// `AsRef<JObject<'static>>` but *not* `Into<JObject<'static>>`. This is good, because trying
	// to delete a global reference as though it were local would cause undefined behavior.
	T: Into<JObject<'local>>,
{
	/// Creates a new auto-delete wrapper for a local ref.
	///
	/// Once this wrapper goes out of scope, the `delete_local_ref` will be
	/// called on the object. While wrapped, the object can be accessed via
	/// the `Deref` impl.
	pub const fn new(obj: T) -> Self {
		Auto {
			obj: ManuallyDrop::new(obj),
			_lifetime: PhantomData,
		}
	}

	/// Returns the local reference without deleting it.
	///
	/// The reference remains owned by its JNI frame. Release it explicitly or let
	/// the frame end; repeated unwrapping in a long-lived frame can exhaust local references.
	pub fn unwrap(self) -> T {
		// Suppress the wrapper's destructor before moving out the reference.
		let self_md: ManuallyDrop<Auto<'_, T>> = ManuallyDrop::new(self);

		// SAFETY: ManuallyDrop prevents a second drop, and the consumed wrapper is not accessed after moving out its object.
		unsafe { ptr::read(&raw const *self_md.obj) }
	}

	/// Unwrap the RAII, auto-delete wrapper, returning the original local reference.
	///
	/// See [`Self::unwrap`]
	#[deprecated = "Renamed to Auto::unwrap"]
	pub fn forget(self) -> T {
		self.unwrap()
	}
}

impl<'local, T> Drop for Auto<'local, T>
where
	T: Into<JObject<'local>>,
{
	fn drop(&mut self) {
		// Extract the local reference from `self.obj` so that we can delete it.
		//
		// This is needed because it is not allowed to move out of `self` during drop. A safe
		// alternative would be to wrap `self.obj` in `Option`, but that would incur a run-time
		// performance penalty from constantly checking if it's `None`.
		//
		// Safety: `self.obj` is not used again after this `take` call.
		let obj: T = unsafe { ManuallyDrop::take(&mut self.obj) };
		let obj: JObject = obj.into();

		JavaVM::delete_local_ref(obj);
	}
}

impl<'local, T, U> AsRef<U> for Auto<'local, T>
where
	T: AsRef<U> + Into<JObject<'local>>,
{
	fn as_ref(&self) -> &U {
		self.obj.as_ref()
	}
}

impl<'local, T> Deref for Auto<'local, T>
where
	T: Into<JObject<'local>>,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.obj
	}
}

/// A trait for wrapping a local reference type into an [`Auto`]
pub trait IntoAuto<'local>: Sized + Into<JObject<'local>> {
	/// Wraps the local reference type into an auto-delete [`Auto`] that will
	/// automatically delete the local reference when it is dropped
	fn auto(self) -> Auto<'local, Self> {
		Auto::new(self)
	}
}

impl<'local, T> IntoAuto<'local> for T where T: Into<JObject<'local>> {}

impl<'local, T> From<T> for Auto<'local, T>
where
	T: Into<JObject<'local>>,
{
	fn from(value: T) -> Self {
		Auto::new(value)
	}
}

// SAFETY: Kind and GlobalKind are implicitly transparent wrappers if T is
// implemented correctly / safely.
unsafe impl<'local, T> Reference for Auto<'local, T>
where
	T: Reference + Into<JObject<'local>>,
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
