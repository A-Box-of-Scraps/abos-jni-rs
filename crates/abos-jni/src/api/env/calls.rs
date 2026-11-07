use super::reference_validation::reference_args;
#[cfg(doc)]
use crate::jni_sig;

use super::{
	Desc, Env, Error, IntoAuto, JClass, JMethodID, JNIStr, JObject, JStaticMethodID, JValue, JValueOwned, JavaType,
	MethodSignature, Primitive, Reference, Result, ReturnType, jvalue,
};

// Reference assignability is checked separately after resolving the method ID.
fn validate_call_args(sig: &MethodSignature<'_, '_>, args: &[JValue]) -> Result<()> {
	if sig.args().len() != args.len() {
		return Err(Error::InvalidArgList(sig.into()));
	}
	let base_types_match: bool = sig.args().iter().zip(args).all(|(expected, actual)| match expected {
		JavaType::Primitive(primitive) => actual.primitive_type() == Some(*primitive),
		JavaType::Object | JavaType::Array => actual.primitive_type().is_none(),
	});
	if !base_types_match {
		return Err(Error::InvalidArgList(sig.into()));
	}
	Ok(())
}

impl<'local> Env<'local> {
	/// Calls an object method safely. This comes with a number of lookups/checks. It
	///
	/// * Looks up the [`JClass`] for the given object.
	/// * Looks up the [`JMethodID`] for the object's class/name/signature combination
	/// * Checks primitive argument types and reference assignability to the resolved parameter classes
	/// * Calls [`Self::call_method_unchecked`] with the verified safe arguments.
	///
	/// Incompatible references return [`Error::WrongObjectType`] before the method is invoked.
	///
	/// # Example
	///
	/// ```rust,no_run
	/// # use abos_jni::{jni_sig, jni_str, errors::Result, Env, objects::JObject, JValue};
	/// # fn example<'local>(env: &mut Env<'local>, obj: &JObject<'local>) -> Result<()> {
	/// // Call a method with signature: String concat(String str)
	/// let arg = env.new_string("world")?;
	/// let result = env.call_method(
	///     obj,
	///     jni_str!("concat"),
	///     jni_sig!((str: JString) -> JString),
	///     &[JValue::Object(&arg)],
	/// )?;
	/// # Ok(())
	/// # }
	/// ```
	/// Note: a traditional, raw JNI signature like `"(Ljava/lang/String;)Ljava/lang/String;"` can also be used with
	/// [`jni_sig!`] but this demonstrates the more ergonomic syntax that the macro allows. See the [`jni_sig!`] macro
	/// documentation for more details and examples.
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] for incompatible reference arguments.
	/// Returns an error if class or method lookup fails, arguments do not match the signature, or Java throws an exception.
	pub fn call_method<'other_local, 'sig, 'sig_args, O, N, S>(
		&mut self,
		obj: O,
		name: N,
		sig: S,
		args: &[JValue],
	) -> Result<JValueOwned<'local>>
	where
		O: AsRef<JObject<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let obj: &JObject<'_> = obj.as_ref();
		let obj: &JObject<'_> = null_check!(obj, "call_method obj argument")?;
		let sig: &MethodSignature<'_, '_> = sig.as_ref();

		validate_call_args(sig, args)?;

		let class: crate::refs::Auto<'_, JClass<'_>> = self.get_object_class(obj)?.auto();

		let method_id: JMethodID = Desc::<JMethodID>::lookup((&class, name, sig), self)?;
		// SAFETY: The method ID was resolved from this class and signature.
		unsafe {
			self.__validate_method_reference_args(&class, method_id.into_raw(), false, &reference_args(args))?;
		}

		let args: Vec<jvalue> = args.iter().map(super::super::jvalue::JValue::as_jni).collect();

		// SAFETY: We've ensured that the Desc trait will be used to dynamically look up a valid
		// method ID for the object's class, method name, and signature (or else it will return an
		// Err). We've also validated the argument counts and types using the same type signature we
		// fetched the original method ID from.
		unsafe { self.call_method_unchecked(obj, method_id, sig.ret(), &args) }
	}

	/// Calls a static method safely. This comes with a number of
	/// lookups/checks. It
	///
	/// * Looks up the [`JMethodID`] for the class/name/signature combination
	/// * Checks primitive argument types and reference assignability to the resolved parameter classes
	/// * Calls [`Self::call_static_method_unchecked`] with the verified safe arguments.
	///
	/// Incompatible references return [`Error::WrongObjectType`] before the method is invoked.
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] for incompatible reference arguments.
	/// Returns an error if class or method lookup fails, arguments do not match the signature, or Java throws an exception.
	pub fn call_static_method<'other_local, 'sig, 'sig_args, C, N, S>(
		&mut self,
		class: C,
		name: N,
		sig: S,
		args: &[JValue],
	) -> Result<JValueOwned<'local>>
	where
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let sig: &MethodSignature<'_, '_> = sig.as_ref();
		validate_call_args(sig, args)?;

		// go ahead and look up the class since we'll need that for the next call.
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		let class: &JClass<'_> = class.as_ref();

		let method_id: JStaticMethodID = *Desc::<JStaticMethodID>::lookup((class, name, sig), self)?.as_ref();
		// SAFETY: The method ID was resolved from this class and signature.
		unsafe {
			self.__validate_method_reference_args(class, method_id.into_raw(), true, &reference_args(args))?;
		}

		let args: Vec<jvalue> = args.iter().map(super::super::jvalue::JValue::as_jni).collect();

		// SAFETY: We've obtained the method_id above, so it is valid for this class.
		// We've also validated the argument counts and types using the same type signature
		// we fetched the original method ID from.
		unsafe { self.call_static_method_unchecked(class, method_id, sig.ret(), &args) }
	}

	/// Calls a non-virtual method safely. This comes with a number of
	/// lookups/checks. It
	///
	/// * Ensures that the receiver is an instance of the supplied class.
	/// * Looks up the [`JMethodID`] for the class/name/signature combination
	/// * Checks primitive argument types and reference assignability to the resolved parameter classes
	/// * Calls [`Self::call_nonvirtual_method_unchecked`] with the verified safe arguments.
	///
	/// Incompatible references return [`Error::WrongObjectType`] before the method is invoked.
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] for an incompatible receiver or reference argument.
	/// Returns an error if the receiver is null, class or method lookup fails, arguments do not match the signature,
	/// or Java throws an exception.
	pub fn call_nonvirtual_method<'other_local, 'sig, 'sig_args, O, C, N, S>(
		&mut self,
		obj: O,
		class: C,
		name: N,
		sig: S,
		args: &[JValue],
	) -> Result<JValueOwned<'local>>
	where
		O: AsRef<JObject<'other_local>>,
		C: Desc<'local, JClass<'other_local>>,
		N: AsRef<JNIStr>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let obj: &JObject<'_> = obj.as_ref();
		let obj: &JObject<'_> = null_check!(obj, "call_nonvirtual_method obj argument")?;
		let sig: &MethodSignature<'_, '_> = sig.as_ref();
		validate_call_args(sig, args)?;

		// go ahead and look up the class since we'll need that for the next call.
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		let class: &JClass<'_> = class.as_ref();

		if !self.is_instance_of(obj, class)? {
			return Err(Error::WrongObjectType);
		}

		let method_id: JMethodID = Desc::<JMethodID>::lookup((class, name, sig), self)?;
		// SAFETY: The method ID was resolved from this class and signature.
		unsafe {
			self.__validate_method_reference_args(class, method_id.into_raw(), false, &reference_args(args))?;
		}

		let args: Vec<jvalue> = args.iter().map(super::super::jvalue::JValue::as_jni).collect();

		// SAFETY: The receiver is an instance of class, and the method ID is resolved from that same class.
		// We've also validated the argument counts and types using the same type signature
		// we fetched the original method ID from.
		unsafe { self.call_nonvirtual_method_unchecked(obj, class, method_id, sig.ret(), &args) }
	}

	/// Create a new object using a constructor. This is done safely using
	/// checks similar to those in `call_static_method`.
	///
	/// # Errors
	///
	/// Returns [`Error::WrongObjectType`] for incompatible reference arguments.
	/// Returns an error if class or method lookup fails, arguments do not match the signature, or Java throws an exception.
	pub fn new_object<'other_local, 'sig, 'sig_args, C, S>(
		&mut self,
		class: C,
		ctor_sig: S,
		ctor_args: &[JValue],
	) -> Result<JObject<'local>>
	where
		C: Desc<'local, JClass<'other_local>>,
		S: AsRef<MethodSignature<'sig, 'sig_args>>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let ctor_sig: &MethodSignature<'_, '_> = ctor_sig.as_ref();

		validate_call_args(ctor_sig, ctor_args)?;

		// check return value
		if ctor_sig.ret() != ReturnType::Primitive(Primitive::Void) {
			return Err(Error::InvalidCtorReturn);
		}

		// build strings
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;
		let class: &JClass<'_> = class.as_ref();

		let method_id: JMethodID = Desc::<JMethodID>::lookup((class, ctor_sig), self)?;

		// SAFETY: The constructor ID was resolved from this class and signature.
		unsafe {
			self.__validate_method_reference_args(class, method_id.into_raw(), false, &reference_args(ctor_args))?;
		}

		let ctor_args: Vec<jvalue> = ctor_args.iter().map(super::super::jvalue::JValue::as_jni).collect();
		// SAFETY: We've obtained the method_id above, so it is valid for this class.
		// We've also validated the argument counts and types using the same type signature
		// we fetched the original method ID from.
		unsafe { self.new_object_unchecked(class, method_id, &ctor_args) }
	}

	/// Create a new object using a constructor. Arguments aren't checked
	/// because of the `JMethodID` usage.
	///
	/// # Safety
	///
	/// The provided `JMethodID` must be valid, and match the types and number of arguments, as well as return type
	/// (always an Object for a constructor). If these are incorrect, the JVM may crash.  The `JMethodID` must also match
	/// the passed type.
	///
	/// # Errors
	///
	/// Returns an error if class or constructor resolution fails, allocation fails, or the constructor throws a Java exception.
	pub unsafe fn new_object_unchecked<'other_local, C, M>(
		&mut self,
		class: C,
		ctor_id: M,
		ctor_args: &[jvalue],
	) -> Result<JObject<'local>>
	where
		C: Desc<'local, JClass<'other_local>>,
		M: Desc<'local, JMethodID>,
	{
		// Runtime check that the 'local reference lifetime will be tied to
		// Env lifetime for the top JNI stack frame
		self.assert_top();
		let class: <C as Desc<'_, JClass<'_>>>::Output = class.lookup(self)?;

		let ctor_id: JMethodID = *ctor_id.lookup(self)?.as_ref();

		let jni_args: *const abos_jni_sys::jvalue = ctor_args.as_ptr();

		// SAFETY: The caller guarantees that ctor_id and ctor_args match this class's constructor signature.
		let obj: JObject<'_> = unsafe {
			jni_call_post_check_ex_and_null_ret!(
				self,
				v1_1,
				NewObjectA,
				class.as_ref().as_raw(),
				ctor_id.into_raw(),
				jni_args
			)
			.map(|obj| JObject::from_raw(self, obj))
		}?;

		// Ensure that `class` isn't dropped before the JNI call returns.
		drop(class);

		Ok(obj)
	}
}

#[cfg(test)]
#[path = "../../../tests/unit/api/env/calls.rs"]
mod tests;
