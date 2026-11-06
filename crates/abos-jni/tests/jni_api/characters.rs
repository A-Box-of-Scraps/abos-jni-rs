use super::*;

#[test]
fn test_java_char_conversion() {
	attach_current_thread(|env| {
		// Make a Java `StringBuilder`.
		let sb = unwrap(
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

		// Finish the `StringBuilder` and get a Java `String`.
		let s = unwrap(
			env.call_method(&sb, jni_str!("toString"), jni_sig!("()Ljava/lang/String;"), &[]),
			env,
		)
		.l()
		.expect("JNI result has the expected value");

		env.delete_local_ref(sb);

		{
			// The first character in the string is U+1F913, which is not representable in a single UTF-16 unit.

			// Get the first Java `char` and try to unwrap it to a Rust `char`.
			#[allow(
				deprecated,
				reason = "Exercise the deprecated character conversion API for compatibility."
			)]
			let c = unwrap(
				env.call_method(&s, jni_str!("charAt"), jni_sig!("(I)C"), &[JValue::Int(0)]),
				env,
			)
			.c_char();

			// That should fail.
			let c = assert_matches!(
				c,
				Err(Error::InvalidUtf16 { source })
				=> source
			);

			// The unpaired surrogate should be correct.
			assert_eq!(c.unpaired_surrogate(), 0xd83e);
		}

		{
			// The first character in the string *is* representable in a single UTF-32 unit.

			// Get the UTF-32 unit and unwrap it.
			let c = unwrap(
				env.call_method(&s, jni_str!("codePointAt"), jni_sig!("(I)I"), &[JValue::Int(0)]),
				env,
			)
			.i_char()
			.expect("JNI result has the expected value");

			// It should be correct.
			assert_eq!(c, '🤓');
		}

		{
			// The second character in the string *is* representable in a single UTF-16 unit.

			// Get it and unwrap it. It should succeed.
			#[allow(
				deprecated,
				reason = "Exercise the deprecated character conversion API for compatibility."
			)]
			let c = unwrap(
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

			// It should be correct.
			assert_eq!(c, '☃');
		}

		Ok(())
	})
	.expect("attached JNI callback succeeds");
}
