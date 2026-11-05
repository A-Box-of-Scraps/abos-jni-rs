use std::{
	convert::TryInto,
	marker::PhantomData,
	os::raw::{c_char, c_void},
	panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
	ptr,
	sync::{Mutex, MutexGuard},
};

use abos_jni_sys::jobject;

use crate::{
	DEFAULT_LOCAL_FRAME_CAPACITY, JNIVersion, JavaVM,
	descriptors::Desc,
	errors::*,
	jni_sig,
	objects::{
		Auto, AutoElements, AutoElementsCritical, Global, IntoAuto, JByteBuffer, JClass, JClassLoader, JFieldID, JList,
		JMap, JMethodID, JObject, JStaticFieldID, JStaticMethodID, JString, JThrowable, JValue, JValueOwned,
		ReleaseMode, TypeArray, Weak,
	},
	signature::{FieldSignature, JavaType, MethodSignature, Primitive},
	strings::{JNIStr, MUTF8Chars},
	sys::{self, JNINativeMethod, jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort, jsize, jvalue},
};
use crate::{
	errors::ErrorPolicy,
	objects::{
		Cast, JBooleanArray, JByteArray, JCharArray, JDoubleArray, JFloatArray, JIntArray, JLongArray, JObjectArray,
		JPrimitiveArray, JShortArray, LoaderContext, type_array_sealed::TypeArraySealed,
	},
};
use crate::{objects::AsJArrayRaw, signature::ReturnType};

use super::{AttachGuard, objects::Reference};

#[cfg(doc)]
use crate::{jni_str, objects::JThread, strings::JNIString};

/// A non-transparent wrapper around a raw [`sys::JNIEnv`] pointer that provides
/// safe access to the Java Native Interface (JNI) functions.
///
/// Since [Env] is not a type that is publicly constructible, you must obtain a
/// reference, either through a thread attachment, by upgrading an
/// [`EnvUnowned`] within a native method, or via the (unsafe)
/// [`AttachGuard::from_unowned`] APIs.
///
/// See:
/// - [`JavaVM::attach_current_thread`]
/// - [`JavaVM::attach_current_thread_for_scope`]
/// - [`JavaVM::with_local_frame`]
/// - [`JavaVM::with_top_local_frame`]
/// - [`EnvUnowned::with_env`]
///
/// [Env] is a non-transparent wrapper that is not FFI-safe, so it must not be
/// used to try and capture a raw [`sys::JNIEnv`] pointer passed to a native
/// method.
///
/// See [`EnvUnowned`] for a wrapper that is FFI-safe and can be used to capture
/// a `sys::JNIEnv` pointer passed to a native method.
///
/// # Overview of the API
///
/// All the [`Env`] methods aim to follow the JNI specification naming closely
/// enough to make it easier to map between the two. This means that method
/// names, parameter types, and return types will generally be consistent with
/// the JNI specification.
///
/// JNI APIs that can logically be thought of as methods, such as
/// `GetObjectArrayElement`, can be found as methods on the relevant Rust type,
/// in this case [`JObjectArray::get_element`].
///
/// # Exception handling
///
/// Since we're calling into the JVM with this, many methods also have the
/// potential to cause an exception to get thrown. If this is the case, an `Err`
/// result will be returned with the error kind [`Error::JavaException`]. Note
/// that this will _not_ clear the exception - it's up to the caller to decide
/// whether to do so or to let it continue being thrown.
///
/// # References and Lifetimes
///
/// As in C JNI, interactions with Java objects happen through
/// <dfn>references</dfn>, either local or global.
///
/// This crate provides various safe wrappers around JNI references, such as
/// [`JObject`], [`JClass`], [`JString`] and [`JList`] which all implement the
/// [`Reference`] trait.
///
/// By default these types represent <dfn>local references</dfn>, and they will
/// be associated with a lifetime that names the local reference frame they
/// belong to.
///
/// So for example `JObject<'local_1>`  and  `JObject<'local_2>` are local
/// reference that are owned by two different local reference frames and
/// although they may both be usable from a higher stack frame, it's not
/// possible to simply move ownership of a local reference from one local
/// reference frame to another.
///
/// A global reference can be represented by a [`Global`], which will wrap one
/// of the above primitive reference types like `Global<JObject<'static>>`. In
/// this case the `'static` lifetime indicates that the reference is not tied to
/// a local reference frame and can be used from any thread.
///
/// So long as there is at least one reference to a Java object, the JVM garbage
/// collector will not reclaim it.
///
/// <dfn>Global references</dfn> exist until deleted. Deletion occurs when the
/// `Global` is dropped.
///
/// <dfn>Local references</dfn> belong to a local reference frame, and exist
/// until [deleted][Env::delete_local_ref] or until the local reference frame is
/// exited. A new <dfn>local reference frame</dfn> is entered when a native
/// method is called from Java, or when Rust code does so explicitly using
/// [`Env::with_local_frame`]. That local reference frame is exited when the
/// native method or `with_local_frame` returns. When a local reference frame is
/// exited, all local references created inside it are deleted.
///
/// Unlike C JNI, this crate creates a separate `Env` for each local reference
/// frame. The associated Rust lifetime `'local` represents that local reference
/// frame. Rust's borrow checker will ensure that local references are not used
/// after their local reference frame exits (which would cause undefined
/// behavior).
///
/// Unlike global references, local references are not deleted when dropped by
/// default. This is for performance: it is faster for the JVM to delete all of
/// the local references in a frame all at once, than to delete each local
/// reference one at a time. However, this can cause a memory leak if the local
/// reference frame remains entered for a long time, such as a long-lasting
/// loop, in which case local references should be deleted explicitly. Local
/// references can be deleted when dropped if desired; use
/// [`Env::delete_local_ref`] or wrap with [`IntoAuto::auto`] to arrange that.
///
/// ## Lifetime Names
///
/// This crate uses the following convention for lifetime names:
///
/// * `'local` is the lifetime of a local reference frame, as described above.
///
/// * `'other_local`, `'other_local_1`, and `'other_local_2` are the lifetimes
///   of some other local reference frame, which may be but doesn't have to be
///   the same as `'local`. For example, [`Env::new_local_ref`] accepts a local
///   reference in any local reference frame `'other_local` and creates a new
///   local reference to the same object in `'local`.
///
/// * `'obj_ref` is the lifetime of a borrow of a JNI reference, like
///   <code>&amp;[JObject]</code> or <code>&amp;[Global]</code>. For example,
///   [`Env::get_list`] constructs a new [`JList`] that borrows a `&'obj_ref
///   JObject`.
///
/// ## `null` Java references
/// `null` Java references are handled by the following rules:
///   - Methods that require non-null references will return [Error::NullPtr] if
///     passed a `null` reference.
///   - If a JNI function returns `null` to indicate an error (e.g.
///     [`Env::new_int_array`]), it is converted to `Err`/[`Error::NullPtr`], or
///     some other more applicable error type, such as
///     [`Error::MethodNotFound`].
///   - Otherwise `null` is considered a valid Java reference and is represented
///     as [`JObject::null()`].
///
/// ## A `&mut Env` represents the top JNI stack frame
///
/// Each `Env<'local>` represents a single local reference frame in the JVM.
///
/// A **mutable** `&mut Env` is special: it always represents the *current top*
/// local reference frame on the JNI stack.
///
/// * You need `&mut Env` when creating new local references, since those can
///   only be created in the top frame.
/// * A shared `&Env` is fine for operations that don’t produce new local
///   references.
///
/// If you only have a shared `&Env` but need to create temporary local
/// references (that won’t escape the scope), you can materialize a new mutable
/// handle to the top frame using:
///
/// * [`Env::with_local_frame`] – pushes a new frame, returns a mutable `Env`
///   for it.
/// * [`Env::with_top_local_frame`] – borrows a mutable `Env` for the existing
///   top frame without pushing a new one.
///
/// If you don’t even have an `Env` (e.g. inside a `Drop` implementation), you
/// can still get one via:
///
/// * [`JavaVM::attach_current_thread`]
/// * [`JavaVM::with_top_local_frame`] - if you can guarantee that the thread is
///   already attached, and you just need a handle to the existing top frame.
///
/// (and you can access a [`JavaVM`] via [`JavaVM::singleton`]).
///
/// These APIs are safe because they constrain the new [`Env`] lifetime to the
/// scope of the call, so any references created cannot leak.
///
/// ## Runtime Top Frame Checks
///
/// Rust’s type system ensures that a mutable `Env<'local>` only produces
/// references tied to its frame.
///
/// Normally this crate also guarantees that a `&mut Env` represents the top
/// frame.
///
/// However, because the JVM is global and multiple crates may independently use
/// jni-rs, some valid usage patterns can be misused to create multiple mutable
/// `Env`s in the same scope. For example:
///
/// ```rust,no_run
/// # use abos_jni::{JavaVM, errors::Result, objects::JString};
/// # fn f(vm: &JavaVM) -> Result<()> {
/// vm.attach_current_thread(|env1| -> Result<()> {
///     vm.attach_current_thread(|env2| -> Result<()> {
///         // env1 is still mutable, but no longer the top frame.
///         // This will panic at runtime:
///         JString::from_str(env1, "not valid here").unwrap();
///         Ok(())
///     })
/// })
/// # ; Ok(())
/// # }
/// ```
///
/// To prevent unsoundness, this crate enforces a **runtime check**:
///
/// * Creating a new local reference with a mutable `Env` that is *not* the top
///   frame will cause a panic.
///
/// ### Rule of thumb
///
/// * Keep only one `Env` in scope at a time.
/// * Always shadow outer `env` variables when introducing a new one.
///
/// As long as you follow this pattern, the type system will ensure that local
/// references are only created from the valid top frame.
///
/// ## Troubleshooting `cannot borrow as mutable`
///
/// If a function takes two or more parameters, one of them is `Env`, and
/// another is something returned by a `Env` method (like [`JObject`]), then
/// calls to that function may not compile:
///
/// ```rust,compile_fail
/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::*};
/// #
/// # fn f(env: &mut Env) -> Result<()> {
/// fn example_function(
///     env: &mut Env,
///     obj: &JObject,
/// ) {
///     // ...
/// }
///
/// example_function(
///     env,
///     // ERROR: cannot borrow `*env` as mutable more than once at a time
///     &env.new_object(
///         jni_str!("com/example/SomeClass"),
///         jni_sig!("()V"),
///         &[],
///     )?,
/// )
/// # ; Ok(())
/// # }
/// ```
///
/// This comes from the compiler needing to borrow `env` as mutable for the call
/// to `new_object`, but also needing to borrow it as mutable for the call to
/// `example_function`.
///
/// In the past we used to recommend here that you could avoid this by
/// reordering the parameters so that `Env` comes last, but in practice this led
/// to very inconsistent APIs where some functions had `Env` first and some had
/// it last, which was confusing.
///
/// Now the recommendation is to just make sure that reference type arguments
/// are derived before the call:
///
/// ```rust,no_run
/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::JObject};
/// #
/// # fn f(env: &mut Env) -> Result<()> {
/// # fn example_function(
/// #     env: &mut Env,
/// #     obj: &JObject,
/// # ) {
/// #    // ...
/// # }
/// let obj = env.new_object(
///    jni_str!("com/example/SomeClass"),
///    jni_sig!("()V"),
///    &[],
/// )?;
/// example_function(env, &obj)
/// # ; Ok(())
/// # }
/// ```
///
/// Quite often reference type arguments have already been derived in some outer
/// scope and don't involve any JNI method calls so this isn't too bad in
/// practice and it keeps the APIs more consistent / predictable, with `Env`
/// always coming first.
///
/// # Checked and unchecked methods
///
/// Some of the methods come in two versions: checked (e.g. `call_method`) and
/// unchecked (e.g. `call_method_unchecked`). Under the hood, checked methods
/// perform some checks to ensure the validity of provided signatures, names and
/// arguments, and then call the corresponding unchecked method.
///
/// Checked methods are more flexible as they allow passing class names and
/// method/field descriptors as strings and may perform lookups of class objects
/// and method/field ids for you, also performing all the needed precondition
/// checks. However, these lookup operations are expensive, so if you need to
/// call the same method (or access the same field) multiple times, it is
/// [recommended](https://docs.oracle.com/en/java/javase/11/docs/specs/jni/design.html#accessing-fields-and-methods)
/// to cache the instance of the class and the method/field id, e.g.
///   - in loops
///   - when calling the same Java callback repeatedly.
///
/// If you do not cache references to classes and method/field ids, you will
/// *not* benefit from the unchecked methods.
///
/// Calling unchecked methods with invalid arguments and/or invalid class and
/// method descriptors may lead to segmentation fault.
///
/// # Zero-copy `AsRef<JNIStr>` arguments
///
/// The JNI specification for many functions requires that string arguments are
/// passed as NUL terminated, Modified UTF-8 encoded byte arrays.
///
/// Anything that accepts an `AsRef<JNIStr>` can directly accept a `JNIStr`
/// literal like `jni_str!("java/lang/String")` which will be encoded as MUTF-8
/// at compile time.
///
/// See [jni_str!] for more details.
///
/// For non-literal, dynamic strings then you can use [JNIString::new] to encode
/// strings as MUTF-8 at runtime (derefs as [`JNIStr`]).
///
#[derive(Debug)]
pub struct Env<'local> {
	/// A non-null JNIEnv pointer
	pub(crate) raw: *mut sys::JNIEnv,
	/// The current [`abos_jni::AttachGuard`] nesting level, which we assert matches
	/// the top/current nesting level whenever some API will return a new local
	/// reference
	pub(crate) level: usize,
	owns_attachment: bool,
	_lifetime: PhantomData<&'local ()>,
}

impl Drop for Env<'_> {
	fn drop(&mut self) {
		// NOOP - we just implement Drop so that the compiler won't consider
		// Env to be FFI safe.
	}
}

#[cfg(test)]
mod test {
	use crate::Env;
	static_assertions::assert_not_impl_any!(Env: Send);
	static_assertions::assert_not_impl_any!(Env: Sync);
}
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
/// Native method descriptor for use with [`Env::register_native_methods`].
///
/// This is a `#[repr(transparent)]` / `repr(C)` wrapper around the [`crate::sys::JNINativeMethod`]
/// struct but with the additional constraint that it can only be constructed via an `unsafe`
/// [`Self::from_raw_parts`] method that requires the caller to ensure that the function pointer is
/// valid and matches the signature.
pub struct NativeMethod<'desc> {
	desc: JNINativeMethod,
	_phantom: PhantomData<&'desc JNIStr>,
}

impl<'desc> NativeMethod<'desc> {
	/// Creates a new `NativeMethod` descriptor.
	///
	/// # Safety
	///
	/// The `fn_ptr` must be a valid pointer to a function that matches the
	/// signature specified by `sig`.
	///
	/// For example, with the signature `"(I)Ljava/lang/String;"`...
	///
	/// A **static method** must point to a function with the signature:
	///
	/// `fn<'local>(unowned_env: EnvUnowned<'local>, class: JClass<'local>, arg0: jint) -> JString<'local>`.
	///
	/// An **instance method** must point to a function with the signature:
	///
	/// `fn<'local>(unowned_env: EnvUnowned<'local>, this: JObject<'local>, arg0: jint) -> JString<'local>`.
	///
	/// or some specific `this` type like:
	///
	/// `fn<'local>(unowned_env: EnvUnowned<'local>, this: MyType<'local>, arg0: jint) -> JString<'local>`.
	pub const unsafe fn from_raw_parts(
		name: &'desc JNIStr,
		sig: &'desc JNIStr,
		fn_ptr: *mut c_void,
	) -> NativeMethod<'desc> {
		NativeMethod {
			desc: JNINativeMethod {
				name: name.as_ptr() as *mut c_char,
				signature: sig.as_ptr() as *mut c_char,
				fnPtr: fn_ptr as *mut c_void,
			},
			_phantom: PhantomData,
		}
	}
}

/// Guard for a lock on a java object. This gets returned from the `lock_obj`
/// method.
#[derive(Debug)]
#[must_use]
pub struct MonitorGuard<'local> {
	obj: sys::jobject,
	life: PhantomData<&'local ()>,
}

impl Drop for MonitorGuard<'_> {
	fn drop(&mut self) {
		// Panics:
		//
		// The first `.expect()` is OK because the `&self` reference is enough to prove that
		// `JavaVM::singleton` must have been initialized and won't panic. (A MonitorGuard can only
		// be created with a Env reference)
		//
		// The second `.expect()` is OK because the guard is associated with a Env lifetime, so
		// it logically shouldn't be possible for the thread to become detached before the monitor
		// is dropped.
		JavaVM::singleton()
			.expect("JavaVM singleton must be initialized")
			.with_top_local_frame(|env| -> crate::errors::Result<()> {
				// Safety:
				//
				// This relies on `MonitorGuard` not being `Send` to maintain the
				// invariant that "The current thread must be the owner of the monitor
				// associated with the underlying Java object referred to by obj"
				//
				// This also means we can assume the `IllegalMonitorStateException`
				// exception can't be thrown due to the current thread not owning
				// the monitor.
				let res = unsafe { ex_safe_jni_call_no_post_check_ex!(env, v1_1, MonitorExit, self.obj) };
				if let Err(err) = jni_error_code_to_result(res) {
					log::error!("error releasing java monitor: {err}");
				}

				Ok(())
			})
			.expect("MonitorGuard dropped on detached thread");
	}
}

#[cfg(test)]
static_assertions::assert_not_impl_any!(MonitorGuard: Send);

mod array_regions;
mod arrays;
mod buffers;
mod calls;
mod classes;
mod exceptions;
mod field_ids;
mod fields;
mod frames;
mod global_refs;
mod local_refs;
mod method_ids;
mod native;
mod reflection;
mod rust_fields;
mod state;
mod strings;
mod unchecked_calls;
mod unowned;
pub use unowned::*;
