#![allow(unsafe_code, reason = "Binding macros generate JNI calls.")]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{
	Env, bind_java_type,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JClassLoader, JIntArray, JObject, JObjectArray, JString},
	refs::Reference,
	strings::JNIString,
};
use assert_matches::assert_matches;
use rusty_fork::rusty_fork_test;

bind_java_type! { LetterZ => ".Z" }
bind_java_type! { LetterB => ".B" }
bind_java_type! { LetterC => ".C" }
bind_java_type! { LetterS => ".S" }
bind_java_type! { LetterI => ".I" }
bind_java_type! { LetterJ => ".J" }
bind_java_type! { LetterF => ".F" }
bind_java_type! { LetterD => ".D" }
bind_java_type! { LetterV => ".V" }
bind_java_type! { LetterA => ".A" }

#[test]
fn object_array_class_names_distinguish_objects_from_descriptors() {
	check_name::<LetterZ>("Z");
	check_name::<LetterB>("B");
	check_name::<LetterC>("C");
	check_name::<LetterS>("S");
	check_name::<LetterI>("I");
	check_name::<LetterJ>("J");
	check_name::<LetterF>("F");
	check_name::<LetterD>("D");
	check_name::<LetterV>("V");
	check_name::<LetterA>("A");
	assert_eq!(JObjectArray::<JIntArray>::class_name().to_str(), "[[I");
	assert_eq!(JObjectArray::<JString>::class_name().to_str(), "[Ljava.lang.String;");
}

fn check_name<E: Reference>(name: &str) {
	assert_eq!(JObjectArray::<E>::class_name().to_str(), format!("[L{name};"));
	assert_eq!(
		JObjectArray::<JObjectArray<E>>::class_name().to_str(),
		format!("[[L{name};")
	);
}

fn check_primitive_cast<E: Reference>(env: &mut Env<'_>, primitive: JObject<'_>) -> Result<()> {
	let class = env.get_object_class(&primitive)?;
	let matrix = env.new_object_array(1, &class, &primitive)?;
	assert_matches!(
		env.cast_local::<JObjectArray<JObjectArray<E>>>(matrix).err(),
		Some(Error::WrongObjectType)
	);
	assert_matches!(
		env.cast_local::<JObjectArray<E>>(primitive).err(),
		Some(Error::WrongObjectType)
	);
	assert!(!env.exception_check());
	Ok(())
}

fn check_valid<E: Reference>(env: &mut Env<'_>, name: &str) -> Result<()> {
	let class_name = JNIString::new(name);
	let object = env.new_object(&class_name, jni_sig!("()V"), &[])?;
	let array = env.new_object_array(1, &class_name, &object)?;
	let array = env.cast_local::<JObjectArray<E>>(array)?;
	let element = array.get_element(env, 0)?;
	assert!(env.is_same_object(&element, &object)?);
	let text = env
		.call_method(&element, jni_str!("toString"), jni_sig!("()Ljava/lang/String;"), &[])?
		.l()?;
	let text = env.cast_local::<JString>(text)?;
	assert!(text.try_to_string(env)?.starts_with(&format!("{name}@")));

	let array_name = JNIString::new(format!("[L{name};"));
	let matrix = env.new_object_array(1, &array_name, &array)?;
	let matrix = env.cast_local::<JObjectArray<JObjectArray<E>>>(matrix)?;
	let element = matrix.get_element(env, 0)?.get_element(env, 0)?;
	assert!(env.is_same_object(element, &object)?);
	Ok(())
}

rusty_fork_test! {
#[test]
fn single_letter_object_array_casts_reject_primitive_arrays() {
	let output = util::setup_test_output("single_letter_array_casts");
	let classes = abos_javac::Build::new()
		.file("tests/java/SingleLetterTypes.java")
		.output_dir(&output)
		.compile();
	util::attach_current_thread(|env| {
		let loader = JClassLoader::get_system_class_loader(env)?;
		for path in classes {
			let bytes = std::fs::read(path).expect("read compiled single-letter class");
			env.define_class(None::<&abos_jni::strings::JNIStr>, &loader, &bytes)?;
		}
		macro_rules! check_cast {
			($type:ty, $primitive:ident) => {{
				let primitive = env.$primitive(1)?;
				check_primitive_cast::<$type>(env, primitive.into())?;
			}};
		}
		check_cast!(LetterI, new_int_array);
		check_cast!(LetterZ, new_boolean_array);
		check_cast!(LetterB, new_byte_array);
		check_cast!(LetterC, new_char_array);
		check_cast!(LetterS, new_short_array);
		check_cast!(LetterJ, new_long_array);
		check_cast!(LetterF, new_float_array);
		check_cast!(LetterD, new_double_array);

		check_valid::<LetterZ>(env, "Z")?;
		check_valid::<LetterB>(env, "B")?;
		check_valid::<LetterC>(env, "C")?;
		check_valid::<LetterS>(env, "S")?;
		check_valid::<LetterI>(env, "I")?;
		check_valid::<LetterJ>(env, "J")?;
		check_valid::<LetterF>(env, "F")?;
		check_valid::<LetterD>(env, "D")?;
		check_valid::<LetterV>(env, "V")?;
		check_valid::<LetterA>(env, "A")?;

		let ints = env.new_int_array(1)?;
		ints.set_region(env, 0, &[42])?;
		let matrix = env.new_object_array(1, jni_str!("[I"), &ints)?;
		let matrix = env.cast_local::<JObjectArray<JIntArray>>(matrix)?;
		let mut values = [0];
		matrix.get_element(env, 0)?.get_region(env, 0, &mut values)?;
		assert_eq!(values, [42]);
		Ok(())
	})
	.expect("single-letter classes preserve object array type safety");
}
}
