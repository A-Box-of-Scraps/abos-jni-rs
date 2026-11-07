use super::{ReferenceTypes, util};
use abos_jni::{
	Env, JValue,
	errors::Result,
	jni_sig, jni_str,
	objects::{JObject, JObjectArray, JString},
	refs::Reference,
};
use rusty_fork::rusty_fork_test;

fn check_reference_roundtrip(
	env: &mut Env<'_>,
	receiver: &ReferenceTypes<'_>,
	generated: bool,
	value: &JString<'_>,
) -> Result<()> {
	let returned: JString<'_> = if generated {
		receiver.echo_string(env, value)?
	} else {
		let object: JObject<'_> = env
			.call_method(
				receiver,
				jni_str!("echoString"),
				jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
				&[JValue::Object(value)],
			)?
			.l()?;
		env.cast_local::<JString>(object)?
	};
	assert!(env.is_same_object(value, &returned)?);
	if generated {
		receiver.set_text(env, &returned)?;
	} else {
		env.set_field(
			receiver,
			jni_str!("text"),
			jni_sig!("Ljava/lang/String;"),
			JValue::Object(&returned),
		)?;
	}
	let stored: JString<'_> = receiver.text(env)?;
	assert!(env.is_same_object(value, &stored)?);
	let array: JObjectArray<'_, JString<'_>> = JObjectArray::<JString>::new(env, 1, &stored)?;
	let returned: JObjectArray<'_, JString<'_>> = if generated {
		receiver.echo_strings(env, &array)?
	} else {
		let object: JObject<'_> = env
			.call_method(
				receiver,
				jni_str!("echoStrings"),
				jni_sig!("([Ljava/lang/String;)[Ljava/lang/String;"),
				&[JValue::Object(&array)],
			)?
			.l()?;
		env.cast_local::<JObjectArray<JString>>(object)?
	};
	assert!(env.is_same_object(&array, &returned)?);
	if generated {
		receiver.set_strings(env, &returned)?;
	} else {
		env.set_field(
			receiver,
			jni_str!("strings"),
			jni_sig!("[Ljava/lang/String;"),
			JValue::Object(&returned),
		)?;
	}
	let stored: JObjectArray<'_, JString<'_>> = receiver.strings(env)?;
	assert!(env.is_same_object(&array, &stored)?);
	let element: JString<'_> = stored.get_element(env, 0)?;
	assert!(env.is_same_object(value, &element)?);
	if value.is_null() {
		assert!(element.is_null());
	} else {
		assert_eq!(element.try_to_string(env)?, "roundtrip");
		assert_eq!(
			env.call_method(&element, jni_str!("length"), jni_sig!("()I"), &[])?
				.i()?,
			9
		);
	}
	Ok(())
}

rusty_fork_test! {
#[test]
fn method_and_field_roundtrips_preserve_typed_references() {
	let output: std::path::PathBuf = util::java_fixtures();
	util::attach_current_thread(|env| {
		util::load_test_class(env, &output, "ReferenceTypes")?;
		let receiver = ReferenceTypes::new(env)?;
		let string = env.new_string("roundtrip")?;
		for generated in [false, true] {
			for value in [&string, &JString::null()] {
				check_reference_roundtrip(env, &receiver, generated, value)?;
			}
		}
		assert!(receiver.echo_strings(env, JObjectArray::<JString>::null())?.is_null());
		assert!(!env.exception_check());
		Ok(())
	})
	.expect("method results and field values remain usable through typed array reads");
}
}
