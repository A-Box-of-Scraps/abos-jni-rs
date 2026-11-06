use super::{
	CLASS_LOCAL_DATE_TIME, CLASS_MATH, METHOD_LOCAL_DATE_TIME_OF, METHOD_MATH_ABS, METHOD_OBJECT_HASH_CODE,
	SIG_LOCAL_DATE_TIME_OF, SIG_MATH_ABS, SIG_OBJECT_HASH_CODE,
};
use abos_jni::{
	Env,
	descriptors::Desc,
	objects::{JClass, JMethodID, JObject, JStaticMethodID, JValue},
	signature::{Primitive, ReturnType},
	sys::jint,
};
use abos_jni_sys::jvalue;

#[inline(never)]
pub(super) const fn native_abs(x: i32) -> i32 {
	x.abs()
}

pub(super) fn jni_abs_safe(env: &mut Env, x: jint) -> jint {
	let x = JValue::from(x);
	let v = env
		.call_static_method(CLASS_MATH, METHOD_MATH_ABS, &SIG_MATH_ABS, &[x])
		.expect("static method call succeeds");
	v.i().expect("JNI result contains an int")
}

pub(super) fn jni_hash_safe(env: &mut Env, obj: &JObject) -> jint {
	let v = env
		.call_method(obj, METHOD_OBJECT_HASH_CODE, &SIG_OBJECT_HASH_CODE, &[])
		.expect("instance method call succeeds");
	v.i().expect("JNI result contains an int")
}

pub(super) fn jni_local_date_time_of_safe<'local>(env: &mut Env<'local>, components: [jint; 7]) -> JObject<'local> {
	let args = components.map(JValue::Int);
	env.call_static_method(
		CLASS_LOCAL_DATE_TIME,
		METHOD_LOCAL_DATE_TIME_OF,
		&SIG_LOCAL_DATE_TIME_OF,
		&args,
	)
	.expect("LocalDateTime.of() succeeds")
	.l()
	.expect("JNI result contains an object")
}

pub(super) fn jni_int_call_static_unchecked<'local, C>(
	env: &mut Env<'local>,
	class: C,
	method_id: JStaticMethodID,
	x: jint,
) -> jint
where
	C: Desc<'local, JClass<'local>>,
{
	let x = JValue::from(x);
	let ret = ReturnType::Primitive(Primitive::Int);
	// SAFETY: The benchmarks resolve this ID from Math.abs(I)I and pass an int argument.
	let v = unsafe { env.call_static_method_unchecked(class, method_id, ret, &[x.as_jni()]) }
		.expect("call static method unchecked succeeds");
	v.i().expect("JNI result contains an int")
}

pub(super) fn jni_int_call_unchecked<'local, M>(env: &mut Env<'local>, obj: &JObject<'local>, method_id: M) -> jint
where
	M: Desc<'local, JMethodID>,
{
	let ret = ReturnType::Primitive(Primitive::Int);
	// SAFETY: Caller retrieved method ID + class specifically for this use: Object.hashCode()I
	let v = unsafe { env.call_method_unchecked(obj, method_id, ret, &[]) }.expect("call method unchecked succeeds");
	v.i().expect("JNI result contains an int")
}

pub(super) fn jni_object_call_static_unchecked<'local, C>(
	env: &mut Env<'local>,
	class: C,
	method_id: JStaticMethodID,
	args: &[jvalue],
) -> JObject<'local>
where
	C: Desc<'local, JClass<'local>>,
{
	// SAFETY: Caller retrieved method ID and constructed arguments
	let v = unsafe { env.call_static_method_unchecked(class, method_id, ReturnType::Object, args) }
		.expect("call static method unchecked succeeds");
	v.l().expect("JNI result contains an object")
}
