use crate::sys::jboolean;

use super::{
	Desc, Env, JClass, JMethodID, JObject, JStaticMethodID, JValueOwned, JavaType, Reference, Result, ReturnType,
	jobject, jvalue,
};

impl<'local> Env<'local> {
	/// Call a static method without validating its class, signature, or arguments.
	///
	/// # Safety
	///
	/// The resolved class must be non-null, and the `JStaticMethodID` must be valid for it.
	/// The argument count and types and the return type must match the method's signature.
	/// Reference arguments must be valid JNI references (or null) assignable to their
	/// declared parameter types. Violations can cause undefined behavior or crash the JVM.
	///
	/// # Errors
	///
	/// Returns an error if descriptor resolution fails or the Java invocation reports an exception.
	pub unsafe fn call_static_method_unchecked<'other_local, T, U>(
		&mut self,
		class: T,
		method_id: U,
		ret: ReturnType,
		args: &[jvalue],
	) -> Result<JValueOwned<'local>>
	where
		T: Desc<'local, JClass<'other_local>>,
		U: Desc<'local, JStaticMethodID>,
	{
		use crate::signature::Primitive::{Boolean, Byte, Char, Double, Float, Int, Long, Short, Void};
		use JavaType::{Array, Object, Primitive};

		self.assert_top();

		let class: <T as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;

		let method_id: crate::sys::jmethodID = method_id.lookup(self)?.as_ref().into_raw();

		let class_raw: crate::sys::jobject = class.as_ref().as_raw();
		let jni_args: *const abos_jni_sys::jvalue = args.as_ptr();

		macro_rules! invoke {
			($call:ident -> $ret:ty) => {{
				let o: $ret = jni_call_post_check_ex!(self, v1_1, $call, class_raw, method_id, jni_args)?;
				o
			}};
		}

		// Safety: we're sure we're calling the correct JNI method based on `ret` type
		let ret: JValueOwned<'_> = unsafe {
			match ret {
				Object | Array => {
					let obj: crate::sys::jobject = invoke!(CallStaticObjectMethodA -> jobject);
					let obj: JObject<'_> = JObject::from_raw(self, obj);
					JValueOwned::from(obj)
				}
				Primitive(Boolean) => invoke!(CallStaticBooleanMethodA -> jboolean).into(),
				Primitive(Char) => invoke!(CallStaticCharMethodA -> u16).into(),
				Primitive(Byte) => invoke!(CallStaticByteMethodA -> i8).into(),
				Primitive(Short) => invoke!(CallStaticShortMethodA -> i16).into(),
				Primitive(Int) => invoke!(CallStaticIntMethodA -> i32).into(),
				Primitive(Long) => invoke!(CallStaticLongMethodA -> i64).into(),
				Primitive(Float) => invoke!(CallStaticFloatMethodA -> f32).into(),
				Primitive(Double) => invoke!(CallStaticDoubleMethodA -> f64).into(),
				Primitive(Void) => {
					jni_call_post_check_ex!(self, v1_1, CallStaticVoidMethodA, class_raw, method_id, jni_args)?;
					JValueOwned::Void
				}
			}
		};

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		Ok(ret)
	}

	/// Call an instance method without validating its receiver, signature, or arguments.
	///
	/// # Safety
	///
	/// The provided `JMethodID` must be valid, and match the types and number of arguments, and return type.
	/// If these are incorrect, the JVM may crash. The `JMethodID` must also match the passed type.
	///
	/// # Errors
	///
	/// Returns an error if descriptor resolution fails or the Java invocation reports an exception.
	pub unsafe fn call_method_unchecked<'other_local, O, T>(
		&mut self,
		obj: O,
		method_id: T,
		ret_ty: ReturnType,
		args: &[jvalue],
	) -> Result<JValueOwned<'local>>
	where
		O: AsRef<JObject<'other_local>>,
		T: Desc<'local, JMethodID>,
	{
		use crate::signature::Primitive::{Boolean, Byte, Char, Double, Float, Int, Long, Short, Void};
		use JavaType::{Array, Object, Primitive};

		self.assert_top();

		let method_id: crate::sys::jmethodID = method_id.lookup(self)?.as_ref().into_raw();

		let obj: crate::sys::jobject = obj.as_ref().as_raw();

		let jni_args: *const abos_jni_sys::jvalue = args.as_ptr();

		macro_rules! invoke {
			($call:ident -> $ret:ty) => {{
				let o: $ret = jni_call_post_check_ex!(self, v1_1, $call, obj, method_id, jni_args)?;
				o
			}};
		}

		// Safety: we're sure we're calling the correct JNI method based on `ret_ty` type
		let ret: JValueOwned<'_> = unsafe {
			match ret_ty {
				Object | Array => {
					let obj: crate::sys::jobject = invoke!(CallObjectMethodA -> jobject);
					let obj: JObject<'_> = JObject::from_raw(self, obj);
					JValueOwned::from(obj)
				}
				Primitive(Boolean) => invoke!(CallBooleanMethodA -> jboolean).into(),
				Primitive(Char) => invoke!(CallCharMethodA -> u16).into(),
				Primitive(Byte) => invoke!(CallByteMethodA -> i8).into(),
				Primitive(Short) => invoke!(CallShortMethodA -> i16).into(),
				Primitive(Int) => invoke!(CallIntMethodA -> i32).into(),
				Primitive(Long) => invoke!(CallLongMethodA -> i64).into(),
				Primitive(Float) => invoke!(CallFloatMethodA -> f32).into(),
				Primitive(Double) => invoke!(CallDoubleMethodA -> f64).into(),
				Primitive(Void) => {
					jni_call_post_check_ex!(self, v1_1, CallVoidMethodA, obj, method_id, jni_args)?;
					JValueOwned::Void
				}
			}
		};

		Ok(ret)
	}

	/// Call a non-virtual method without validating its receiver, class, signature, or arguments.
	///
	/// # Safety
	///
	/// The provided `JClass`, `JMethodID` must be valid, and match the types and number of arguments, and return type.
	/// If these are incorrect, the JVM may crash. The `JMethodID` must also match the passed type.
	///
	/// # Errors
	///
	/// Returns an error if descriptor resolution fails or the Java invocation reports an exception.
	pub unsafe fn call_nonvirtual_method_unchecked<'other_local, O, T, U>(
		&mut self,
		obj: O,
		class: T,
		method_id: U,
		ret_ty: ReturnType,
		args: &[jvalue],
	) -> Result<JValueOwned<'local>>
	where
		O: AsRef<JObject<'other_local>>,
		T: Desc<'local, JClass<'other_local>>,
		U: Desc<'local, JMethodID>,
	{
		use crate::signature::Primitive::{Boolean, Byte, Char, Double, Float, Int, Long, Short, Void};
		use JavaType::{Array, Object, Primitive};

		self.assert_top();

		let method_id: crate::sys::jmethodID = method_id.lookup(self)?.as_ref().into_raw();
		let class: <T as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;

		let obj: crate::sys::jobject = obj.as_ref().as_raw();
		let class_raw: crate::sys::jobject = class.as_ref().as_raw();

		let jni_args: *const abos_jni_sys::jvalue = args.as_ptr();

		macro_rules! invoke {
			($call:ident -> $ret:ty) => {{
				let o: $ret = jni_call_post_check_ex!(self, v1_1, $call, obj, class_raw, method_id, jni_args)?;
				o
			}};
		}

		// Safety: we're sure we're calling the correct JNI method based on `ret_ty` type
		let ret: JValueOwned<'_> = unsafe {
			match ret_ty {
				Object | Array => {
					let obj: crate::sys::jobject = invoke!(CallNonvirtualObjectMethodA -> jobject);
					let obj: JObject<'_> = JObject::from_raw(self, obj);
					JValueOwned::from(obj)
				}
				Primitive(Boolean) => invoke!(CallNonvirtualBooleanMethodA -> jboolean).into(),
				Primitive(Char) => invoke!(CallNonvirtualCharMethodA -> u16).into(),
				Primitive(Byte) => invoke!(CallNonvirtualByteMethodA -> i8).into(),
				Primitive(Short) => invoke!(CallNonvirtualShortMethodA -> i16).into(),
				Primitive(Int) => invoke!(CallNonvirtualIntMethodA -> i32).into(),
				Primitive(Long) => invoke!(CallNonvirtualLongMethodA -> i64).into(),
				Primitive(Float) => invoke!(CallNonvirtualFloatMethodA -> f32).into(),
				Primitive(Double) => invoke!(CallNonvirtualDoubleMethodA -> f64).into(),
				Primitive(Void) => {
					jni_call_post_check_ex!(
						self,
						v1_1,
						CallNonvirtualVoidMethodA,
						obj,
						class_raw,
						method_id,
						jni_args
					)?;
					JValueOwned::Void
				}
			}
		};

		Ok(ret)
	}
}
