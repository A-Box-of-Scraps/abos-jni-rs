//! Exercise both source and bytecode bindings against the same erased generic API.
use abos_jni::{Env, errors::Result};

macro_rules! check_bindings {
	($module:ident, $binding:path) => {
		mod $module {
			use abos_jni::{Env, errors::Result, jni_sig, jni_str};
			use $binding as GenericTypes;

			pub fn check(env: &mut Env<'_>) -> Result<()> {
				let number = env.new_object(jni_str!("java/lang/Integer"), jni_sig!("(I)V"), &[42.into()])?;
				let string = env.new_string("generic")?;
				let instance = GenericTypes::new(env, &number, &string)?;
				let echoed = instance.echo(env, &number)?;
				assert!(env.is_same_object(&echoed, &number)?);
				let echoed = instance.unbounded(env, &string)?;
				assert!(env.is_same_object(&echoed, &string)?);
				let echoed = instance.echo_dependent(env, &number)?;
				assert!(env.is_same_object(&echoed, &number)?);
				let echoed = instance.related(env, &number)?;
				assert!(env.is_same_object(&echoed, &number)?);
				let echoed = GenericTypes::identity(env, &string)?;
				assert!(env.is_same_object(&echoed, &string)?);
				let echoed = GenericTypes::bounded(env, &string)?;
				assert!(env.is_same_object(&echoed, &string)?);
				let echoed = instance.recursive(env, &string)?;
				assert!(env.is_same_object(&echoed, &string)?);
				check_fields(env, &instance, &number, &string)?;
				check_arrays(env, &instance, &number, &string)?;
				Ok(())
			}

			fn check_fields(
				env: &mut Env<'_>,
				instance: &GenericTypes<'_>,
				number: &abos_jni::objects::JObject<'_>,
				string: &abos_jni::objects::JString<'_>,
			) -> Result<()> {
				instance.set_number(env, number)?;
				let read = instance.number(env)?;
				assert!(env.is_same_object(&read, number)?);
				instance.set_value(env, string)?;
				let read = instance.value(env)?;
				assert!(env.is_same_object(&read, string)?);
				instance.set_dependent(env, number)?;
				let read = instance.dependent(env)?;
				assert!(env.is_same_object(&read, number)?);
				Ok(())
			}

			fn check_arrays(
				env: &mut Env<'_>,
				instance: &GenericTypes<'_>,
				number: &abos_jni::objects::JObject<'_>,
				string: &abos_jni::objects::JString<'_>,
			) -> Result<()> {
				let numbers = env.new_object_array(1, jni_str!("java/lang/Number"), number)?;
				let echoed = instance.echo_array(env, &numbers)?;
				assert!(env.is_same_object(&echoed, &numbers)?);
				instance.set_numbers(env, &numbers)?;
				let read = instance.numbers(env)?.get_element(env, 0)?;
				assert!(env.is_same_object(&read, number)?);
				let matrix = env.new_object_array(1, jni_str!("[Ljava/lang/Number;"), &numbers)?;
				let matrix =
					env.cast_local::<abos_jni::objects::JObjectArray<abos_jni::objects::JObjectArray>>(matrix)?;
				let echoed = instance.method_array(env, &matrix)?;
				assert!(env.is_same_object(&echoed, &matrix)?);
				let objects = env.new_object_array(1, jni_str!("java/lang/Object"), string)?;
				let matrix = env.new_object_array(1, jni_str!("[Ljava/lang/Object;"), &objects)?;
				let matrix =
					env.cast_local::<abos_jni::objects::JObjectArray<abos_jni::objects::JObjectArray>>(matrix)?;
				let echoed = instance.matrix(env, &matrix)?;
				assert!(env.is_same_object(&echoed, &matrix)?);
				instance.set_values(env, &matrix)?;
				let read = instance.values(env)?.get_element(env, 0)?.get_element(env, 0)?;
				assert!(env.is_same_object(&read, string)?);
				Ok(())
			}
		}
	};
}

check_bindings!(source, crate::bindings::com::example::GenericTypes);
check_bindings!(bytecode, crate::parity_bytecode::com::example::GenericTypes);

pub fn check(env: &mut Env<'_>) -> Result<()> {
	source::check(env)?;
	bytecode::check(env)
}
