use std::{borrow::Cow, ffi::CStr, io, os::raw::c_void, ptr};

use thiserror::Error;

use crate::{
	JNIVersion,
	sys::{JavaVMInitArgs, JavaVMOption},
};

mod char_encoding_generic;

#[cfg(windows)]
mod char_encoding_windows;

#[cfg(doc)]
use crate::JavaVM;

/// Errors that can occur when invoking a [`JavaVM`] with the
/// [Invocation API](https://docs.oracle.com/en/java/javase/12/docs/specs/jni/invocation.html).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JvmError {
	/// The number of initialization options does not fit a JNI integer.
	#[error("too many JVM options: {0}")]
	TooManyOptions(usize),

	/// [`InitArgsBuilder::option`] or [`InitArgsBuilder::try_option`] was used, but the supplied
	/// string contains a U+0000 code point (except at the end).
	///
	/// This error is not raised if the string has a single U+0000 code point at the end.
	///
	/// [`InitArgsBuilder::option_encoded`] never raises this error.
	#[error("internal null in option: {0}")]
	NullOptString(String),

	/// [`InitArgsBuilder::option`] or [`InitArgsBuilder::try_option`] was used, but the option
	/// string is too long.
	///
	/// Currently, this error only occurs on Windows, where string length is limited to 1MB to
	/// avoid overflow in [`WideCharToMultiByte`] (see [discussion]). String length is not
	/// currently limited (other than by available memory) on other platforms.
	///
	/// [`InitArgsBuilder::option_encoded`] never raises this error, regardless of platform.
	///
	/// [discussion]: https://github.com/jni-rs/jni-rs/pull/414
	/// [`WideCharToMultiByte`]: https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-widechartomultibyte
	#[error("option is too long: {opt_string}")]
	#[non_exhaustive]
	OptStringTooLong {
		/// The option string.
		opt_string: String,
	},

	/// [`InitArgsBuilder::option`] or [`InitArgsBuilder::try_option`] was used, but the option
	/// string is not representable in the platform default character encoding.
	///
	/// [`InitArgsBuilder::option_encoded`] never raises this error.
	#[error("option {opt_string:?} is not representable in the platform default character encoding")]
	#[non_exhaustive]
	OptStringNotRepresentable {
		/// The option string.
		opt_string: String,
	},

	/// [`InitArgsBuilder::option`] or [`InitArgsBuilder::try_option`] was used, but the platform
	/// reported an error converting it to its default character encoding.
	///
	/// [`InitArgsBuilder::option_encoded`] never raises this error.
	#[error("couldn't convert option {opt_string:?} to the platform default character encoding: {error}")]
	#[non_exhaustive]
	OptStringTranscodeFailure {
		/// The option string.
		opt_string: String,

		/// The error reported by the platform's character encoding conversion routine.
		#[source]
		error: io::Error,
	},
}

impl JvmError {
	/// Returns the JVM option that caused the error, if it was caused by one.
	pub fn opt_string(&self) -> Option<&str> {
		match self {
			Self::TooManyOptions(_) => None,
			Self::NullOptString(opt_string)
			| Self::OptStringTooLong { opt_string, .. }
			| Self::OptStringNotRepresentable { opt_string, .. }
			| Self::OptStringTranscodeFailure { opt_string, .. } => Some(opt_string),
		}
		.map(String::as_str)
	}
}

const SPECIAL_OPTIONS: &[&str] = &["vfprintf", "abort", "exit"];

const SPECIAL_OPTIONS_C: &[&CStr] = &[c"vfprintf", c"abort", c"exit"];

#[derive(Debug, Default)]
struct Options<'a> {
	// Keep strings separate so `InitArgsBuilder::options` can expose its existing slice API.
	strings: Vec<Cow<'a, CStr>>,
	extra_info: Vec<*mut c_void>,
}

impl<'a> Options<'a> {
	fn push(&mut self, string: Cow<'a, CStr>, extra_info: *mut c_void) {
		self.strings.push(string);
		self.extra_info.push(extra_info);
	}
}

/// Builder for `JavaVM` `InitArgs`.
///
/// *This API requires "invocation" feature to be enabled,
/// see [Launching JVM from Rust](struct.JavaVM.html#launching-jvm-from-rust).*
#[derive(Debug)]
pub struct InitArgsBuilder<'a> {
	opts: Result<Options<'a>, JvmError>,
	ignore_unrecognized: bool,
	version: JNIVersion,
}

impl Default for InitArgsBuilder<'_> {
	fn default() -> Self {
		InitArgsBuilder {
			opts: Ok(Options::default()),
			ignore_unrecognized: false,
			version: JNIVersion::V1_8,
		}
	}
}

impl<'a> InitArgsBuilder<'a> {
	/// Create a new default `InitArgsBuilder`
	#[must_use]
	pub fn new() -> Self {
		InitArgsBuilder::default()
	}

	/// Adds a JVM option, such as `-Djavax.net.debug=all`.
	///
	/// See [the JNI specification][jni-options] for details on which options are accepted.
	///
	/// The `vfprintf`, `abort`, and `exit` options are ignored by this safe method. Use
	/// [`Self::option_with_extra_info`] to supply a callback explicitly.
	///
	/// The option must not contain any U+0000 code points except one at the end. A U+0000 code
	/// point at the end is not required, but on platforms where UTF-8 is the default character
	/// encoding, including one U+0000 code point at the end will make this method run slightly
	/// faster.
	///
	/// # Errors
	///
	/// This method can fail if:
	///
	/// * `opt_string` contains a U+0000 code point before the end.
	/// * `opt_string` cannot be represented in the platform default character encoding.
	/// * the platform's character encoding conversion API reports some other error.
	/// * `opt_string` is too long. (In the current implementation, the maximum allowed length is
	///   1048576 bytes on Windows. There is currently no limit on other platforms.)
	///
	/// Errors raised by this method are deferred. If an error occurs, it is returned from
	/// [`InitArgsBuilder::build`] instead.
	///
	/// [jni-options]: https://docs.oracle.com/en/java/javase/11/docs/specs/jni/invocation.html#jni_createjavavm
	#[must_use]
	pub fn option(mut self, opt_string: impl AsRef<str> + Into<Cow<'a, str>>) -> Self {
		if let Err(error) = self.try_option(opt_string) {
			self.opts = Err(error);
		}

		self
	}

	/// Adds a JVM option with an explicit JNI `extraInfo` pointer.
	///
	/// Unlike [`Self::option`], this accepts `vfprintf`, `abort`, and `exit`. The pointer is
	/// passed unchanged to the JVM, including when it is null. String encoding and deferred
	/// error handling are the same as for [`Self::option`].
	///
	/// # Safety
	///
	/// `extra_info` must satisfy the target JVM's requirements for this option. A non-null
	/// callback pointer must point to a function with the exact signature and calling convention
	/// expected by the JVM, not to a Rust closure or a pointer containing a function pointer.
	/// HotSpot uses these C signatures, with `JNICALL` corresponding to Rust's `extern "system"`
	/// (see the [OpenJDK hook declarations][hooks]):
	///
	/// ```text
	/// jint JNICALL vfprintf(FILE *stream, const char *format, va_list args);
	/// void JNICALL exit(jint status);
	/// void JNICALL abort(void);
	/// ```
	///
	/// In particular, `vfprintf` takes a platform-specific `va_list`, not Rust variadic
	/// arguments or a portable `*mut c_void`. Use an ABI-correct binding or a C shim compiled
	/// for the target JVM. Do not assume `extern "C"` matches `JNICALL` on every target.
	///
	/// The caller retains ownership of the pointer. Any referenced data, callback code (and
	/// its library), and callback state must remain valid for every JVM use, potentially from
	/// JVM creation through shutdown and until all in-flight callbacks have finished. This
	/// lifetime is not tied to `'a`, the builder, or `InitArgs`, which may be dropped as soon
	/// as JVM creation returns. Callbacks must tolerate the JVM's calling threads and any
	/// concurrent or reentrant calls, and must not unwind across the FFI boundary.
	///
	/// # Errors
	///
	/// Defers the same string conversion errors as [`Self::option`] until [`Self::build`].
	///
	/// [hooks]: https://github.com/openjdk/jdk/blob/jdk-25-ga/src/hotspot/share/runtime/arguments.hpp
	///
	/// # Examples
	///
	/// ```
	/// use abos_jni::{InitArgsBuilder, sys::jint};
	/// use std::ffi::c_void;
	///
	/// extern "system" fn on_exit(_status: jint) {
	///     // Avoid panics and access only state that remains valid during JVM shutdown.
	/// }
	///
	/// // Safety: The function has the JVM's exit-hook ABI, has static code lifetime,
	/// // accesses no state, and cannot unwind.
	/// let args = unsafe {
	///     InitArgsBuilder::new().option_with_extra_info("exit", on_exit as *mut c_void)
	/// }.build()?;
	/// # Ok::<(), abos_jni::JvmError>(())
	/// ```
	#[must_use]
	pub unsafe fn option_with_extra_info(
		mut self,
		opt_string: impl AsRef<str> + Into<Cow<'a, str>>,
		extra_info: *mut c_void,
	) -> Self {
		// Safety: The caller guarantees the pointer's ABI and lifetime requirements.
		if let Err(error) = unsafe { self.try_option_with_extra_info(opt_string, extra_info) } {
			self.opts = Err(error);
		}
		self
	}

	/// Adds a JVM option, such as `-Djavax.net.debug=all`. Returns an error immediately upon
	/// failure.
	///
	/// This is an alternative to [`InitArgsBuilder::option`] that does not defer errors. See
	/// below for details.
	///
	/// See [the JNI specification][jni-options] for details on which options are accepted.
	///
	/// The `vfprintf`, `abort`, and `exit` options are ignored by this safe method. Use
	/// [`Self::option_with_extra_info`] to supply a callback explicitly.
	///
	/// The option must not contain any U+0000 code points except one at the end. A U+0000 code
	/// point at the end is not required, but on platforms where UTF-8 is the default character
	/// encoding, including one U+0000 code point at the end will make this method run slightly
	/// faster.
	///
	/// # Errors
	///
	/// This method can fail if:
	///
	/// * `opt_string` contains a U+0000 code point before the end.
	/// * `opt_string` cannot be represented in the platform default character encoding.
	/// * the platform's character encoding conversion API reports some other error.
	/// * `opt_string` is too long. (In the current implementation, the maximum allowed length is
	///   1048576 bytes on Windows. There is currently no limit on other platforms.)
	///
	/// Unlike the `option` method, this one does not defer errors. If the `opt_string` cannot be
	/// used, then this method returns `Err` and `self` is not changed. If there is already a
	/// deferred error, however, then this method does nothing.
	///
	/// [jni-options]: https://docs.oracle.com/en/java/javase/11/docs/specs/jni/invocation.html#jni_createjavavm
	pub fn try_option(&mut self, opt_string: impl Into<Cow<'a, str>>) -> Result<(), JvmError> {
		let opt_string: Cow<'_, str> = opt_string.into();
		let name: &str = opt_string.strip_suffix('\0').unwrap_or(&opt_string);
		if SPECIAL_OPTIONS.contains(&name) {
			return Ok(());
		}

		// Safety: Callback options were excluded, and ordinary options use null extraInfo.
		unsafe { self.try_option_with_extra_info(opt_string, ptr::null_mut()) }
	}

	/// Adds an option with an explicit JNI `extraInfo` pointer, returning encoding errors
	/// immediately. Unlike [`Self::try_option`], callback options are not filtered out.
	///
	/// # Safety
	///
	/// The pointer, callback ABI, and lifetime requirements of [`Self::option_with_extra_info`]
	/// apply equally to this method.
	///
	/// # Errors
	///
	/// Returns the same errors as [`Self::try_option`], leaving the builder unchanged on
	/// failure. Does nothing if the builder already has a deferred error.
	pub unsafe fn try_option_with_extra_info(
		&mut self,
		opt_string: impl Into<Cow<'a, str>>,
		extra_info: *mut c_void,
	) -> Result<(), JvmError> {
		let opt_string: Cow<'_, str> = opt_string.into();

		// If there is already a deferred error, do nothing.
		let Ok(opts) = &mut self.opts else { return Ok(()) };

		// If the option is the empty string, then skip everything else and pass a constant empty
		// C string. This isn't just an optimization; Win32 `WideCharToMultiByte` will **fail** if
		// passed an empty string, so we have to do this check first.
		if matches!(opt_string.as_ref(), "" | "\0") {
			opts.push(Cow::Borrowed(c""), extra_info);
			return Ok(());
		}

		#[cfg(windows)]
		let encoded: Cow<'a, CStr> = char_encoding_windows::str_to_cstr_win32_default_codepage(opt_string)?;
		#[cfg(not(windows))]
		// Assume UTF-8 on all other platforms.
		let encoded: Cow<'a, CStr> = char_encoding_generic::utf8_to_cstr(opt_string)?;

		opts.push(encoded, extra_info);
		Ok(())
	}

	/// Adds a JVM option, such as `-Djavax.net.debug=all`. The option must be a `CStr` encoded in
	/// the platform default character encoding.
	///
	/// This is an alternative to [`InitArgsBuilder::option`] that does not do any encoding. This
	/// method is not `unsafe` as it cannot cause undefined behavior, but the option will be
	/// garbled (that is, become [mojibake](https://en.wikipedia.org/wiki/Mojibake)) if not
	/// encoded correctly.
	///
	/// See [the JNI specification][jni-options] for details on which options are accepted.
	///
	/// The `vfprintf`, `abort`, and `exit` options are ignored by this safe method. Use
	/// [`Self::option_with_extra_info`] to supply a callback explicitly.
	///
	/// This method does not fail, and will neither return nor defer an error.
	///
	/// [jni-options]: https://docs.oracle.com/en/java/javase/11/docs/specs/jni/invocation.html#jni_createjavavm
	#[must_use]
	pub fn option_encoded(self, opt_string: impl Into<Cow<'a, CStr>>) -> Self {
		let opt_string: Cow<'_, CStr> = opt_string.into();

		if SPECIAL_OPTIONS_C.contains(&&*opt_string) {
			return self;
		}

		// Safety: Callback options were excluded, and ordinary options use null extraInfo.
		unsafe { self.option_encoded_with_extra_info(opt_string, ptr::null_mut()) }
	}

	/// Adds a platform-encoded option with an explicit JNI `extraInfo` pointer.
	/// Unlike [`Self::option_encoded`], callback options are not filtered out.
	/// This method neither returns nor defers errors, and does nothing after a deferred error.
	///
	/// # Safety
	///
	/// The pointer, callback ABI, and lifetime requirements of [`Self::option_with_extra_info`]
	/// apply equally to this method. The string must use the target JVM's platform encoding.
	#[must_use]
	pub unsafe fn option_encoded_with_extra_info(
		mut self,
		opt_string: impl Into<Cow<'a, CStr>>,
		extra_info: *mut c_void,
	) -> Self {
		let opt_string: Cow<'_, CStr> = opt_string.into();
		if let Ok(opts) = &mut self.opts {
			opts.push(opt_string, extra_info);
		}
		self
	}

	/// Set JNI version for the init args
	///
	/// Default: V8
	#[must_use]
	pub const fn version(self, version: JNIVersion) -> Self {
		let mut s: InitArgsBuilder<'_> = self;
		s.version = version;
		s
	}

	/// Set the `ignoreUnrecognized` init arg flag
	///
	/// If ignoreUnrecognized is true, `JavaVM::new` ignores all unrecognized option strings that
	/// begin with "-X" or "_". If ignoreUnrecognized is false, `JavaVM::new` returns Err as soon as
	/// it encounters any unrecognized option strings.
	///
	/// Default: `false`
	#[must_use]
	pub const fn ignore_unrecognized(self, ignore: bool) -> Self {
		let mut s: InitArgsBuilder<'_> = self;
		s.ignore_unrecognized = ignore;
		s
	}

	/// Build the `InitArgs`
	///
	/// # Errors
	///
	/// If a call to [`Self::option`] or [`Self::option_with_extra_info`] caused a deferred error,
	/// it is returned from this method. Also returns [`JvmError::TooManyOptions`] if the option count does not fit JNI.
	pub fn build(self) -> Result<InitArgs<'a>, JvmError> {
		let Options {
			strings: opt_strings,
			extra_info,
		} = self.opts?;
		let option_count: i32 = checked_option_count(opt_strings.len())?;

		let opts: Vec<JavaVMOption> = opt_strings
			.iter()
			.zip(extra_info)
			.map(|(opt_string, extra_info)| JavaVMOption {
				optionString: opt_string.as_ptr().cast_mut(),
				extraInfo: extra_info,
			})
			.collect();

		Ok(InitArgs {
			inner: JavaVMInitArgs {
				version: self.version.into(),
				ignoreUnrecognized: self.ignore_unrecognized.into(),
				options: opts.as_ptr().cast_mut(),
				nOptions: option_count,
			},
			_opts: opts,
			_opt_strings: opt_strings,
		})
	}

	/// Returns collected option strings, including those added with explicit `extraInfo` pointers.
	///
	/// If a call to [`Self::option`] or [`Self::option_with_extra_info`] caused a deferred error,
	/// this method returns a reference to that error.
	///
	/// # Errors
	///
	/// Returns the deferred error from an earlier attempt to add an invalid JVM option.
	pub fn options(&self) -> Result<&[Cow<'a, CStr>], &JvmError> {
		self.opts.as_ref().map(|opts| opts.strings.as_slice())
	}
}

/// `JavaVM` `InitArgs`.
///
/// *This API requires "invocation" feature to be enabled,
/// see [Launching JVM from Rust](struct.JavaVM.html#launching-jvm-from-rust).*
#[derive(Debug)]
pub struct InitArgs<'a> {
	inner: JavaVMInitArgs,

	// `JavaVMOption` structures are stored here. The JVM accesses this `Vec`'s contents through a
	// raw pointer.
	_opts: Vec<JavaVMOption>,

	// Option strings are stored here. This ensures that any that are owned aren't dropped before
	// the JVM is finished with them.
	_opt_strings: Vec<Cow<'a, CStr>>,
}

impl InitArgs<'_> {
	#[allow(
		dead_code,
		reason = "The pointer is used only when the host JVM invocation feature is enabled."
	)]
	pub(crate) const fn inner_ptr(&self) -> *mut c_void {
		&raw const self.inner as _
	}
}

fn checked_option_count(count: usize) -> Result<i32, JvmError> {
	i32::try_from(count).map_err(|_| JvmError::TooManyOptions(count))
}

#[cfg(test)]
#[path = "../../../tests/unit/api/vm/init_args.rs"]
mod tests;
