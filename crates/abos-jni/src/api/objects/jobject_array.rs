use std::{
	any::{Any, TypeId},
	borrow::Cow,
	collections::HashMap,
	ops::Deref,
	sync::{OnceLock, RwLock},
};

use crate::{
	DEFAULT_LOCAL_FRAME_CAPACITY, Env, JavaVM,
	errors::{Error, JniError, Result},
	objects::{Global, JClass, JObject, LoaderContext, Reference},
	strings::{JNIStr, JNIString},
	sys::{jobject, jobjectArray},
};

use super::AsJArrayRaw;

/// A `java.lang.Object[]` reference, tied to a JNI local reference frame.
///
/// See the [`JObject`] documentation for more information about reference
/// wrappers, how to cast them, and local reference frame lifetimes.
///
/// # Example
///
/// ```rust,no_run
/// # use abos_jni::{errors::Result, Env, objects::{JObjectArray, JString}};
/// # fn example<'local>(env: &mut Env<'local>) -> Result<()> {
/// // Create some strings
/// let str1 = JString::from_str(env, "hello")?;
/// let str2 = JString::from_str(env, "world")?;
///
/// // Create an array of 2 strings, with all elements initially set to str1
/// let arr = JObjectArray::<JString>::new(env, 2, &str1)?;
///
/// // Set the second element to str2
/// arr.set_element(env, 1, &str2)?;
///
/// // Get elements from the array
/// let element0: JString = arr.get_element(env, 0)?;
/// let element1: JString = arr.get_element(env, 1)?;
///
/// assert_eq!(element0.to_string(), "hello");
/// assert_eq!(element1.to_string(), "world");
///
/// // Get the length of the array
/// let len = arr.len(env)?;
/// assert_eq!(len, 2);
/// # Ok(())
/// # }
/// ```
#[repr(transparent)]
#[derive(Debug, Default)]
pub struct JObjectArray<'local, E: Reference + 'local = JObject<'local>> {
	array: JObject<'local>,
	_marker: std::marker::PhantomData<E>,
}

impl<E: Reference> AsRef<Self> for JObjectArray<'_, E> {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl<'local, E: Reference> AsRef<JObject<'local>> for JObjectArray<'local, E> {
	fn as_ref(&self) -> &JObject<'local> {
		self
	}
}

impl<'local, E: Reference> ::std::ops::Deref for JObjectArray<'local, E> {
	type Target = JObject<'local>;

	fn deref(&self) -> &Self::Target {
		&self.array
	}
}

impl<'local, E: Reference> From<JObjectArray<'local, E>> for JObject<'local> {
	fn from(other: JObjectArray<'local, E>) -> Self {
		other.array
	}
}

// SAFETY: JObjectArray contains only an object-array reference with the declared local lifetime.
unsafe impl<'local, E: Reference> AsJArrayRaw<'local> for JObjectArray<'local, E> {}

pub struct JObjectArrayAPI<E: Reference> {
	class: Global<JClass<'static>>,
	_marker: std::marker::PhantomData<E>,
}

// Unlike other Reference types, JObjectArray is generic and we can't simply use
// a static `OnceCell` to cache state since Rust statics can't be generic.
static API_REGISTRY: OnceLock<RwLock<HashMap<TypeId, &'static (dyn Any + Send + Sync)>>> = OnceLock::new();

impl<E: Reference + Send + Sync> JObjectArrayAPI<E> {
	pub(crate) fn get(env: &Env<'_>, loader_context: &LoaderContext<'_, '_>) -> Result<&'static Self> {
		let map: &RwLock<HashMap<TypeId, &(dyn Any + Send + Sync)>> =
			API_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()));
		let tid: TypeId = TypeId::of::<Self>();

		// Fast path (read-lock)
		if let Some(any_ref) = map.read().expect("array binding cache is not poisoned").get(&tid) {
			// Stored as &'static dyn Any; downcast back to &'static JObjectArrayAPI<E>
			return Ok(any_ref
				.downcast_ref::<Self>()
				.expect("TypeId matched but downcast failed"));
		}

		// Slow path: do the class lookup and cache

		// So we can avoid holding any lock while doing (slow) JNI lookups, then
		// in the unlikely case that another thread is also trying to look up
		// the same state then we let them race and keep the first one to finish.

		let created: Self = {
			env.with_local_frame(DEFAULT_LOCAL_FRAME_CAPACITY, |env| -> Result<_> {
				// Derive the array class from the resolved element class, not its name:
				// E may already be bound to a different loader than loader_context.
				let element_class: &Global<JClass<'static>> = &*E::lookup_class(env, loader_context)?;
				let element_class: &JClass<'_> = element_class.as_ref();
				let array: JObjectArray<'_> = env.new_object_array(0, element_class, JObject::null())?;
				let class: JClass<'_> = env.get_object_class(&array)?;
				let class: Global<JClass<'_>> = env.new_global_ref(&class)?;
				Ok(Self {
					class,
					_marker: std::marker::PhantomData,
				})
			})?
		};

		let mut write: std::sync::RwLockWriteGuard<'_, HashMap<TypeId, &(dyn Any + Send + Sync)>> =
			map.write().expect("array binding cache is not poisoned");

		// Another thread might have inserted while we acquired the write lock:
		if let Some(any_ref) = write.get(&tid) {
			let api: &Self = any_ref
				.downcast_ref::<Self>()
				.expect("TypeId matched but downcast failed");
			return Ok(api);
		}

		// Leak it to get a true 'static reference and erase the type for storage
		let leaked: &'static Self = Box::leak(Box::new(created));
		write.insert(tid, leaked as &'static (dyn Any + Send + Sync));
		drop(write);

		Ok(leaked)
	}
}

impl<'local, E: Reference + 'local> JObjectArray<'local, E> {
	/// Creates a new [`JObjectArray`] of the given `length`, with each element initialized to
	/// `initial_element`.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use abos_jni::{errors::Result, Env, objects::{JObjectArray, JString}};
	/// # fn example<'local>(env: &mut Env<'local>) -> Result<()> {
	/// let str = JString::from_str(env, "hello")?;
	/// let arr = JObjectArray::<JString>::new(env, 5, &str)?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns an error if the length is too large, the element class cannot be resolved, or the JVM cannot allocate the array.
	pub fn new<'env_local, 'any_local>(
		env: &mut Env<'env_local>,
		length: usize,
		initial_element: impl AsRef<E::Kind<'any_local>>,
	) -> Result<JObjectArray<'env_local, E::Kind<'env_local>>> {
		let length: i32 =
			crate::sys::jsize::try_from(length).map_err(|_| Error::JniCall(JniError::InvalidArguments))?;
		env.assert_top();
		// Borrowing the lookup result keeps its guard alive through the JNI call.
		let element_class: &Global<JClass<'static>> = &*E::lookup_class(env, &LoaderContext::default())?;
		// SAFETY: The element class is resolved through Reference, and the initial value has the matching element type.
		let array: JObjectArray<'_, E::Kind<'_>> = unsafe {
			jni_call_with_catch_and_null_check!(
				catch |env| {
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					else => Err(Error::NullPtr("Unexpected Exception")),
				},
				env,
				v1_1,
				NewObjectArray,
				length,
				element_class.as_raw(),
				initial_element.as_ref().as_raw()
			)
			.map(|array| JObjectArray::<E>::from_raw(env, array))?
		};

		Ok(array)
	}

	/// Creates a [`JObjectArray`] that wraps the given `raw` [`jobjectArray`]
	///
	/// # Safety
	///
	/// - `raw` must be a valid raw JNI local reference (or `null`).
	/// - `raw` must be an instance of `java.lang.Object[]` where each element is an instance of `E`.
	/// - There must not be any other owning [`Reference`] wrapper for the same reference.
	/// - The local reference must belong to the current thread and not outlive the
	///   JNI stack frame associated with the [Env] `'env_local` lifetime.
	pub unsafe fn from_raw<'env_local>(
		env: &Env<'env_local>,
		raw: jobjectArray,
	) -> JObjectArray<'env_local, E::Kind<'env_local>> {
		JObjectArray {
			// SAFETY: The caller guarantees raw is an object-array reference valid for this environment's lifetime.
			array: unsafe { JObject::from_raw(env, raw as jobject) },
			_marker: std::marker::PhantomData,
		}
	}

	/// Creates a new null reference.
	///
	/// Null references are always valid and do not belong to a local reference frame. Therefore,
	/// the returned `JObjectArray` always has the `'static` lifetime.
	#[must_use]
	pub const fn null() -> JObjectArray<'static, E::GlobalKind> {
		JObjectArray {
			array: JObject::null(),
			_marker: std::marker::PhantomData,
		}
	}

	/// Unwrap to the raw jni type.
	#[must_use]
	pub const fn into_raw(self) -> jobjectArray {
		self.array.into_raw() as jobjectArray
	}

	/// Cast a local reference to a [`JObjectArray<T>`]
	///
	/// This will do a runtime (`IsInstanceOf`) check that the object is an instance of `T[]`.
	///
	/// Also see these other options for casting local or global references to a [`JObjectArray<T>`]:
	/// - [`Env::as_cast`]
	/// - [`Env::new_cast_local_ref`]
	/// - [`Env::cast_local`]
	/// - [`Env::new_cast_global_ref`]
	/// - [`Env::cast_global`]
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] if the `IsInstanceOf` check fails.
	pub fn cast_local<'any_local>(
		env: &mut Env<'_>,
		obj: impl Reference + Into<JObject<'any_local>> + AsRef<JObject<'any_local>>,
	) -> Result<<JObjectArray<'any_local, E> as Reference>::Kind<'any_local>>
	where
		E: 'any_local,
	{
		env.cast_local::<JObjectArray<'any_local, E>>(obj)
	}

	/// Returns the length of the array.
	///
	/// # Errors
	///
	/// Returns an error if the array is null or a Java exception is pending.
	pub fn len(&self, env: &Env) -> Result<usize> {
		let array: crate::sys::jobject = null_check!(self.as_raw() as jobjectArray, "JObjectArray::len self argument")?;
		// SAFETY: The typed array reference is non-null and valid for this Env.
		let len: i32 = unsafe { jni_call_no_post_check_ex!(env, v1_1, GetArrayLength, array)? };
		usize::try_from(len).map_err(|_| Error::JniCall(JniError::Unknown))
	}

	/// Returns a local reference to an element of the [`JObjectArray`] `array`.
	///
	/// Returns [`Error::IndexOutOfBounds`] if the `index` is out of bounds.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the array is null, the index is out of bounds or does not fit JNI, or a Java exception is pending.
	///
	/// # Panics
	///
	/// Panics if the environment is not associated with the current top JNI frame.
	pub fn get_element<'env_local>(&self, env: &mut Env<'env_local>, index: usize) -> Result<E::Kind<'env_local>> {
		assert_eq!(env.level, JavaVM::thread_attach_guard_level());
		let array: crate::sys::jobject =
			null_check!(self.as_raw() as jobjectArray, "get_object_array_element array argument")?;
		let index: i32 = crate::sys::jsize::try_from(index).map_err(|_| Error::JniCall(JniError::InvalidArguments))?;
		// SAFETY: The typed array is non-null, index fits jint, and JNI checks array bounds.
		unsafe {
			jni_call_with_catch!(
                catch |env| {
                    crate::exceptions::JArrayIndexOutOfBoundsException =>
                        Err(Error::IndexOutOfBounds),
                    else => Err(Error::JniCall(JniError::Unknown)),
                },
                env, v1_1, GetObjectArrayElement, array, index)
			.map(|obj| E::kind_from_raw(obj))
		}
	}

	/// Sets an element of the [`JObjectArray`] `array`.
	///
	/// Returns:
	/// - [`Error::IndexOutOfBounds`] if the `index` is out of bounds.
	/// - [`Error::WrongObjectType`] if the `value` is not of the correct type.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the array is null, the index is invalid, the element has an incompatible type, or a Java exception is pending.
	pub fn set_element<'any_local>(
		&self,
		env: &Env<'_>,
		index: usize,
		value: impl AsRef<E::Kind<'any_local>>,
	) -> Result<()> {
		let array: crate::sys::jobject =
			null_check!(self.as_raw() as jobjectArray, "set_object_array_element array argument")?;
		let index: i32 = crate::sys::jsize::try_from(index).map_err(|_| Error::JniCall(JniError::InvalidArguments))?;
		// SAFETY: The typed array and element references are valid, index fits jint, and JNI checks bounds and element compatibility.
		unsafe {
			jni_call_with_catch!(
				catch |env| {
					crate::exceptions::JArrayIndexOutOfBoundsException =>
						Err(Error::IndexOutOfBounds),
					crate::exceptions::JArrayStoreException =>
						Err(Error::WrongObjectType),
					else => Err(Error::JniCall(JniError::Unknown)),
				},
				env,
				v1_1,
				SetObjectArrayElement,
				array,
				index,
				value.as_ref().as_raw()
			)?;
		}
		Ok(())
	}
}

// SAFETY: JObjectArray is a transparent JObject wrapper with no Drop side effects
unsafe impl<'local, E: Reference + 'local> Reference for JObjectArray<'local, E> {
	type Kind<'env>
		= JObjectArray<'env, E::Kind<'env>>
	where
		E::Kind<'env>: 'env;
	type GlobalKind = JObjectArray<'static, E::GlobalKind>;

	fn as_raw(&self) -> jobject {
		self.array.as_raw()
	}

	fn class_name() -> Cow<'static, JNIStr> {
		let inner: Cow<'_, JNIStr> = E::class_name();
		let inner: Cow<'_, str> = inner.to_str();
		let name: String = if inner.starts_with('[') {
			// Array references already use descriptors, including primitive arrays.
			format!("[{inner}")
		} else {
			// Other references use object binary names, even single letters such as I.
			format!("[L{inner};")
		};
		let name: JNIString = name.into();
		Cow::Owned(name)
	}

	fn lookup_class<'caller>(
		env: &Env<'_>,
		loader_context: &LoaderContext,
	) -> crate::errors::Result<impl Deref<Target = Global<JClass<'static>>> + 'caller> {
		let api: &JObjectArrayAPI<E::GlobalKind> = JObjectArrayAPI::<E::GlobalKind>::get(env, loader_context)?;
		Ok(&api.class)
	}

	unsafe fn kind_from_raw<'env>(local_ref: jobject) -> Self::Kind<'env> {
		JObjectArray {
			// SAFETY: The caller guarantees a valid local array reference with the requested lifetime.
			array: unsafe { JObject::kind_from_raw(local_ref) },
			_marker: std::marker::PhantomData,
		}
	}

	unsafe fn global_kind_from_raw(global_ref: jobject) -> Self::GlobalKind {
		JObjectArray {
			// SAFETY: The caller guarantees a valid global array reference.
			array: unsafe { JObject::global_kind_from_raw(global_ref) },
			_marker: std::marker::PhantomData,
		}
	}
}
