#[path = "../support/generate.rs"]
mod generate;
#[path = "../support/types.rs"]
mod types;

use syn::{Type, parse_quote};

#[test]
fn preserves_pointer_constness_at_each_level() {
	let ty: Type = parse_quote!(*const *mut core_type);
	assert_eq!(types::c_type(&ty), "core_type * const *");
	let ty: Type = parse_quote!(*mut *const core_type);
	assert_eq!(types::c_type(&ty), "core_type const * *");
}

#[test]
fn preserves_function_abi_variadics_and_parameter_order() {
	let ty: Type = parse_quote!(unsafe extern "system" fn(*mut JNIEnv, jobject, *const jvalue) -> jint);
	assert_eq!(
		types::c_type(&ty),
		"jint (JNICALL *)(JNIEnv *, jobject, jvalue const *)"
	);
	let ty: Type = parse_quote!(unsafe extern "C" fn(*mut JNIEnv, ...) -> jobject);
	assert_eq!(types::c_type(&ty), "jobject (*)(JNIEnv *, ...)");
	let ty: Type = parse_quote!(unsafe extern "system" fn() -> !);
	assert_eq!(types::c_type(&ty), "void (JNICALL *)(void)");
}

#[test]
#[should_panic(expected = "Unsupported JNI type")]
fn rejects_unsupported_types_instead_of_skipping_checks() {
	types::c_type(&parse_quote!(&jint));
}

#[test]
fn emits_checks_for_every_historical_view_without_reading_tables() {
	let mut probe = generate::Probe::default();
	probe.add_source(
		r#"
        #[jni_to_union]
        pub struct JNINativeInterface_ {
            #[jni_added("reserved")]
            pub reserved0: *mut c_void,
            pub first: unsafe extern "system" fn(),
            #[jni_added("1.2")]
            pub second: unsafe extern "system" fn(),
            pub third: unsafe extern "system" fn(),
            #[jni_added("24")]
            pub last: unsafe extern "system" fn(),
        }
    "#,
	);
	let expectations = probe.expected.to_string();
	assert!(expectations.contains("offsetof(JNINativeInterface__1_1, third)"));
	assert!(!expectations.contains("offsetof(JNINativeInterface__1_1, second)"));
	assert!(!expectations.contains("offsetof(JNINativeInterface__1_1, reserved0)"));
	assert!(expectations.contains("offsetof(JNINativeInterface__1_2, second)"));
	assert!(expectations.contains("offsetof(JNINativeInterface__24, last)"));
	assert!(expectations.contains("offsetof(JNINativeInterface__reserved, reserved0)"));
	assert!(
		probe
			.measurements
			.contains("offsetof(struct JNINativeInterface_, third) + sizeof")
	);
	assert_eq!(probe.signatures.lines().count(), 5);
}

#[test]
fn emits_scalar_record_constant_and_exported_function_checks() {
	let mut probe = generate::Probe::default();
	probe.add_source(
		r#"
        pub type jint = i32;
        pub type jboolean = u8;
        pub type va_list<'a> = core::ffi::VaList<'a>;
        pub const JNI_OK: jint = 0;
        pub struct JavaVMOption { pub optionString: *mut c_char }
        pub union jvalue { pub i: jint }
        pub enum jobjectRefType { JNIInvalidRefType = 0 }
        unsafe extern "system" { pub fn JNI_GetDefaultJavaVMInitArgs(args: *mut c_void) -> jint; }
    "#,
	);
	assert!(probe.signatures.contains("int: sizeof(int) == sizeof(int32_t)"));
	assert!(
		probe
			.signatures
			.contains("unsigned char: sizeof(unsigned char) == sizeof(uint8_t)")
	);
	assert!(probe.signatures.contains("void (*)(va_list): 1"));
	assert!(probe.signatures.contains("&JNI_GetDefaultJavaVMInitArgs"));
	assert!(probe.signatures.contains("jint (JNICALL *)(void *)"));
	assert!(probe.measurements.contains("sizeof(jboolean)"));
	assert!(probe.measurements.contains("sizeof(va_list)"));
	assert!(probe.measurements.contains("_Alignof(va_list)"));
	assert!(probe.measurements.contains("(int64_t)(JNI_OK)"));
	assert!(probe.measurements.contains("(int64_t)(JNIInvalidRefType)"));
	assert!(probe.measurements.contains("offsetof(JavaVMOption, optionString)"));
	assert!(probe.measurements.contains("offsetof(jvalue, i)"));
}

#[test]
fn integer_aliases_check_width_and_sign_without_requiring_identical_c_typedefs() {
	let mut probe = generate::Probe::default();
	probe.add_source("pub type jlong = i64; pub type unsigned_word = u64;");
	let signed = probe.signatures.lines().next().expect("signed alias assertion");
	assert!(signed.contains("_Generic((jlong)0"));
	assert!(signed.contains("long: sizeof(long) == sizeof(int64_t)"));
	assert!(signed.contains("long long: sizeof(long long) == sizeof(int64_t)"));
	assert!(!signed.contains("unsigned"));
	let unsigned = probe.signatures.lines().nth(1).expect("unsigned alias assertion");
	assert!(unsigned.contains("unsigned long: sizeof(unsigned long) == sizeof(uint64_t)"));
	assert!(unsigned.contains("unsigned long long: sizeof(unsigned long long) == sizeof(uint64_t)"));
	assert!(signed.contains("default: 0"));
	assert!(unsigned.contains("default: 0"));
	assert!(probe.measurements.contains("sizeof(jlong)"));
	assert!(probe.measurements.contains("_Alignof(jlong)"));
}

#[test]
fn incorrect_boolean_and_va_list_aliases_are_not_silently_substituted() {
	let mut probe = generate::Probe::default();
	probe.add_source("pub type jboolean = bool; pub type va_list = *mut c_void;");
	assert!(probe.signatures.contains("void (*)(_Bool): 1"));
	assert!(probe.signatures.contains("void (*)(void *): 1"));
	assert!(probe.measurements.contains("sizeof(va_list)"));
}

#[test]
fn va_list_parameters_keep_the_c_type_and_accept_only_lifetimes() {
	let ty: Type = parse_quote!(unsafe extern "system" fn(args: va_list<'_>));
	assert_eq!(types::c_type(&ty), "void (JNICALL *)(va_list)");
}

#[test]
#[should_panic(expected = "va_list requires exactly one lifetime argument")]
fn rejects_va_list_type_arguments() {
	types::c_type(&parse_quote!(va_list<u8>));
}

#[test]
fn nullable_callback_types_preserve_the_function_signature() {
	let ty: Type = parse_quote!(Option<unsafe extern "system" fn(*mut jvmtiEnv, *mut JNIEnv)>);
	assert_eq!(types::c_type(&ty), "void (JNICALL *)(jvmtiEnv *, JNIEnv *)");
}

#[test]
#[should_panic(expected = "Only nullable function pointers")]
fn rejects_options_without_a_guaranteed_c_niche() {
	types::c_type(&parse_quote!(Option<jint>));
}

#[test]
fn opaque_bitfield_storage_still_gets_record_layout_checks() {
	let mut probe = generate::Probe::default();
	probe.add_source("pub struct jvmtiCapabilities { words: [u32; 4] }");
	let expected = probe.expected.to_string();
	assert!(expected.contains("sizeof(jvmtiCapabilities)"));
	assert!(expected.contains("alignof(jvmtiCapabilities)"));
	assert_eq!(probe.signatures, "");
}

#[test]
fn fixed_arrays_check_element_type_and_length_before_decay() {
	let mut probe = generate::Probe::default();
	probe.add_source("pub struct Example { pub message: [c_char; 50] }");
	assert!(probe.signatures.contains("char *: 1"));
	assert!(
		probe
			.measurements
			.contains("sizeof(((Example *)0)->message) / sizeof(((Example *)0)->message[0])")
	);
	assert!(probe.expected.to_string().contains("lengthof(Example.message)"));
}
