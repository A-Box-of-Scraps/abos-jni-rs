//! Source and bytecode constructors must receive the immediate enclosing instance.
use abos_jni::{Env, errors::Result};

macro_rules! check_bindings {
	($module:ident, $bindings:path) => {
		mod $module {
			use abos_jni::{Env, errors::Result};
			use $bindings as types;

			pub fn check(env: &mut Env<'_>) -> Result<()> {
				let outer = types::EnclosingTypes::new(env, 30)?;
				let inner = types::EnclosingTypesInner::new(env, &outer, 10)?;
				assert_eq!(inner.value(env)?, 40);
				let deep = types::EnclosingTypesInnerDeep::new(env, &inner, 2)?;
				assert_eq!(deep.value(env)?, 42);
				let implicit = types::EnclosingTypesImplicit::new(env, &outer)?;
				assert_eq!(implicit.value(env)?, 30);
				let derived = types::EnclosingTypesDerived::new(env, &outer, 12)?;
				let inner: &types::EnclosingTypesInner<'_> = derived.as_ref();
				assert_eq!(inner.value(env)?, 42);
				let nested = types::EnclosingTypesStaticNested::new(env, 17)?;
				assert_eq!(nested.value(env)?, 17);
				let member = types::EnclosingTypesContainerMember::new(env, 19)?;
				assert_eq!(member.value(env)?, 19);
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
