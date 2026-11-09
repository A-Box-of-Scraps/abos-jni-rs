use crate::{
	errors::{Error, JniError},
	objects::JThrowable,
};

/// Compile-check typed exception handlers, including handlers that retain Java causes.
fn _compile_test_jni_call_with_catch_and_null_check(
	env: &mut crate::Env<'_>,
	name: &crate::strings::JNIStr,
	sig: &crate::strings::JNIStr,
) -> crate::errors::Result<crate::sys::jclass> {
	// SAFETY: The caller guarantees valid class bytes and JNI references; the call checks exceptions and null returns.
	unsafe {
		jni_call_with_catch_and_null_check!(
			catch |env| {
				crate::exceptions::JOutOfMemoryError =>
					Err(Error::JniCall(JniError::NoMemory)),

				crate::exceptions::JClassFormatError =>
					Err(Error::ClassFormatError),

				crate::exceptions::JClassCircularityError =>
					Err(Error::ClassCircularityError),

				e: crate::exceptions::JClassNotFoundException => {
					let cause: &JThrowable = &e.as_throwable();
					let cause: Option<crate::refs::Global<JThrowable<'_>>> = env.new_global_ref(cause).ok();
					Err(Error::NoClassDefFound {
						requested: name.to_string(),
						cause
					})
				},
				e: crate::exceptions::JNoClassDefFoundError => {
					let cause: &JThrowable = &e.as_throwable();
					let cause: Option<crate::refs::Global<JThrowable<'_>>> = env.new_global_ref(cause).ok();
					Err(Error::NoClassDefFound {
						requested: name.to_string(),
						cause
					})
				},

				e: crate::exceptions::JExceptionInInitializerError => {
					let exception: std::result::Result<JThrowable<'_>, crate::errors::Error> = e.get_exception(env);
					let exception: Option<crate::refs::Global<JThrowable<'_>>> = exception.map_or_else(|_| None, |exception| env.new_global_ref(exception).ok());
					Err(Error::ExceptionInInitializer { exception })
				},

				crate::exceptions::JNoSuchMethodError =>
					Err(Error::MethodNotFound {
						name: name.to_string(),
						sig: sig.to_string(),
					}),

				crate::exceptions::JNoSuchFieldError =>
					Err(Error::FieldNotFound {
						name: name.to_string(),
						sig: sig.to_string(),
					}),

				crate::exceptions::JArrayStoreException =>
					Err(Error::WrongObjectType),

				crate::exceptions::JIllegalArgumentException =>
					Err(Error::JniCall(JniError::InvalidArguments)),

				crate::exceptions::JIllegalMonitorStateException =>
					Err(Error::IllegalMonitorState),

				e: crate::exceptions::JLinkageError => {
					let cause: &JThrowable = &e.as_throwable();
					let cause: Option<crate::refs::Global<JThrowable<'_>>> = env.new_global_ref(cause).ok();
					Err(Error::LinkageError {
						requested: name.to_string(),
						cause
					})
				},

				crate::exceptions::JSecurityException =>
					Err(Error::SecurityViolation),

				crate::exceptions::JArrayIndexOutOfBoundsException =>
					Err(Error::IndexOutOfBounds),

				crate::exceptions::JStringIndexOutOfBoundsException =>
					Err(Error::IndexOutOfBounds),

				crate::exceptions::JInstantiationException =>
					Err(Error::Instantiation),

				crate::exceptions::JNumberFormatException =>
					Err(Error::ParseFailed(String::new())),

				else => Err(Error::NullPtr("Unexpected Exception"))
			},
			env,
			v1_1,
			FindClass,
			core::ptr::null()
		)
	}
}

fn _compile_test_jni_try(env: &mut crate::Env<'_>) -> crate::errors::Result<()> {
	let _res: crate::objects::JString<'_> = jni_try! {
		(env) -> crate::errors::Result<crate::objects::JString> {
			let s: crate::objects::JString<'_> = env.new_string("String")?;

			{
				let msg: crate::objects::JString<'_> = env.new_string("Test")?;
				let e: crate::api::exceptions::JNumberFormatException<'_> = crate::exceptions::JNumberFormatException::new(env, msg)?;
				let e: crate::objects::JThrowable = e.into();
				env.throw(e)?;
			}

			Ok(s)
		} catch |env| {
			crate::exceptions::JNumberFormatException => Err(Error::ParseFailed(String::new())),
			else => Err(Error::NullPtr("Test"))
		}
	}?;

	Ok(())
}

#[test]
fn arguments_are_evaluated_before_exception_check_and_temporaries_survive() {
	use std::cell::RefCell;

	struct PendingEnv {
		events: RefCell<Vec<&'static str>>,
	}

	impl PendingEnv {
		fn exception_check(&self) -> bool {
			self.events.borrow_mut().push("check");
			true
		}

		fn get_raw(&self) -> *mut crate::sys::JNIEnv {
			panic!(
				"unexpected access to the JNI function table after {:?}",
				self.events.borrow()
			);
		}
	}

	struct Argument<'a>(&'a PendingEnv);

	impl Argument<'_> {
		fn as_raw(&self) -> crate::sys::jobject {
			self.0.events.borrow_mut().push("value");
			std::ptr::null_mut()
		}
	}

	impl Drop for Argument<'_> {
		fn drop(&mut self) {
			self.0.events.borrow_mut().push("drop");
		}
	}

	let env: PendingEnv = PendingEnv {
		events: RefCell::new(Vec::new()),
	};
	// SAFETY: The mock reports a pending exception, so no raw JNI call may occur.
	let result: crate::errors::Result<()> = unsafe {
		jni_call_no_post_check_ex!(
			env,
			v1_1,
			SetObjectArrayElement,
			{
				env.events.borrow_mut().push("array");
				std::ptr::null_mut()
			},
			{
				env.events.borrow_mut().push("index");
				0
			},
			Argument(&env).as_raw()
		)
	};
	assert!(matches!(result, Err(Error::JavaException)));
	assert_eq!(*env.events.borrow(), ["array", "index", "value", "check", "drop"]);
}
