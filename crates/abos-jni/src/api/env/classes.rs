use super::*;

impl<'local> Env<'local> {
	/// Load a class from a buffer of raw class data.
	///
	/// If `name` is null, the name of the class is inferred from the buffer.
	///
	/// Note: This requires `&mut` because it returns a new local reference to a class.
	///
	/// # Safety
	///
	/// The `buf` pointer must be valid for `buf_len` bytes.
	pub(super) unsafe fn define_class_impl(
		&mut self,
		name: *const c_char,
		loader: &JClassLoader,
		buf: *const jbyte,
		buf_len: usize,
	) -> Result<JClass<'local>> {
		if buf_len > jsize::MAX as usize {
			return Err(Error::JniCall(JniError::InvalidArguments));
		}
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		// Safety:
		// - DefineClass is 1.1 API that must be valid
		// - It is valid to potentially pass a `null` `name` to `DefineClass`, since the name can be
		// read from the bytecode.
		// - A null loader corresponds to the bootstrap class loader.
		//
		// Throws:
		// - `ClassFormatError`: if the class data does not specify a valid class.
		// - `ClassCircularityError`: if a class or interface would be its own superclass or
		//   superinterface.
		// - `OutOfMemoryError`: if the system runs out of memory.
		// - `SecurityException`: if the caller attempts to define a class in the `"java"` package
		//   tree.
		unsafe {
			jni_call_with_catch_and_null_check!(
				catch |env| {
					crate::exceptions::JClassFormatError =>
						Err(Error::ClassFormatError),
					crate::exceptions::JClassCircularityError =>
						Err(Error::ClassCircularityError),
					crate::exceptions::JOutOfMemoryError =>
						Err(Error::JniCall(JniError::NoMemory)),
					crate::exceptions::JSecurityException =>
						Err(Error::SecurityViolation),
					else => Err(Error::NullPtr("Unexpected Exception")),
				},
				self,
				v1_1,
				DefineClass,
				name,
				loader.as_raw(),
				buf,
				buf_len as jsize
			)
			.map(|class| JClass::from_raw(self, class))
		}
	}

	/// Load a class from a buffer of raw class data.
	///
	/// If `name` is `None` then the name of the loaded class will be inferred, otherwise the given
	/// `name` must match the name encoded within the class file data.
	///
	/// Typically the `loader` should come from [`JClassLoader::get_system_class_loader`] or the
	/// loader from some other application-specific class.
	///
	/// If `loader` is `null` then the bootstrap class loader will be used, which may not be able to
	/// load dependencies of the defined class.
	///
	/// Alternatively, call [`Self::define_class_jbyte`] if your data comes from a [`JByteArray`] or
	/// `&[jbyte]`.
	///
	/// - Returns [`Error::ClassFormatError`] if the class data does not specify a valid class.
	/// - Returns [`Error::ClassCircularityError`] if a class or interface would be its own
	///   superclass or superinterface.
	/// - Returns [`Error::JniCall`] + [`JniError::NoMemory`] if the JVM reports running out of
	///   memory.
	/// - Returns [`Error::SecurityViolation`] if the caller attempts to define a class in the
	///   `"java"` package tree.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Portability Note
	///
	/// The JNI `DefineClass` function may not be supported by all JVM implementations, even though
	/// it is part of the JNI specification.
	///
	/// In particular Android's ART runtime does not support it, since all classes must be
	/// pre-compiled into DEX format.
	pub fn define_class<'any_loader, N, L>(&mut self, name: Option<N>, loader: L, buf: &[u8]) -> Result<JClass<'local>>
	where
		N: AsRef<JNIStr>,
		L: AsRef<JClassLoader<'any_loader>>,
	{
		let name: Option<&JNIStr> = name.as_ref().map(|n| n.as_ref());
		let name = name.map_or(ptr::null(), |n| n.as_ptr());
		// Safety: we know the pointer for the u8 slice is valid for buf.len() bytes
		unsafe { self.define_class_impl(name, loader.as_ref(), buf.as_ptr() as *const jbyte, buf.len()) }
	}

	/// Load a class from a buffer of raw class data.
	///
	/// If `name` is `None` then the name of the loaded class will be inferred, otherwise the given
	/// `name` must match the name encoded within the class file data.
	///
	/// Typically the `loader` should come from [`JClassLoader::get_system_class_loader`] or the
	/// loader from some other application-specific class.
	///
	/// If `loader` is `null` then the bootstrap class loader will be used, which may not be able to
	/// load dependencies of the defined class.
	///
	/// This is the same as [`Self::define_class`] but takes a `&[jbyte]` instead of `&[u8]`.
	///
	/// - Returns [`Error::ClassFormatError`] if the class data does not specify a valid class.
	/// - Returns [`Error::ClassCircularityError`] if a class or interface would be its own
	///   superclass or superinterface.
	/// - Returns [`Error::JniCall`] + [`JniError::NoMemory`] if the JVM reports running out of
	///   memory.
	/// - Returns [`Error::SecurityViolation`] if the caller attempts to define a class in the
	///   `"java"` package tree.
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending exception).
	///
	/// # Portability Note
	///
	/// The JNI `DefineClass` function may not be supported by all JVM implementations, even though
	/// it is part of the JNI specification.
	///
	/// In particular Android's ART runtime does not support it, since all classes must be
	/// pre-compiled into DEX format.
	pub fn define_class_jbyte<'any_loader, N, L>(
		&mut self,
		name: Option<N>,
		loader: L,
		buf: &[jbyte],
	) -> Result<JClass<'local>>
	where
		N: AsRef<JNIStr>,
		L: AsRef<JClassLoader<'any_loader>>,
	{
		let name: Option<&JNIStr> = name.as_ref().map(|n| n.as_ref());
		let name = name.as_ref().map_or(ptr::null(), |n| n.as_ptr());
		// Safety: we know the pointer for the u8 slice is valid for buf.len() bytes
		unsafe { self.define_class_impl(name, loader.as_ref(), buf.as_ptr() as *const jbyte, buf.len()) }
	}

	/// Look up a class by its fully-qualified "internal" JNI name, via JNI
	/// `FindClass`.
	///
	/// **Note:** Only use this method if you _strictly_ need the JNI
	/// `FindClass` behavior exactly, otherwise, it is strongly recommended to
	/// use [Reference::lookup_class] (cached), [Env::load_class] or
	/// [LoaderContext::load_class].
	///
	/// The `name` must be in the "internal" form, with slashes instead of dots,
	/// like `jni_str!("java/lang/IllegalArgumentException")` or an array
	/// descriptor like `jni_str!("[Ljava/lang/Number;")`.
	///
	/// # Example
	/// ```rust,no_run
	/// # use abos_jni::{jni_str, errors::Result, Env, objects::JClass};
	/// #
	/// # fn example<'local>(env: &mut Env<'local>) -> Result<()> {
	/// let class: JClass<'local> = env.find_class(jni_str!("java/lang/IllegalArgumentException"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// - Returns a reference to loaded class on success
	/// - Returns [`Error::NoClassDefFound`] if a `NoClassDefFoundError`
	///   exception occurs, indicating that no definition for a requested class
	///   or interface can be found (most common failure)
	/// - Returns [`Error::ClassFormatError`] if a `ClassFormatError` exception
	///   occurs, indicating that the class data does not specify a valid class.
	/// - Returns [`Error::ClassCircularityError`] if a `ClassCircularityError`
	///   exception occurs, indicating that a class or interface would be its
	///   own superclass or superinterface.
	/// - Returns [`Error::JniCall`] + [`JniError::NoMemory`] if an
	///   `OutOfMemoryError` exception occurs, indicating that the JVM reports
	///   running out of memory.
	/// - Returns [`Error::NullPtr`] for any other unknown error condition
	///
	/// This API catches exceptions internally and is not expected to return
	/// [`Error::JavaException`] (unless called while there is a pending
	/// exception).
	///
	/// # Android
	///
	/// On Android in particular it's notable that `FindClass` will not be able
	/// to find application classes when called from a native thread, outside of
	/// a native method call.
	///
	/// [Env::load_class] can work on Android if a loader has been set up for
	/// the current thread (see: [JThread::set_context_class_loader]) and if you
	/// use the `android-activity` crate this may be done for you.
	pub fn find_class<S>(&mut self, name: S) -> Result<JClass<'local>>
	where
		S: AsRef<JNIStr>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let name = name.as_ref();
		// Safety:
		// FindClass is 1.1 API that must be valid
		// name is non-null
		unsafe {
			jni_call_with_catch_and_null_check!(
                catch |env| {
                    e: crate::exceptions::JNoClassDefFoundError => {
                        let cause: &JThrowable = &e.as_throwable();
                        let cause = env.new_global_ref(cause).ok();
                        Err(Error::NoClassDefFound {
                            requested: name.to_string(),
                            cause
                        })
                    },
                    // Although the JNI spec does not list `ClassNotFoundException` as a possible
                    // exception for `FindClass` we want to make sure that would be mapped to
                    // Error:NoClassDefFound, consistent with `LoaderContext::load_class`
                    e: crate::exceptions::JClassNotFoundException => {
                        let cause: &JThrowable = &e.as_throwable();
                        let cause = env.new_global_ref(cause).ok();
                        Err(Error::NoClassDefFound {
                            requested: name.to_string(),
                            cause
                        })
                    },
                    crate::exceptions::JClassFormatError =>
                        Err(Error::ClassFormatError),
                    crate::exceptions::JClassCircularityError =>
                        Err(Error::ClassCircularityError),

                    // Although the JNI spec doesn't say that `FindClass` may throw `LinkageErrors`,
                    // we want to make sure we would map them to `Error::LinkageError`, consistent
                    // with `LoaderContext::load_class`
                    e: crate::exceptions::JLinkageError => {
                        let cause: &JThrowable = &e.as_throwable();
                        let cause = env.new_global_ref(cause).ok();
                        Err(Error::LinkageError {
                            requested: name.to_string(),
                            cause
                        })
                    },
                    crate::exceptions::JOutOfMemoryError =>
                        Err(Error::JniCall(JniError::NoMemory)),
                    else => Err(Error::NullPtr("Unexpected exception")),
                },
                self, v1_1, FindClass, name.as_ptr())
			.map(|class| JClass::from_raw(self, class))
		}
	}

	/// Look up a class by its fully-qualified name, via `Class.forName(<name>,
	/// <initialize>, <loader>)` with a `FindClass` fallback.
	///
	/// This is a convenience wrapper around [LoaderContext::load_class] with
	/// [LoaderContext::None].
	///
	/// `name` should be a binary name like `jni_str!("java.lang.Number")` or an
	/// array descriptor like `jni_str!("[Ljava.lang.Number;")`.
	///
	/// **Note:** that unlike [Env::find_class], the name uses **dots instead of
	/// slashes** and should conform to the format that `Class.getName()`
	/// returns and that `Class.forName()` expects.
	///
	/// For example, lookup the class for
	/// `java.lang.IllegalArgumentException`like this:
	/// ```rust,no_run
	/// # use abos_jni::{jni_str, errors::Result, Env, objects::JClass};
	/// #
	/// # fn example<'local>(env: &mut Env<'local>) -> Result<()> {
	/// let class: JClass<'local> = env.load_class(jni_str!("java.lang.IllegalArgumentException"))?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// - Returns a reference to loaded class on success
	/// - Returns [`Error::NoClassDefFound`] if a `NoClassDefFoundError` or
	///   `ClassNotFoundException` exception occurs, indicating that no
	///   definition for a requested class or interface can be found (most
	///   common failure)
	/// - Returns [`Error::ClassFormatError`] if a `ClassFormatError` exception
	///   occurs, indicating that the class data does not specify a valid class.
	/// - Returns [`Error::ClassCircularityError`] if a `ClassCircularityError`
	///   exception occurs, indicating that a class or interface would be its
	///   own superclass or superinterface.
	/// - May Return [`Error::LinkageError`] for various failures to link the
	///   requested class.
	/// - May Return [`Error::ExceptionInInitializer`] if an exception occurs
	///   during class initialization.
	/// - Returns [`Error::JniCall`] + [`JniError::NoMemory`] if an
	///   `OutOfMemoryError` exception occurs, indicating that the JVM reports
	///   running out of memory.
	/// - Returns [`Error::NullPtr`] for any other unknown error condition
	///
	/// *Note:* In order for this API to be consistent with
	/// [`abos_jni::Env::find_class`], this API does _not_ directly return
	/// [`Error::ClassNotFound`] errors that may come directly from the loader.
	///
	/// *Note:* Any [`Error::NoClassDefFound`], [`Error::LinkageError`] or
	/// [`Error::ExceptionInInitializer`] errors include the original `cause`
	/// exception if more details are needed.
	///
	/// # Android
	///
	/// This API will notably check the context class loader of the current
	/// thread, which makes it possible to associate a thread with an
	/// application class loader.
	///
	/// This may be particularly useful for native applications on Android
	/// because native threads will not normally be able to find application
	/// classes through `FindClass`.
	///
	/// If you use the `android-activity` crate, it may set up the context class
	/// loader for you.
	pub fn load_class<S>(&mut self, name: S) -> Result<JClass<'local>>
	where
		S: AsRef<JNIStr>,
	{
		let name = name.as_ref();
		LoaderContext::None.load_class(self, name, false)
	}

	/// Returns the superclass for a particular class. Returns None for `java.lang.Object` or
	/// an interface. As with [Self::find_class], takes a descriptor
	///
	/// # Errors
	///
	/// If a JNI call fails
	pub fn get_superclass<'other_local, T>(&mut self, class: T) -> Result<Option<JClass<'local>>>
	where
		T: Desc<'local, JClass<'other_local>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let class = class.lookup(self)?;
		let superclass = unsafe {
			JClass::from_raw(
				self,
				jni_call_no_post_check_ex!(self, v1_1, GetSuperclass, class.as_ref().as_raw())?,
			)
		};

		Ok((!superclass.is_null()).then_some(superclass))
	}

	// Like is_assignable_from but it doesn't need a mutable Env reference because it doesn't do any
	// descriptor lookups.
	pub(super) fn is_assignable_from_class(&self, class1: &JClass, class2: &JClass) -> Result<bool> {
		let class1 = null_check!(class1, "is_assignable_from class1")?;
		let class2 = null_check!(class2, "is_assignable_from class2")?;

		// Safety:
		// - IsAssignableFrom is 1.1 API that must be valid
		// - We make sure class1 and class2 can't be null
		unsafe {
			Ok(jni_call_no_post_check_ex!(
				self,
				v1_1,
				IsAssignableFrom,
				class1.as_raw(), // MUST not be null
				class2.as_raw()  // MUST not be null
			)?)
		}
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JClass>::lookup`)
	//
	/// Tests whether class1 is assignable from class2.
	pub fn is_assignable_from<'other_local_1, 'other_local_2, T, U>(&mut self, class1: T, class2: U) -> Result<bool>
	where
		T: Desc<'local, JClass<'other_local_1>>,
		U: Desc<'local, JClass<'other_local_2>>,
	{
		let class1 = class1.lookup(self)?;
		let class1 = null_check!(class1.as_ref(), "is_assignable_from class1")?;
		let class2 = class2.lookup(self)?;
		let class2 = null_check!(class2.as_ref(), "is_assignable_from class2")?;

		self.is_assignable_from_class(class1, class2)
	}

	/// Checks if an object can be cast to a specific reference type.
	pub(crate) fn is_instance_of_cast_type<To: Reference>(&self, obj: &JObject) -> Result<bool> {
		let class = match To::lookup_class(self, &LoaderContext::FromObject(obj)) {
			Ok(class) => class,
			Err(Error::ClassNotFound { name: _ }) => return Ok(false),
			Err(e) => return Err(e),
		};

		let class: &JClass = &class;
		self.is_instance_of_class(obj, class)
	}

	// An internal helper that implements is_instance_of except it doesn't take a
	// Desc for the class and doesn't need a mutable Env reference since it never
	// needs to allocate a new local reference.
	/// Returns true if the object reference can be cast to the given type.
	pub(crate) fn is_instance_of_class<'other_local_1, 'other_local_2, O, C>(&self, object: O, class: C) -> Result<bool>
	where
		O: AsRef<JObject<'other_local_1>>,
		C: AsRef<JClass<'other_local_2>>,
	{
		let class = null_check!(class.as_ref(), "is_instance_of class")?;

		// Safety:
		// - IsInstanceOf is 1.1 API that must be valid
		// - We make sure class can't be null
		unsafe {
			Ok(jni_call_no_post_check_ex!(
				self,
				v1_1,
				IsInstanceOf,
				object.as_ref().as_raw(), // may be null
				class.as_raw()            // MUST not be null
			)?)
		}
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JClass>::lookup`)
	//
	/// Returns true if the object reference can be cast to the given type.
	///
	/// _NB: Unlike the operator `instanceof`, function `IsInstanceOf` *returns `true`*
	/// for all classes *if `object` is `null`.*_
	///
	/// See [JNI documentation](https://docs.oracle.com/javase/8/docs/technotes/guides/jni/spec/functions.html#IsInstanceOf)
	/// for details.
	pub fn is_instance_of<'other_local_1, 'other_local_2, O, T>(&mut self, object: O, class: T) -> Result<bool>
	where
		O: AsRef<JObject<'other_local_1>>,
		T: Desc<'local, JClass<'other_local_2>>,
	{
		let class = class.lookup(self)?;
		self.is_instance_of_class(object, class)
	}

	/// Returns true if ref1 and ref2 refer to the same Java object, or are both `NULL`. Otherwise,
	/// returns false.
	///
	/// Returns [`Error::JavaException`] if called while there is a pending Java exception
	pub fn is_same_object<'other_local_1, 'other_local_2, O, T>(&self, ref1: O, ref2: T) -> Result<bool>
	where
		O: AsRef<JObject<'other_local_1>>,
		T: AsRef<JObject<'other_local_2>>,
	{
		// Safety:
		// - IsSameObject is 1.1 API that must be valid
		// - the spec allows either object reference to be `null`
		unsafe {
			jni_call_no_post_check_ex!(
				self,
				v1_1,
				IsSameObject,
				ref1.as_ref().as_raw(), // may be null
				ref2.as_ref().as_raw()  // may be null
			)
		}
	}

	// FIXME: this API shouldn't need a `&mut self` reference since it doesn't return a local reference
	// (currently it just needs the `&mut self` for the sake of `Desc<JThrowable>::lookup`)
	//
}
