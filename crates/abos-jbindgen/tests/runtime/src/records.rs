//! Exercise record and annotation binary names through both generated binding sets.
use abos_jni::{Env, errors::Result};

macro_rules! check_bindings {
	($module:ident, $bindings:path) => {
		mod $module {
			use abos_jni::{Env, errors::Result, objects::JObjectArray};
			use $bindings as types;

			pub fn check(env: &mut Env<'_>) -> Result<()> {
				let record = types::RecordTypes::new(env, 42)?;
				assert_eq!(record.value(env)?, 42);
				let member = types::RecordTypesMember::new(env)?;
				assert_eq!(member.echo(env, &record)?.value(env)?, 42);
				let values = JObjectArray::<types::RecordTypes>::new(env, 1, &record)?;
				let nested = types::RecordTypesNested::new(env, &values)?;
				assert_eq!(nested.values(env)?.get_element(env, 0)?.value(env)?, 42);
				let marker = types::RecordTypes::marker(env)?;
				assert_eq!(marker.value(env)?, 7);
				let markers = JObjectArray::<types::Marker>::new(env, 1, &marker)?;
				let member = types::MarkerMember::new(env, &markers)?;
				assert_eq!(member.values(env)?.get_element(env, 0)?.value(env)?, 7);
				Ok(())
			}
		}
	};
}

check_bindings!(source, crate::bindings::com::example);
check_bindings!(bytecode, crate::record_bytecode::com::example);

pub fn check(env: &mut Env<'_>) -> Result<()> {
	crate::record_bytecode::jni_init(env, &abos_jni::refs::LoaderContext::default())?;
	source::check(env)?;
	bytecode::check(env)
}
