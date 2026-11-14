use thiserror::Error;

use crate::{
	Env, JavaVM,
	errors::{Error, JniError, Result},
	strings::{JNIStr, JNIString, MUTF8Chars},
};

use super::Reference as _;

crate::bind_java_type! {
	pub JString => "java.lang.String",
	__jni_core = true,
	__sys_type = jstring,
	is_instance_of {
		char_sequence = JCharSequence,
	},
	methods {
		/// Returns a canonical, interned version of this string.
		fn intern() -> JString,
	}
}

/// Displays the string contents, temporarily attaching the thread if needed.
///
/// Null references display as `"<NULL>"`. JNI failures are logged and display as
/// `"<JNI Not Initialized>"` if the VM singleton is unavailable, or `"<JNI Error>"` otherwise.
impl std::fmt::Display for JString<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		#[derive(Error, Debug)]
		#[error(transparent)]
		enum FmtOrJniError {
			Fmt(#[from] std::fmt::Error),
			Jni(#[from] crate::errors::Error),
		}

		if self.is_null() {
			return write!(f, "<NULL>");
		}

		// Native arguments can exist before EnvUnowned initializes the singleton.
		JavaVM::singleton()
			.map_err(FmtOrJniError::Jni)
			.and_then(|vm| {
				// Formatting a global reference must not leave a new permanent attachment.
				vm.attach_current_thread_for_scope(|env| -> std::result::Result<(), FmtOrJniError> {
					let mutf8_chars: MUTF8Chars<'_, &JString<'_>> = self.mutf8_chars(env)?;
					let s: std::borrow::Cow<'_, str> = mutf8_chars.to_str();
					write!(f, "{s}")?;
					Ok(())
				})
			})
			.or_else(|err| {
				match err {
					FmtOrJniError::Fmt(err) => Err(err),
					FmtOrJniError::Jni(crate::errors::Error::UninitializedJavaVM) => {
						log::error!("error getting JavaVM singleton to format JString: {err:#?}");
						write!(f, "<JNI Not Initialized>")
					}
					FmtOrJniError::Jni(err) => {
						// JNI errors become placeholders rather than fmt::Error.
						log::error!("error getting JString contents: {err:#?}");
						write!(f, "<JNI Error>")
					}
				}
			})
	}
}

impl JString<'_> {
	/// Encodes a Rust `&str` to MUTF-8 and creates a `JString` (`java.lang.String` object).
	///
	/// This is a convenience that's equivalent to calling [`Self::from_str`]
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Performance
	///
	/// The input string is re-encoded to modified UTF-8, so this involves a copy of your input to
	/// encode, before calling into JNI to create the `JString`.
	///
	/// To avoid the overhead of encoding and copying, use [`Self::from_jni_str`] and the
	/// [`crate::jni_str`!] macro to encode strings to MUTF-8 at compile time.
	///
	/// # Errors
	///
	/// Returns an error if the JVM cannot allocate the string or a Java exception is already pending.
	pub fn new<'env_local>(env: &mut Env<'env_local>, from: impl AsRef<str>) -> Result<JString<'env_local>> {
		Self::from_str(env, from)
	}

	/// Encodes a Rust `&str` to MUTF-8 and creates a `JString` (`java.lang.String` object).
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Performance
	///
	/// The input string is re-encoded to modified UTF-8, so this involves a copy of your input to
	/// encode, before calling into JNI to create the `JString`.
	///
	/// To avoid the overhead of encoding and copying, use [`Self::from_jni_str`] and the
	/// [`crate::jni_str`!] macro to encode strings to MUTF-8 at compile time.
	///
	/// # Errors
	///
	/// Returns an error if the JVM cannot allocate the string or a Java exception is already pending.
	pub fn from_str<'env_local>(env: &mut Env<'env_local>, from: impl AsRef<str>) -> Result<JString<'env_local>> {
		Self::from_jni_str(env, JNIString::new(from))
	}

	/// Creates a `JString` (`java.lang.String` object) from a [`JNIStr`] (modified UTF-8).
	///
	/// For simple string literals, consider using the [`crate::jni_str`!] macro to create / encode
	/// [`JNIStr`] literals at compile time.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Errors
	///
	/// Returns an error if the JVM cannot allocate the string or a Java exception is already pending.
	pub fn from_jni_str<'env_local>(
		env: &mut Env<'env_local>,
		from: impl AsRef<JNIStr>,
	) -> Result<JString<'env_local>> {
		env.assert_top();
		let ffi_str: &JNIStr = from.as_ref();
		// SAFETY: JNIStr supplies a terminated modified-UTF-8 string, and env owns the current local frame.
		unsafe {
			jni_call_with_catch_and_null_check!(
                catch |env| {
                    crate::exceptions::JOutOfMemoryError =>
                        Err(Error::JniCall(JniError::NoMemory)),
                    else => Err(Error::NullPtr("Unexpected Exception")),
                },
                env, v1_1, NewStringUTF, ffi_str.as_ptr())
			.map(|s| JString::from_raw(env, s))
		}
	}

	/// Gets the contents of this string, in [modified UTF-8] encoding (via `GetStringUTFChars`).
	///
	/// The returned [`MUTF8Chars`] guard can be used to access the modified UTF-8 bytes, or to
	/// convert to a Rust string (UTF-8).
	///
	/// For example:
	///
	/// ```rust,no_run
	/// # use abos_jni::{errors::Result, Env, objects::*, strings::*};
	/// #
	/// # fn f(env: &mut Env) -> Result<()> {
	/// let my_jstring = JString::from_str(env, "Hello, world!")?;
	/// let mutf8_chars = my_jstring.mutf8_chars(env)?;
	/// let jni_str: &JNIStr = &mutf8_chars;
	/// let rust_str = jni_str.to_str();
	/// # Ok(())
	/// # }
	/// ```
	///
	/// When the [`MUTF8Chars`] guard is dropped, the reference to the contents gets released.
	/// Cleanup temporarily attaches the current thread if it is no longer attached.
	///
	/// The [`MUTF8Chars`] guard dereferences to a [`JNIStr`].
	///
	/// Also note that [`MUTF8Chars`] (and also [`JString`] itself) implements `Display` and
	/// `ToString` so it's also possible to use `.to_string()` to get a Rust String from a [`JString`]
	///
	/// [modified UTF-8]: https://en.wikipedia.org/wiki/UTF-8#Modified_UTF-8
	///
	/// # Errors
	///
	/// Returns an [`Error::NullPtr`] if this [`JString`] is null.
	pub fn mutf8_chars(&self, env: &Env<'_>) -> Result<MUTF8Chars<'_, &JString<'_>>> {
		MUTF8Chars::from_get_string_utf_chars(env, self)
	}

	/// Gets the contents of this string as a Rust `String`.
	///
	/// Each unpaired UTF-16 surrogate is replaced with U+FFFD. Valid characters,
	/// including NULs and supplementary characters, are preserved.
	///
	/// For example:
	///
	/// ```rust,no_run
	/// # use abos_jni::{errors::Result, Env, objects::*};
	/// #
	/// # fn f(env: &mut Env) -> Result<()> {
	/// let jstring = JString::from_str(env, "Hello, world!")?;
	/// let rust_string = jstring.try_to_string(&env)?;
	/// assert_eq!(rust_string, "Hello, world!");
	/// # ; Ok(())
	/// # }
	/// ```
	///
	/// This is equivalent to calling [`Self::mutf8_chars`] and then converting that to a `String`, like:
	///
	/// ```rust,no_run
	/// # use abos_jni::{errors::Result, Env, objects::*};
	/// #
	/// # fn f(env: &mut Env) -> Result<()> {
	/// let jstring = JString::from_str(env, "Hello, world!")?;
	/// let mutf8_chars = jstring.mutf8_chars(&env)?;
	/// let rust_string = mutf8_chars.to_string();
	/// # ; Ok(())
	/// # }
	/// ```
	///
	/// # Errors
	///
	/// Returns an [`Error::NullPtr`] if this [`JString`] is null.
	pub fn try_to_string(&self, env: &Env<'_>) -> Result<String> {
		let mutf8_chars: MUTF8Chars<'_, &JString<'_>> = self.mutf8_chars(env)?;
		Ok(mutf8_chars.to_string())
	}
}
