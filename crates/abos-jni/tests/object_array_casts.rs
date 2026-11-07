#![allow(unsafe_code, reason = "Binding macros generate JNI calls.")]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{
	Env, bind_java_type,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JClass, JClassLoader, JIntArray, JObject, JObjectArray, JString},
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
	let class: JClass<'_> = env.get_object_class(&primitive)?;
	let matrix: JObjectArray<'_> = env.new_object_array(1, &class, &primitive)?;
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
	let object: JObject<'_> = env.new_object(&class_name, jni_sig!("()V"), &[])?;
	let array: JObjectArray<'_> = env.new_object_array(1, &class_name, &object)?;
	let array: JObjectArray<'_, E::Kind<'_>> = env.cast_local::<JObjectArray<E>>(array)?;
	let element: JObject<'_> = array.get_element(env, 0)?.into();
	assert!(env.is_same_object(&element, &object)?);
	let text: JObject<'_> = env
		.call_method(&element, jni_str!("toString"), jni_sig!("()Ljava/lang/String;"), &[])?
		.l()?;
	let text: JString<'_> = env.cast_local::<JString>(text)?;
	assert!(text.try_to_string(env)?.starts_with(&format!("{name}@")));

	let array_name: JNIString = JNIString::new(format!("[L{name};"));
	let matrix: JObjectArray<'_> = env.new_object_array(1, &array_name, &array)?;
	let matrix: JObjectArray<'_, JObjectArray<'_, E::Kind<'_>>> =
		env.cast_local::<JObjectArray<JObjectArray<E>>>(matrix)?;
	let element: JObject<'_> = matrix.get_element(env, 0)?.get_element(env, 0)?.into();
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

#[test]
fn widened_array_rejects_incompatible_writes_and_preserves_elements() {
	util::attach_current_thread(|env| {
		let original = env.new_string("original")?;
		let strings = JObjectArray::<JString>::new(env, 1, &original)?;
		let objects = env.new_cast_local_ref::<JObjectArray<JObject>>(&strings)?;
		let invalid = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		assert_matches!(objects.set_element(env, 0, &invalid), Err(Error::WrongObjectType));
		assert!(!env.exception_check());
		let strings = env.cast_local::<JObjectArray<JString>>(objects)?;
		let element = strings.get_element(env, 0)?;
		assert!(env.is_same_object(&original, &element)?);
		assert_eq!(element.try_to_string(env)?, "original");

		// Array casts check the runtime component type, even when every element is a String.
		let objects = env.new_object_array(1, jni_str!("java/lang/Object"), &original)?;
		assert_matches!(env.new_cast_local_ref::<JObjectArray<JString>>(&objects), Err(Error::WrongObjectType));
		assert!(!env.exception_check());
		let element = objects.get_element(env, 0)?;
		let element = env.cast_local::<JString>(element)?;
		assert_eq!(element.try_to_string(env)?, "original");
		Ok(())
	})
	.expect("widening an array does not weaken its runtime element checks");
}

#[test]
fn nested_array_rejects_incompatible_rows_and_preserves_typed_reads() {
	util::attach_current_thread(|env| {
		let original = env.new_string("nested")?;
		let row = JObjectArray::<JString>::new(env, 1, &original)?;
		let matrix = JObjectArray::<JObjectArray<JString>>::new(env, 1, &row)?;
		let widened = env.new_cast_local_ref::<JObjectArray<JObject>>(&matrix)?;
		let objects = env.new_object_array(1, jni_str!("java/lang/Object"), &original)?;
		let ints = env.new_int_array(1)?;
		let invalid_rows: [&JObject; 3] = [objects.as_ref(), ints.as_ref(), original.as_ref()];
		for invalid in invalid_rows {
			assert_matches!(widened.set_element(env, 0, invalid), Err(Error::WrongObjectType));
			assert!(!env.exception_check());
			let typed = env.new_cast_local_ref::<JObjectArray<JObjectArray<JString>>>(&widened)?;
			let stored = typed.get_element(env, 0)?;
			assert!(env.is_same_object(&row, &stored)?);
			assert_eq!(stored.get_element(env, 0)?.try_to_string(env)?, "nested");
		}
		assert_matches!(env.new_cast_local_ref::<JObjectArray<JIntArray>>(&matrix), Err(Error::WrongObjectType));
		assert!(!env.exception_check());
		widened.set_element(env, 0, JObject::null())?;
		assert!(matrix.get_element(env, 0)?.is_null());
		widened.set_element(env, 0, &row)?;
		assert_eq!(matrix.get_element(env, 0)?.get_element(env, 0)?.try_to_string(env)?, "nested");
		Ok(())
	})
	.expect("nested array writes preserve the runtime row type");
}

#[test]
fn array_writes_accept_subclasses_interfaces_and_nulls() {
	util::attach_current_thread(|env| {
		let string = env.new_string("assignable")?;
		let object = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		for class in [jni_str!("java/lang/Object"), jni_str!("java/lang/Comparable")] {
			let array = env.new_object_array(1, class, JObject::null())?;
			assert!(array.get_element(env, 0)?.is_null());
			array.set_element(env, 0, &string)?;
			if class == jni_str!("java/lang/Comparable") {
				assert_matches!(array.set_element(env, 0, &object), Err(Error::WrongObjectType));
				assert!(!env.exception_check());
			}
			let element = array.get_element(env, 0)?;
			assert_eq!(env.cast_local::<JString>(element)?.try_to_string(env)?, "assignable");
			array.set_element(env, 0, JObject::null())?;
			assert!(array.get_element(env, 0)?.is_null());
		}
		Ok(())
	})
	.expect("valid reference assignments survive array writes and reads");
}
}
