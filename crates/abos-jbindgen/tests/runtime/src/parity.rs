//! Identical assertions over independent source and bytecode wrapper types.
use abos_jni::{Env, errors::Result};

macro_rules! checks {
	($module:ident, $bindings:path, $echo:ident) => {
		mod $module {
			use abos_jni::{
				Env,
				errors::Result,
				jni_sig, jni_str,
				objects::{JIntArray, JObjectArray, JString},
			};
			use bindings::com::example::{
				ParityInheritanceBase, ParityInheritanceChild, ParityInheritanceNamed, ParityMembers, ParityVisibility,
			};
			use $bindings as bindings;

			pub fn members(env: &mut Env<'_>) -> Result<()> {
				let text = env.new_string("four")?;
				let public = ParityVisibility::new(env)?;
				assert_eq!(public.public_method(env)?, 11);
				assert_eq!(public.protected_method(env)?, 22);
				assert_eq!(public.package_method(env)?, 33);
				public.set_protected_field(env, 51)?;
				public.set_package_field(env, 52)?;
				assert_eq!(public.protected_field(env)?, 51);
				assert_eq!(public.package_method(env)?, 52);
				let protected = ParityVisibility::new_int(env, 61)?;
				let package = ParityVisibility::new_string(env, &text)?;
				assert_eq!(protected.public_field(env)?, 61);
				assert_eq!(package.public_field(env)?, 4);

				let value = ParityMembers::new(env, 7)?;
				assert_eq!(value.fixed(env)?, 8);
				assert_eq!(ParityMembers::CONSTANT(env)?, 19);
				value.set_value(env, 15)?;
				assert_eq!(value.value(env)?, 15);
				ParityMembers::set_shared(env, 6)?;
				assert_eq!(ParityMembers::shared(env)?, 6);
				assert_eq!(value.choose_int(env, 3)?, 13);
				assert_eq!(value.choose_string(env, &text)?, 24);
				let ints = env.new_int_array(2)?;
				assert_eq!(ParityMembers::choose_int_1d(env, &ints)?, 32);
				let strings = JObjectArray::<JString>::new(env, 3, &text)?;
				assert_eq!(ParityMembers::choose_string_1d(env, &strings)?, 43);
				let matrix = JObjectArray::<JIntArray>::new(env, 1, &ints)?;
				let returned = value.matrix(env, &matrix)?;
				assert!(env.is_same_object(&matrix, &returned)?);
				let overload = bindings::com::example::Overloads::new(env)?;
				assert_eq!(overload.calculate_int_int(env, 2, 3)?, 5);
				assert_eq!(overload.calculate_int_string(env, 7, &text)?, 7);
				assert_eq!(overload.calculate_string_int(env, &text, 9)?, 9);
				assert_eq!(overload.calculate_string_string(env, &text, &text)?, 0);
				Ok(())
			}

			pub fn inheritance(env: &mut Env<'_>) -> Result<()> {
				let child = ParityInheritanceChild::new(env)?;
				let text = env.new_string("test")?;
				assert_eq!(child.$echo(env, &text)?.to_string(), "child:test");
				let number = child.number(env)?;
				assert_eq!(
					env.call_method(&number, jni_str!("intValue"), jni_sig!("()I"), &[])?
						.i()?,
					42
				);
				let base: &ParityInheritanceBase<'_> = child.as_ref();
				assert_eq!(base.inherited(env)?, 17);
				let echoed = base.echo(env, &text)?;
				let echoed = env.cast_local::<JString>(echoed)?;
				assert_eq!(echoed.to_string(), "child:test");
				let number = base.number(env)?;
				assert_eq!(
					env.call_method(&number, jni_str!("intValue"), jni_sig!("()I"), &[])?
						.i()?,
					42
				);
				let named: &ParityInheritanceNamed<'_> = child.as_ref();
				assert_eq!(named.default_value(env)?, 31);
				Ok(())
			}

			pub fn identifiers(env: &mut Env<'_>) -> Result<()> {
				let single = bindings::I::new(env)?;
				single.set_peer(env, &single)?;
				let read = single.peer(env)?;
				assert!(env.is_same_object(&single, &read)?);
				let values = JObjectArray::<bindings::I>::new(env, 1, &single)?;
				let returned = single.echo(env, &values)?.get_element(env, 0)?;
				assert!(env.is_same_object(&single, &returned)?);
				let owner = bindings::DefaultType::new(env)?;
				let peer = bindings::DefaultPeer::new(env, &owner)?;
				owner.set_peer(env, &peer)?;
				let read = owner.peer(env)?;
				assert!(env.is_same_object(&peer, &read)?);
				let child = bindings::DefaultTypeChild::new(env)?;
				let returned = owner.echo_child(env, &child)?;
				assert!(env.is_same_object(&child, &returned)?);
				let identifiers = bindings::com::type_::Identifiers::new(env)?;
				identifiers.set_caf_ue9_(env, 23)?;
				assert_eq!(identifiers.caf_ue9_(env)?, 23);
				assert_eq!(identifiers._u3bb_(env)?, 10);
				assert_eq!(identifiers.match_(env, 1, 2, 3, 4, 5)?, 54321);
				let dollars = bindings::com::example::UnsupportedNames::new(env)?;
				dollars.set__value(env, 29)?;
				assert_eq!(dollars.foo_bar(env)?, 29);
				Ok(())
			}
		}
	};
}

// Bridge overloads change Rust names but not the declared Java signature.
checks!(source, crate::bindings, echo);
checks!(bytecode, crate::parity_bytecode, echo_string);

pub fn members(env: &mut Env<'_>) -> Result<()> {
	source::members(env)?;
	bytecode::members(env)
}

pub fn inheritance(env: &mut Env<'_>) -> Result<()> {
	source::inheritance(env)?;
	bytecode::inheritance(env)?;
	let child = crate::parity_bytecode::com::example::ParityInheritanceChild::new(env)?;
	let text = env.new_string("bridge")?;
	let returned = child.echo_object(env, &text)?;
	let returned = env.cast_local::<abos_jni::objects::JString>(returned)?;
	assert_eq!(returned.to_string(), "child:bridge");
	let number = child.number_1(env)?;
	assert_eq!(
		env.call_method(&number, abos_jni::jni_str!("intValue"), abos_jni::jni_sig!("()I"), &[])?
			.i()?,
		42
	);
	Ok(())
}

pub fn identifiers(env: &mut Env<'_>) -> Result<()> {
	source::identifiers(env)?;
	bytecode::identifiers(env)
}
