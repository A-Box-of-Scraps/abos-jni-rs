//! Enum constants must be usable static final fields through both input paths.
use abos_jni::{Env, errors::Result};

macro_rules! check_bindings {
	($module:ident, $bindings:path) => {
		mod $module {
			use abos_jni::{Env, errors::Result};
			use $bindings as types;

			pub fn check(env: &mut Env<'_>) -> Result<()> {
				let red = types::EnumTypes::RED(env)?;
				let blue = types::EnumTypes::BLUE(env)?;
				assert_eq!(red.id(env)?, 11);
				assert_eq!(red.code(env)?, 11);
				assert_eq!(blue.id(env)?, 22);
				assert_eq!(blue.code(env)?, 42);
				let values = types::EnumTypes::values(env)?;
				assert_eq!(values.len(env)?, 2);
				let element = values.get_element(env, 0)?;
				assert!(env.is_same_object(&red, &element)?);
				let element = values.get_element(env, 1)?;
				assert!(env.is_same_object(&blue, &element)?);
				let name = env.new_string("BLUE")?;
				let found = types::EnumTypes::value_of(env, name)?;
				assert!(env.is_same_object(&blue, &found)?);
				let on = types::EnumTypesState::ON(env)?;
				let off = types::EnumTypesState::OFF(env)?;
				assert!(!env.is_same_object(&on, &off)?);
				let states = types::EnumTypesState::values(env)?;
				assert_eq!(states.len(env)?, 2);
				let element = states.get_element(env, 0)?;
				assert!(env.is_same_object(&on, &element)?);
				let element = states.get_element(env, 1)?;
				assert!(env.is_same_object(&off, &element)?);
				assert_eq!(types::EnumTypesEmpty::values(env)?.len(env)?, 0);
				Ok(())
			}
		}
	};
}

check_bindings!(source, crate::bindings::com::example);
check_bindings!(bytecode, crate::parity_bytecode::com::example);

pub fn check(env: &mut Env<'_>) -> Result<()> {
	source::check(env)?;
	bytecode::check(env)
}
