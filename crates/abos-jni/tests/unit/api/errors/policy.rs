use crate::{
	EnvUnowned,
	objects::{JClass, JObject, JString},
};

use super::*;
struct TestCustomPolicyCaptures<'local, 'native_method>
where
	'local: 'native_method,
{
	context: &'native_method JObject<'local>, // capture a local reference
}

struct TestCustomPolicy;

impl<T: Default, E: std::error::Error> ErrorPolicy<T, E> for TestCustomPolicy {
	type Captures<'unowned_env_local: 'native_method, 'native_method> =
		TestCustomPolicyCaptures<'unowned_env_local, 'native_method>;

	fn on_error<'unowned_env_local: 'native_method, 'native_method>(
		_env: &mut Env<'unowned_env_local>,
		cap: &mut TestCustomPolicyCaptures<'unowned_env_local, 'native_method>,
		err: E,
	) -> crate::errors::Result<T> {
		// Handle the error, possibly throwing a Java exception
		eprintln!("Error: {:?}, context: {:?}", err, cap.context);
		// You can access cap.context here
		Ok(T::default())
	}

	fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
		_env: &mut Env<'unowned_env_local>,
		_cap: &mut TestCustomPolicyCaptures<'unowned_env_local, 'native_method>,
		payload: Box<dyn std::any::Any + Send + 'static>,
	) -> crate::errors::Result<T> {
		// Handle the panic, possibly throwing a Java exception
		eprintln!("Panic: {payload:?}");
		// You can access cap.context here
		Ok(T::default())
	}
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_HelloWorld_test<'local>(
	mut unowned_env: EnvUnowned<'local>,
	_this: JObject<'local>,
	context: JObject<'local>,
) -> JClass<'local> {
	unowned_env
		.with_env(|env| -> crate::errors::Result<_> {
			// do stuff that might fail or panic
			// for the sake of testing, use the context that will also be captured
			let class: JClass<'_> = env.get_object_class(&context)?;
			Ok(class)
		})
		.resolve_with::<TestCustomPolicy, _>(|| {
			// capture state from the native method scope or from the JNI local reference frame
			TestCustomPolicyCaptures::<'local, '_> {
				context: &context, // capture a local reference
			}
		})
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_HelloWorld_logErrorWithContextString<'local>(
	mut unowned_env: EnvUnowned<'local>,
	_this: JObject<'local>,
	context: JObject<'local>,
	arg: JString<'local>,
) -> JClass<'local> {
	unowned_env
		.with_env(|env| -> crate::errors::Result<_> {
			// do stuff that might fail or panic
			// for the sake of testing, use the context that will also be captured
			let class: JClass<'_> = env.get_object_class(&context)?;
			Ok(class)
		})
		.resolve_with::<LogContextErrorAndDefault, _>(|| format!("In 'logErrorWithContextString' with arg: {arg}"))
}
