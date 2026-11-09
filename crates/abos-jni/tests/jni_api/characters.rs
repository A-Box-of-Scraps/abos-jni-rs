use super::*;

#[test]
fn test_java_char_conversion() {
	attach_current_thread(|env| {
		let sb: abos_jni::objects::JObject<'_> = unwrap(
			env.new_object(jni_str!("java/lang/StringBuilder"), jni_sig!("()V"), &[]),
			env,
		);

		// U+1F913 is not representable in a single UTF-16 unit, so this conversion should fail.
		assert_matches!(JValue::try_from('🤓'), Err(CharToJavaError { char: '🤓' }));

		// It is of course representable in a single UTF-32 unit.
		unwrap(
			env.call_method(
				&sb,
				jni_str!("appendCodePoint"),
				jni_sig!("(I)Ljava/lang/StringBuilder;"),
				&[JValue::int_from_char('🤓')],
			),
			env,
		);

		// U+2603, on the other hand, *is* representable in a single UTF-16 unit.
		unwrap(
			env.call_method(
				&sb,
				jni_str!("append"),
				jni_sig!("(C)Ljava/lang/StringBuilder;"),
				&[JValue::try_from('☃').expect("try from succeeds")],
			),
			env,
		);

		let s: abos_jni::objects::JObject<'_> = unwrap(
			env.call_method(&sb, jni_str!("toString"), jni_sig!("()Ljava/lang/String;"), &[]),
			env,
		)
		.l()
		.expect("JNI result has the expected value");

		env.delete_local_ref(sb);

		{
			// The first character in the string is U+1F913, which is not representable in a single UTF-16 unit.

			#[allow(
				deprecated,
				reason = "Exercise the deprecated character conversion API for compatibility."
			)]
			let c: std::result::Result<char, abos_jni::errors::Error> = unwrap(
				env.call_method(&s, jni_str!("charAt"), jni_sig!("(I)C"), &[JValue::Int(0)]),
				env,
			)
			.c_char();

			let c: std::char::DecodeUtf16Error = assert_matches!(
				c,
				Err(Error::InvalidUtf16 { source })
				=> source
			);

			assert_eq!(c.unpaired_surrogate(), 0xd83e);
		}

		{
			// The first character in the string *is* representable in a single UTF-32 unit.

			let c: char = unwrap(
				env.call_method(&s, jni_str!("codePointAt"), jni_sig!("(I)I"), &[JValue::Int(0)]),
				env,
			)
			.i_char()
			.expect("JNI result has the expected value");

			assert_eq!(c, '🤓');
		}

		{
			// The second character in the string *is* representable in a single UTF-16 unit.

			#[allow(
				deprecated,
				reason = "Exercise the deprecated character conversion API for compatibility."
			)]
			let c: char = unwrap(
				env.call_method(
					&s,
					jni_str!("charAt"),
					jni_sig!("(I)C"),
					// The first character is represented in UTF-16 as a surrogate pair, so the second character occurs at index 2 instead of 1.
					&[JValue::Int(2)],
				),
				env,
			)
			.c_char()
			.expect("JNI result has the expected value");

			assert_eq!(c, '☃');
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}

#[test]
fn string_conversion_preserves_valid_characters_around_unpaired_surrogates() {
	attach_current_thread(|env| {
		// Include NUL, a supplementary character, and BMP text before and after
		// unpaired high and low surrogates, including a trailing high surrogate.
		let utf16: &[u16] = &[
			0x0041, 0x0000, 0xD83D, 0xDE00, 0xD800, 0x0042, 0x0000, 0xD83D, 0xDE00, 0xDC00, 0x65E5, 0xD800,
		];
		assert_string_conversion_matches_utf16(env, utf16)
	})
	.expect("Java string conversion succeeds");
}

#[test]
fn string_conversion_matches_utf16_for_every_code_unit() {
	attach_current_thread(|env| {
		let utf16: Vec<u16> = (0..=u16::MAX).collect();
		assert_string_conversion_matches_utf16(env, &utf16)
	})
	.expect("all UTF-16 code units are decoded correctly");
}

fn assert_string_conversion_matches_utf16(env: &mut Env<'_>, utf16: &[u16]) -> abos_jni::errors::Result<()> {
	let chars: abos_jni::objects::JCharArray<'_> = env.new_char_array(utf16.len())?;
	chars.set_region(env, 0, utf16)?;
	let object: JObject<'_> = env.new_object(STRING_CLASS, jni_sig!("([C)V"), &[JValue::from(&chars)])?;
	let string: JString<'_> = JString::cast_local(env, object)?;
	let expected: String = String::from_utf16_lossy(utf16);
	assert_eq!(string.try_to_string(env)?, expected);
	assert_eq!(string.mutf8_chars(env)?.to_str(), expected);
	Ok(())
}
