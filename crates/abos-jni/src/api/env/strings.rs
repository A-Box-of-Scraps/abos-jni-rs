use super::{Env, JList, JMap, JObject, JString, MUTF8Chars, Reference, Result};

impl<'local> Env<'local> {
	/// Cast a [`JObject`] to a [`JList`].
	///
	/// Returns `Error::WrongObjectType` if the object is not a `java.util.List`.
	#[deprecated(
		since = "0.22.0",
		note = "use JList::cast_local instead or Env::new_cast_local_ref/cast_local/as_cast_local or Env::new_cast_global_ref/cast_global/as_cast_global"
	)]
	pub fn get_list<'any_local>(
		&mut self,
		obj: impl Reference + Into<JObject<'any_local>> + AsRef<JObject<'any_local>>,
	) -> Result<JList<'any_local>> {
		JList::cast_local(self, obj)
	}

	/// Cast a [`JObject`] to a [`JMap`].
	///
	/// Returns `Error::WrongObjectType` if the object is not a `java.util.Map`.
	#[deprecated(
		since = "0.22.0",
		note = "use JMap::cast_local instead or Env::new_cast_local_ref/cast_local/as_cast_local or Env::new_cast_global_ref/cast_global/as_cast_global"
	)]
	pub fn get_map<'any_local>(
		&mut self,
		obj: impl Reference + Into<JObject<'any_local>> + AsRef<JObject<'any_local>>,
	) -> Result<JMap<'any_local>> {
		JMap::cast_local(self, obj)
	}

	/// Gets the contents of a Java string, in [modified UTF-8] encoding.
	///
	/// The returned [`MUTF8Chars`] can be used to access the modified UTF-8 bytes,
	/// or to convert to a Rust string (which uses standard UTF-8 encoding).
	///
	/// This entails calling the JNI function `GetStringUTFChars`.
	///
	/// [modified UTF-8]: https://en.wikipedia.org/wiki/UTF-8#Modified_UTF-8
	///
	/// # Errors
	///
	/// Returns an error if `obj` is `null`.
	#[deprecated(
		since = "0.22.0",
		note = "use JString::mutf8_chars or JString::to_string instead; this method is redundant and does not perform unsafe operations"
	)]
	pub fn get_string_unchecked<'any_local, StringRef>(
		&mut self,
		obj: StringRef,
	) -> Result<MUTF8Chars<'any_local, StringRef>>
	where
		StringRef: AsRef<JString<'any_local>> + Reference,
	{
		MUTF8Chars::from_get_string_utf_chars(self, obj)
	}

	/// Gets the contents of a Java string, in [modified UTF-8] encoding.
	///
	/// The returned [`MUTF8Chars`] can be used to access the modified UTF-8 bytes,
	/// or to convert to a Rust string (which uses standard UTF-8 encoding).
	///
	/// This entails calling the JNI function `GetStringUTFChars`.
	///
	/// [modified UTF-8]: https://en.wikipedia.org/wiki/UTF-8#Modified_UTF-8
	///
	/// # Errors
	///
	/// Returns an error if `obj` is `null`.
	#[deprecated(since = "0.22.0", note = "use JString::mutf8_chars or JString::to_string instead")]
	pub fn get_string<'any_local, StringRef>(&self, obj: StringRef) -> Result<MUTF8Chars<'any_local, StringRef>>
	where
		StringRef: AsRef<JString<'any_local>> + Reference,
	{
		MUTF8Chars::from_get_string_utf_chars(self, obj)
	}

	/// Create a new java string object from a rust string.
	///
	/// This requires a re-encoding of rusts UTF-8 strings to JNI's modified UTF-8 format before
	/// calling the JNI function `NewStringUTF`.
	///
	/// To avoid this intermediate copy + encoding, consider using [`JString::from_jni_str`], and
	/// using the [`crate::jni_str`] macro to encode string literals at compile time.
	pub fn new_string(&mut self, from: impl AsRef<str>) -> Result<JString<'local>> {
		JString::new(self, from)
	}
}
