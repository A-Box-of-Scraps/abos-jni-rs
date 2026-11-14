use abos_jni::{
	Env, jni_sig, jni_str,
	objects::{JCharArray, JObject, JString, JValue},
	strings::{JNIStr, JNIString, MUTF8Chars},
};

use crate::test_error::{TestError, require};

pub fn test_roundtrips(env: &mut Env<'_>) -> Result<(), TestError> {
	let cases: &[&[u16]] = &[
		&[],
		&[0x0041, 0x0000, 0x65E5],
		&[0xD83D, 0xDE00],
		&[0x0000, 0xD83D, 0xDE00],
		&[0xD83D, 0xDE00, 0x0000],
		&[0xD800, 0xDC00, 0xDBFF, 0xDFFF],
		&[
			0xD800, 0x0041, 0x0000, 0xD83D, 0xDE00, 0xDC00, 0x65E5, 0xD83D, 0xDE00, 0xD800,
		],
	];
	for &units in cases {
		env.with_local_frame(8, |env| test_case(env, units))?;
	}
	Ok(())
}

fn test_case(env: &mut Env<'_>, units: &[u16]) -> Result<(), TestError> {
	// Construct through char[] so neither Rust UTF-8 nor NewStringUTF can alter
	// unpaired surrogates before ART's GetStringUTFChars is exercised.
	let array: JCharArray<'_> = env.new_char_array(units.len())?;
	array.set_region(env, 0, units)?;
	let object: JObject<'_> =
		env.new_object(jni_str!("java/lang/String"), jni_sig!("([C)V"), &[JValue::from(&array)])?;
	let string: JString<'_> = env.cast_local::<JString>(object)?;
	let expected: String = String::from_utf16_lossy(units);
	require(
		string.try_to_string(env)? == expected,
		"Java string decoding changed characters",
	)?;
	require(string.to_string() == expected, "Java string display changed characters")?;
	let chars: MUTF8Chars<'_, &JString<'_>> = string.mutf8_chars(env)?;
	require(
		chars.to_str() == expected,
		"borrowed Java string decoding changed characters",
	)?;
	require(
		JNIStr::from_cstr(chars.as_cstr()).is_some(),
		"borrowed Java string is not strict MUTF-8",
	)?;
	let owned: JNIString = chars.into();
	let roundtrip: JString<'_> = JString::from_jni_str(env, &owned)?;
	require(
		roundtrip.try_to_string(env)? == expected,
		"owned JNI string round-trip failed",
	)?;

	for (index, &unit) in units.iter().enumerate() {
		let actual: u16 = env
			.call_method(
				&roundtrip,
				jni_str!("charAt"),
				jni_sig!("(I)C"),
				&[JValue::Int(i32::try_from(index).expect("small test string"))],
			)?
			.c()?;
		require(actual == unit, "normalization changed a UTF-16 code unit")?;
	}
	Ok(())
}
