#![allow(unsafe_code, reason = "Binding macros generate JNI calls.")]
#![allow(missing_docs, reason = "Integration test fixtures are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use abos_jni::{
	Env, JValue, bind_java_type,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JClassLoader, JObject, JObjectArray, JString},
	refs::Reference,
	signature::RuntimeMethodSignature,
	strings::JNIString,
};
use assert_matches::assert_matches;
use rusty_fork::rusty_fork_test;

bind_java_type! {
	ReferenceTypes => "com.example.ReferenceTypes",
	constructors {
		fn new(),
		fn with_comparable(value: "java.lang.Comparable"),
	},
	methods {
		fn accept_comparable(value: "java.lang.Comparable") -> jint,
		fn accept_comparables(value: "java.lang.Comparable"[]) -> jint,
		static fn accept_static(value: "java.lang.Comparable") -> jint,
	},
	fields {
		comparable: "java.lang.Comparable",
		static static_comparable: "java.lang.Comparable",
		comparables: "java.lang.Comparable"[],
		static static_comparables: "java.lang.Comparable"[],
	}
}

fn compile_fixture(name: &str) -> std::path::PathBuf {
	let output: std::path::PathBuf = util::setup_test_output(name);
	let _classes: Vec<std::path::PathBuf> = abos_javac::Build::new()
		.file("tests/java/com/example/ReferenceTypes.java")
		.output_dir(&output)
		.compile();
	output
}

fn assert_wrong_type<T: std::fmt::Debug>(result: Result<T>, env: &Env<'_>) {
	assert_matches!(
		result.expect_err("incompatible reference must be rejected"),
		Error::WrongObjectType
	);
	assert!(!env.exception_check());
}

// A safe AsRef implementation may return a different object on each call.
struct AlternatingObject<'a> {
	valid: &'a JObject<'a>,
	invalid: &'a JObject<'a>,
	calls: std::cell::Cell<usize>,
}

impl<'a> AsRef<JObject<'a>> for AlternatingObject<'a> {
	fn as_ref(&self) -> &JObject<'a> {
		let calls: usize = self.calls.get();
		self.calls.set(calls + 1);
		if calls == 0 { self.valid } else { self.invalid }
	}
}

rusty_fork_test! {
#[test]
fn object_array_rejects_incompatible_initializers() {
	util::attach_current_thread(|env| {
		let object = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let ints = env.new_int_array(1)?;
		for length in [2, 0] {
			for invalid in [&object, ints.as_ref()] {
				assert_wrong_type(env.new_object_array(length, jni_str!("java/lang/String"), invalid), env);
			}
		}
		let string = env.new_string("still usable")?;
		assert_eq!(string.try_to_string(env)?, "still usable");
		Ok(())
	})
	.expect("incompatible array initializers are rejected");
}

#[test]
fn object_array_accepts_assignable_and_null_initializers() {
	util::attach_current_thread(|env| {
		let string = env.new_string("initial")?;
		for class in [jni_str!("java/lang/String"), jni_str!("java/lang/Object"), jni_str!("java/lang/Comparable")] {
			let array = env.new_object_array(2, class, &string)?;
			for index in 0..2 {
				let element = array.get_element(env, index)?;
				let element = env.cast_local::<JString>(element)?;
				assert_eq!(element.try_to_string(env)?, "initial");
			}
		}
		for value in [string.as_ref(), &JObject::null()] {
			let array = env.new_object_array(2, jni_str!("java/lang/String"), value)?;
			let array = env.cast_local::<JObjectArray<JString>>(array)?;
			for index in 0..2 {
				let element = array.get_element(env, index)?;
				if value.is_null() {
					assert!(element.is_null());
				} else {
					assert_eq!(element.try_to_string(env)?, "initial");
				}
			}
		}
		Ok(())
	})
	.expect("valid array initializers preserve typed reads");
}

#[test]
fn object_array_uses_the_validated_initializer() {
	util::attach_current_thread(|env| {
		let string = env.new_string("initial")?;
		let object = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let value = AlternatingObject {
			valid: string.as_ref(),
			invalid: &object,
			calls: std::cell::Cell::new(0),
		};
		let array = env.new_object_array(2, jni_str!("java/lang/String"), &value)?;
		assert_eq!(value.calls.get(), 1);
		let array = env.cast_local::<JObjectArray<JString>>(array)?;
		for index in 0..2 {
			assert_eq!(array.get_element(env, index)?.try_to_string(env)?, "initial");
		}
		Ok(())
	})
	.expect("array allocation uses the checked reference");
}

#[test]
fn calls_validate_reference_assignability() {
	let output = compile_fixture("reference_calls");
	util::attach_current_thread(|env| {
		util::load_test_class(env, &output, "ReferenceTypes")?;
		let class = env.find_class(jni_str!("com/example/ReferenceTypes"))?;
		let receiver = env.new_object(&class, jni_sig!("()V"), &[])?;
		let object = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let string = env.new_string("valid")?;
		let strings = env.new_object_array(0, jni_str!("java/lang/String"), JObject::null())?;
		let objects = env.new_object_array(0, jni_str!("java/lang/Object"), JObject::null())?;
		let matrix = env.new_object_array(0, jni_str!("[Ljava/lang/String;"), JObject::null())?;
		let bytes = env.new_byte_array(0)?;
		let ints = env.new_int_array(0)?;
		let cases: [(&str, &str, &JObject, &JObject); 6] = [
			("acceptString", "(Ljava/lang/String;)I", &object, string.as_ref()),
			(
				"acceptComparable",
				"(Ljava/lang/Comparable;)I",
				&object,
				string.as_ref(),
			),
			(
				"acceptArray",
				"([Ljava/lang/String;)I",
				objects.as_ref(),
				strings.as_ref(),
			),
			(
				"acceptMatrix",
				"([[Ljava/lang/String;)I",
				strings.as_ref(),
				matrix.as_ref(),
			),
			("acceptBytes", "([B)I", ints.as_ref(), bytes.as_ref()),
			(
				"acceptComparables",
				"([Ljava/lang/Comparable;)I",
				objects.as_ref(),
				strings.as_ref(),
			),
		];
		for (name, signature, invalid, valid) in cases {
			let name = JNIString::new(name);
			let signature = RuntimeMethodSignature::from_str(signature)?;
			let signature = signature.method_signature();
			assert_wrong_type(
				env.call_method(&receiver, &name, &signature, &[JValue::Object(invalid)]),
				env,
			);
			assert_wrong_type(
				env.call_nonvirtual_method(&receiver, &class, &name, &signature, &[JValue::Object(invalid)]),
				env,
			);
			for value in [valid, &JObject::null()] {
				env.call_method(&receiver, &name, &signature, &[JValue::Object(value)])?;
				env.call_nonvirtual_method(&receiver, &class, &name, &signature, &[JValue::Object(value)])?;
			}
		}
		assert_wrong_type(
			env.call_static_method(
				&class,
				jni_str!("acceptStatic"),
				jni_sig!("(Ljava/lang/Comparable;)I"),
				&[JValue::Object(&object)],
			),
			env,
		);
		assert_wrong_type(
			env.new_object(
				&class,
				jni_sig!("(Ljava/lang/Comparable;)V"),
				&[JValue::Object(&object)],
			),
			env,
		);
		for value in [string.as_ref(), &JObject::null()] {
			env.call_static_method(
				&class,
				jni_str!("acceptStatic"),
				jni_sig!("(Ljava/lang/Comparable;)I"),
				&[JValue::Object(value)],
			)?;
			env.new_object(&class, jni_sig!("(Ljava/lang/Comparable;)V"), &[JValue::Object(value)])?;
		}
		// Reference parameter indices must include preceding primitive parameters.
		let mixed_sig = jni_sig!("(ILjava/lang/String;II)Z");
		assert_wrong_type(
			env.call_method(
				&string,
				jni_str!("regionMatches"),
				&mixed_sig,
				&[JValue::Int(0), JValue::Object(&object), JValue::Int(0), JValue::Int(1)],
			),
			env,
		);
		assert!(
			env.call_method(
				&string,
				jni_str!("regionMatches"),
				&mixed_sig,
				&[JValue::Int(0), JValue::Object(&string), JValue::Int(0), JValue::Int(1)]
			)?
			.z()?
		);
		// Rejected calls and constructors must not have executed their bodies.
		assert_eq!(env.get_static_field(&class, jni_str!("calls"), jni_sig!("I"))?.i()?, 28);
		Ok(())
	})
	.expect("reference call validation");
}

#[test]
fn fields_validate_before_writing() {
	let output = compile_fixture("reference_fields");
	util::attach_current_thread(|env| {
		util::load_test_class(env, &output, "ReferenceTypes")?;
		let class = env.find_class(jni_str!("com/example/ReferenceTypes"))?;
		let receiver = env.new_object(&class, jni_sig!("()V"), &[])?;
		let object = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let original = env
			.get_field(&receiver, jni_str!("text"), jni_sig!("Ljava/lang/String;"))?
			.l()?;
		let original_static = env
			.get_static_field(&class, jni_str!("staticText"), jni_sig!("Ljava/lang/String;"))?
			.l()?;
		assert_wrong_type(
			env.set_field(
				&receiver,
				jni_str!("text"),
				jni_sig!("Ljava/lang/String;"),
				JValue::Object(&object),
			),
			env,
		);
		assert_wrong_type(
			env.set_static_field(
				&class,
				jni_str!("staticText"),
				jni_sig!("Ljava/lang/String;"),
				JValue::Object(&object),
			),
			env,
		);
		let current = env
			.get_field(&receiver, jni_str!("text"), jni_sig!("Ljava/lang/String;"))?
			.l()?;
		let current_static = env
			.get_static_field(&class, jni_str!("staticText"), jni_sig!("Ljava/lang/String;"))?
			.l()?;
		assert!(env.is_same_object(&original, &current)?);
		assert!(env.is_same_object(&original_static, &current_static)?);

		let strings = env.new_object_array(0, jni_str!("java/lang/String"), JObject::null())?;
		let objects = env.new_object_array(0, jni_str!("java/lang/Object"), JObject::null())?;
		assert_wrong_type(
			env.set_field(
				&receiver,
				jni_str!("strings"),
				jni_sig!("[Ljava/lang/String;"),
				JValue::Object(&objects),
			),
			env,
		);
		assert_wrong_type(
			env.set_static_field(
				&class,
				jni_str!("staticStrings"),
				jni_sig!("[Ljava/lang/String;"),
				JValue::Object(&objects),
			),
			env,
		);
		for value in [strings.as_ref(), &JObject::null()] {
			env.set_field(
				&receiver,
				jni_str!("strings"),
				jni_sig!("[Ljava/lang/String;"),
				JValue::Object(value),
			)?;
			env.set_static_field(
				&class,
				jni_str!("staticStrings"),
				jni_sig!("[Ljava/lang/String;"),
				JValue::Object(value),
			)?;
		}
		let string = env.new_string("new")?;
		for value in [string.as_ref(), &JObject::null()] {
			env.set_field(
				&receiver,
				jni_str!("text"),
				jni_sig!("Ljava/lang/String;"),
				JValue::Object(value),
			)?;
			env.set_static_field(
				&class,
				jni_str!("staticText"),
				jni_sig!("Ljava/lang/String;"),
				JValue::Object(value),
			)?;
		}
		Ok(())
	})
	.expect("reference field validation");
}

#[test]
fn generated_bindings_validate_unmapped_references() {
	let output = compile_fixture("generated_reference_validation");
	util::attach_current_thread(|env| {
		util::load_test_class(env, &output, "ReferenceTypes")?;
		let receiver = ReferenceTypes::new(env)?;
		let object = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		assert_wrong_type(receiver.accept_comparable(env, &object), env);
		assert_wrong_type(ReferenceTypes::accept_static(env, &object), env);
		assert_wrong_type(ReferenceTypes::with_comparable(env, &object), env);
		assert_wrong_type(receiver.set_comparable(env, &object), env);
		assert_wrong_type(ReferenceTypes::set_static_comparable(env, &object), env);
		assert!(receiver.comparable(env)?.is_null());
		assert!(ReferenceTypes::static_comparable(env)?.is_null());
		let objects = env.new_object_array(0, jni_str!("java/lang/Object"), JObject::null())?;
		assert_wrong_type(receiver.accept_comparables(env, &objects), env);
		assert_wrong_type(receiver.set_comparables(env, &objects), env);
		assert_wrong_type(ReferenceTypes::set_static_comparables(env, &objects), env);
		let strings = env.new_object_array(0, jni_str!("java/lang/String"), JObject::null())?;
		receiver.accept_comparables(env, &strings)?;
		receiver.set_comparables(env, &strings)?;
		ReferenceTypes::set_static_comparables(env, &strings)?;
		let string = env.new_string("valid")?;
		for value in [string.as_ref(), &JObject::null()] {
			receiver.accept_comparable(env, value)?;
			ReferenceTypes::accept_static(env, value)?;
			ReferenceTypes::with_comparable(env, value)?;
			receiver.set_comparable(env, value)?;
			ReferenceTypes::set_static_comparable(env, value)?;
		}
		assert_eq!(
			env.get_static_field(jni_str!("com/example/ReferenceTypes"), jni_str!("calls"), jni_sig!("I"))?
				.i()?,
			7
		);
		Ok(())
	})
	.expect("generated reference validation");
}

#[test]
fn generated_bindings_use_the_validated_reference() {
	let output = compile_fixture("reference_as_ref");
	util::attach_current_thread(|env| {
		util::load_test_class(env, &output, "ReferenceTypes")?;
		let receiver = ReferenceTypes::new(env)?;
		let valid = env.new_string("valid")?;
		let invalid = env.new_object(jni_str!("java/lang/Object"), jni_sig!("()V"), &[])?;
		let value = AlternatingObject {
			valid: valid.as_ref(),
			invalid: &invalid,
			calls: std::cell::Cell::new(0),
		};
		for operation in 0..5 {
			value.calls.set(0);
			match operation {
				0 => {
					receiver.accept_comparable(env, &value)?;
				}
				1 => {
					ReferenceTypes::accept_static(env, &value)?;
				}
				2 => {
					ReferenceTypes::with_comparable(env, &value)?;
				}
				3 => receiver.set_comparable(env, &value)?,
				_ => ReferenceTypes::set_static_comparable(env, &value)?,
			}
			assert_eq!(value.calls.get(), 1);
		}
		Ok(())
	})
	.expect("validation and invocation use the same reference");
}

#[test]
fn validation_preserves_class_loader_identity_and_inheritance() {
	let output = compile_fixture("reference_loaders");
	let bytes = std::fs::read(output.join("com/example/ReferenceTypes.class")).expect("fixture class");
	let child_bytes = std::fs::read(output.join("com/example/ReferenceTypes$Child.class")).expect("child class");
	util::attach_current_thread(|env| {
		let urls = env.new_object_array(0, jni_str!("java/net/URL"), JObject::null())?;
		let mut classes = Vec::new();
		for _ in 0..2 {
			let loader = env.new_object(
				jni_str!("java/net/URLClassLoader"),
				jni_sig!("([Ljava/net/URL;Ljava/lang/ClassLoader;)V"),
				&[JValue::Object(&urls), JValue::Object(&JObject::null())],
			)?;
			let loader = env.new_cast_local_ref::<JClassLoader>(&loader)?;
			let base = env.define_class(Some(jni_str!("com/example/ReferenceTypes")), &loader, &bytes)?;
			let child = env.define_class(
				Some(jni_str!("com/example/ReferenceTypes$Child")),
				&loader,
				&child_bytes,
			)?;
			classes.push((base, child));
		}
		let first = env.new_object(&classes[0].1, jni_sig!("()V"), &[])?;
		let second = env.new_object(&classes[1].0, jni_sig!("()V"), &[])?;
		let signature = jni_sig!("(Lcom/example/ReferenceTypes;)Lcom/example/ReferenceTypes;");
		assert_wrong_type(
			env.call_method(&first, jni_str!("echo"), &signature, &[JValue::Object(&second)]),
			env,
		);
		assert_wrong_type(
			env.set_field(
				&first,
				jni_str!("peer"),
				jni_sig!("Lcom/example/ReferenceTypes;"),
				JValue::Object(&second),
			),
			env,
		);
		let returned = env
			.call_method(&first, jni_str!("echo"), &signature, &[JValue::Object(&first)])?
			.l()?;
		assert!(env.is_same_object(&first, &returned)?);
		env.set_field(
			&first,
			jni_str!("peer"),
			jni_sig!("Lcom/example/ReferenceTypes;"),
			JValue::Object(&first),
		)?;
		Ok(())
	})
	.expect("class loader identity validation");
}
}
