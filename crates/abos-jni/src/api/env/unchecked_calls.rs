use super::{
	Desc, Env, JClass, JMethodID, JObject, JStaticMethodID, JValueOwned, JavaType, Reference, Result, ReturnType,
	jobject, jvalue,
};

impl<'local> Env<'local> {
	/// Call a static method in an unsafe manner. This does nothing to check
	/// whether the method is valid to call on the class, whether the return
	/// type is correct, or whether the number of args is valid for the method.
	///
	/// Under the hood, this simply calls the `CallStatic<Type>MethodA` method
	/// with the provided arguments.
	///
	/// # Safety
	///
	/// The provided `JMethodID` must be valid, and match the types and number of arguments, and return type.
	/// If these are incorrect, the JVM may crash. The `JMethodID` must also match the passed type.
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

		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();

		let class = class.lookup(self)?;

		let method_id = method_id.lookup(self)?.as_ref().into_raw();

		let class_raw = class.as_ref().as_raw();
		let jni_args = args.as_ptr();

		macro_rules! invoke {
			($call:ident -> $ret:ty) => {{
				let o: $ret = jni_call_post_check_ex!(self, v1_1, $call, class_raw, method_id, jni_args)?;
				o
			}};
		}

		// Safety: we're sure we're calling the correct JNI method based on `ret` type
		let ret = unsafe {
			match ret {
				Object | Array => {
					let obj = invoke!(CallStaticObjectMethodA -> jobject);
					let obj = JObject::from_raw(self, obj);
					JValueOwned::from(obj)
				}
				Primitive(Boolean) => invoke!(CallStaticBooleanMethodA -> bool).into(),
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

	/// Call an object method in an unsafe manner. This does nothing to check
	/// whether the method is valid to call on the object, whether the return
	/// type is correct, or whether the number of args is valid for the method.
	///
	/// Under the hood, this simply calls the `Call<Type>MethodA` method with
	/// the provided arguments.
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

		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();

		let method_id = method_id.lookup(self)?.as_ref().into_raw();

		let obj = obj.as_ref().as_raw();

		let jni_args = args.as_ptr();

		macro_rules! invoke {
			($call:ident -> $ret:ty) => {{
				let o: $ret = jni_call_post_check_ex!(self, v1_1, $call, obj, method_id, jni_args)?;
				o
			}};
		}

		// Safety: we're sure we're calling the correct JNI method based on `ret_ty` type
		let ret = unsafe {
			match ret_ty {
				Object | Array => {
					let obj = invoke!(CallObjectMethodA -> jobject);
					let obj = JObject::from_raw(self, obj);
					JValueOwned::from(obj)
				}
				Primitive(Boolean) => invoke!(CallBooleanMethodA -> bool).into(),
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

	/// Call an non-virtual object method in an unsafe manner. This does nothing to check
	/// whether the method is valid to call on the object, whether the return
	/// type is correct, or whether the number of args is valid for the method.
	///
	/// Under the hood, this simply calls the `CallNonvirtual<Type>MethodA` method with
	/// the provided arguments.
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

		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();

		let method_id = method_id.lookup(self)?.as_ref().into_raw();
		let class = class.lookup(self)?;

		let obj = obj.as_ref().as_raw();
		let class_raw = class.as_ref().as_raw();

		let jni_args = args.as_ptr();

		macro_rules! invoke {
			($call:ident -> $ret:ty) => {{
				let o: $ret = jni_call_post_check_ex!(self, v1_1, $call, obj, class_raw, method_id, jni_args)?;
				o
			}};
		}

		// Safety: we're sure we're calling the correct JNI method based on `ret_ty` type
		let ret = unsafe {
			match ret_ty {
				Object | Array => {
					let obj = invoke!(CallNonvirtualObjectMethodA -> jobject);
					let obj = JObject::from_raw(self, obj);
					JValueOwned::from(obj)
				}
				Primitive(Boolean) => invoke!(CallNonvirtualBooleanMethodA -> bool).into(),
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
