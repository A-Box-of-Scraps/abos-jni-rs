//! Keep the committed seed corpus useful for reaching successful parsing paths.

use abos_cafebabe::{AccessFlags, ClassFile, parse_class};

fn parse_seed(bytes: &[u8]) -> ClassFile {
	let class: ClassFile = parse_class(bytes).expect("seed must parse successfully");
	assert_eq!(class.major_version, 52);
	assert_eq!(class.minor_version, 0);
	class
}

#[test]
fn minimal() {
	let class: ClassFile = parse_seed(include_bytes!("../seeds/cafebabe_parse_class/minimal.class"));
	assert_eq!(class.this_class, "java/lang/Object");
	assert!(class.super_class.is_none());
	assert!(class.interfaces.is_empty());
	assert!(class.fields.is_empty());
	assert!(class.methods.is_empty());
}

#[test]
fn members() {
	let class: ClassFile = parse_seed(include_bytes!("../seeds/cafebabe_parse_class/members.class"));
	assert_eq!(class.this_class, "sample/Members");
	assert_eq!(class.super_class.as_deref(), Some("java/lang/Object"));
	assert_eq!(class.interfaces, ["java/lang/Runnable"]);
	assert_eq!(class.fields.len(), 1);
	assert_eq!(class.fields[0].name, "value");
	assert_eq!(class.fields[0].descriptor, "I");
	assert!(class.fields[0].access_flags.contains(AccessFlags::STATIC));
	assert_eq!(class.methods.len(), 1);
	assert_eq!(class.methods[0].name, "run");
	assert_eq!(class.methods[0].descriptor, "()V");
	assert!(class.methods[0].access_flags.contains(AccessFlags::NATIVE));
}

#[test]
fn unicode() {
	let class: ClassFile = parse_seed(include_bytes!("../seeds/cafebabe_parse_class/unicode.class"));
	assert_eq!(class.this_class, "sample/Café");
	assert_eq!(class.super_class.as_deref(), Some("java/lang/Object"));
	assert!(class.interfaces.is_empty());
	assert_eq!(class.fields.len(), 1);
	assert_eq!(class.fields[0].name, "𐐀");
	assert_eq!(class.fields[0].descriptor, "Ljava/lang/String;");
	assert!(class.methods.is_empty());
}

fn seed_files(directory: &str) -> Vec<(String, Vec<u8>)> {
	let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("seeds")
		.join(directory);
	let mut seeds: Vec<(String, Vec<u8>)> = std::fs::read_dir(root)
		.expect("seed directory exists")
		.map(|entry| {
			let entry = entry.expect("seed directory entry");
			let name = entry.file_name().into_string().expect("seed filename is UTF-8");
			(name, std::fs::read(entry.path()).expect("seed is readable"))
		})
		.collect();
	seeds.sort_by(|left, right| left.0.cmp(&right.0));
	assert!(!seeds.is_empty());
	seeds
}

#[test]
fn binding_metadata() {
	for (name, bytes) in seed_files("cafebabe_parse_class") {
		let metadata = parse_class(&bytes).expect("class metadata seed");
		let binding = abos_jbindgen::fuzzing::parse_class(&bytes).expect("binding metadata seed");
		assert_eq!(binding.class_name, metadata.this_class, "{name}");
		assert_eq!(binding.fields.len(), metadata.fields.len(), "{name}");
		if name == "members.class" {
			assert_eq!(binding.fields[0].type_info.name, "int");
			assert!(binding.fields[0].is_static);
			assert_eq!(binding.native_methods.len(), 1);
			assert_eq!(binding.native_methods[0].signature.return_type.name, "void");
			assert_eq!(binding.instance_of[0].java_type, "java.lang.Runnable");
		}
	}
}

#[test]
fn descriptor_seeds() {
	use abos_jbindgen::fuzzing::{parse_method_descriptor, parse_type_descriptor};
	use abos_jni::signature::{JavaType, RuntimeFieldSignature, RuntimeMethodSignature};

	for (name, bytes) in seed_files("descriptors") {
		let input = std::str::from_utf8(&bytes).expect("descriptor seed is UTF-8");
		let method = RuntimeMethodSignature::from_str(input);
		let field = RuntimeFieldSignature::from_str(input);
		let ty = input.parse::<JavaType>();
		let binding_method = parse_method_descriptor(input);
		let binding_field = parse_type_descriptor(input);
		match name.as_str() {
			"invalid-void-array" | "truncated-method" => {
				assert!(method.is_err() && field.is_err() && ty.is_err(), "{name}");
				assert!(binding_method.is_err() && binding_field.is_err(), "{name}");
			}
			"method" => {
				let parsed = method.expect("valid method seed");
				assert_eq!(parsed.method_signature().sig().to_str(), input);
				assert!(binding_method.is_ok());
				assert!(field.is_err() && ty.is_err() && binding_field.is_err());
			}
			_ => {
				let parsed = field.expect("valid field seed");
				assert_eq!(parsed.field_signature().sig().to_str(), input);
				assert!(ty.is_ok() && binding_field.is_ok(), "{name}");
				assert!(method.is_err() && binding_method.is_err(), "{name}");
			}
		}
	}
}

#[test]
fn string_roundtrip_seeds() {
	use abos_jni::strings::{JNIStr, JNIString};

	for (_, bytes) in seed_files("jni_string_roundtrip") {
		let input = std::str::from_utf8(&bytes).expect("string seed is UTF-8");
		let encoded = JNIString::new(input);
		assert_eq!(encoded.to_str(), input);
		let validated = JNIStr::from_cstr(encoded.as_cstr()).expect("encoded string is valid");
		assert_eq!(validated.to_str(), input);
	}
	assert_eq!(JNIString::new("").to_str(), "");
}

#[test]
fn modified_utf8_seeds() {
	use abos_jni::strings::{JNIStr, JNIString};
	use std::ffi::CString;

	for (name, bytes) in seed_files("jni_mutf8") {
		let cstring = CString::new(bytes).expect("seed has no interior NUL");
		let validated = JNIStr::from_cstr(&cstring);
		if matches!(name.as_str(), "truncated" | "overlong" | "standard-utf8") {
			assert!(validated.is_none(), "{name}");
		} else {
			let validated = validated.expect("valid modified UTF-8 code units");
			assert_eq!(validated.to_bytes(), cstring.to_bytes());
			let decoded = validated.to_str();
			assert_eq!(JNIString::new(&decoded).to_str(), decoded);
			match name.as_str() {
				"nul" => assert_eq!(decoded, "before\0after"),
				"surrogate-pair" => assert_eq!(decoded, "𐐀"),
				"unpaired-surrogate" => assert!(decoded.contains('\u{fffd}')),
				_ => assert_eq!(decoded, "java/lang/String"),
			}
		}
	}
	assert_eq!(JNIStr::from_cstr(c"").expect("empty string is valid").to_str(), "");
}

#[test]
fn jvmti_compiled_method_seeds() {
	for (_, bytes) in seed_files("jvmti_compiled_method") {
		// Shortened seeds also exercise zero-filled defaults and partial graphs.
		for length in 0..=bytes.len() {
			abos_jni_fuzz::jvmti_compiled_method::run(&bytes[..length]);
		}
	}
}

#[test]
fn jvmti_returned_data_seeds() {
	for (_, bytes) in seed_files("jvmti_returned_data") {
		abos_jni_fuzz::jvmti_returned_data::run(&bytes);
	}
	abos_jni_fuzz::jvmti_returned_data::run(&[]);
}

fn check_cleanup_failure_positions(kind: u8, mode: u8, action: u8) {
	// Eight local-variable entries own up to 24 strings plus the outer array.
	// Cover every possible failure position, including one beyond the last free.
	for missing in 0..3 {
		for fail_at in 0..=26 {
			for optional in [0, 255] {
				let bytes = [kind, mode, action, missing, fail_at, 7, 3, optional, b'x', 0, 255];
				abos_jni_fuzz::jvmti_returned_data::run(&bytes);
			}
		}
	}
}

#[test]
fn jvmti_cleanup_failure_matrix() {
	for kind in 0..3 {
		for mode in 0..7 {
			for action in 0..3 {
				check_cleanup_failure_positions(kind, mode, action);
			}
		}
	}
}
