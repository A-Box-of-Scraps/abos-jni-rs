use std::{
	borrow::Cow,
	ffi::{CStr, CString, c_void},
	ptr,
};

use super::{InitArgs, InitArgsBuilder, JvmError, checked_option_count};
use crate::sys::{JavaVMOption, jint};

#[test]
fn option_count_respects_jni_integer_range() {
	let limit: usize = usize::try_from(i32::MAX).expect("JNI integer limit fits usize");
	assert_eq!(checked_option_count(0).expect("zero options fit"), 0);
	assert_eq!(
		checked_option_count(limit).expect("maximum option count fits"),
		i32::MAX
	);
	for count in [limit + 1, usize::MAX] {
		let error: JvmError = checked_option_count(count).expect_err("oversized option count is rejected");
		assert!(matches!(error, JvmError::TooManyOptions(actual) if actual == count));
		assert!(error.opt_string().is_none());
	}
}

extern "system" fn exit_hook(_status: jint) {}
extern "system" fn abort_hook() {}

fn assert_raw_options(args: &InitArgs<'_>, expected: &[(&CStr, *mut c_void)]) {
	assert_eq!(
		usize::try_from(args.inner.nOptions).expect("nonnegative count"),
		expected.len()
	);
	// Safety: The live InitArgs owns the option array and all referenced option strings.
	let options: &[JavaVMOption] = unsafe { std::slice::from_raw_parts(args.inner.options, expected.len()) };
	for (option, (string, extra_info)) in options.iter().zip(expected) {
		// Safety: Each option string is a live, NUL-terminated CStr owned or borrowed by args.
		assert_eq!(unsafe { CStr::from_ptr(option.optionString) }, *string);
		assert_eq!(option.extraInfo, *extra_info);
	}
}

#[test]
fn safe_options_have_null_extra_info_and_filter_callbacks() {
	let mut builder: InitArgsBuilder<'_> = InitArgsBuilder::new();
	for name in ["vfprintf", "abort", "exit", "vfprintf\0", "abort\0", "exit\0"] {
		builder = builder.option(name);
		builder.try_option(name).expect("callback option is ignored");
	}
	for name in [c"vfprintf", c"abort", c"exit"] {
		builder = builder.option_encoded(name);
	}
	assert_eq!(builder.options().expect("no error").len(), 0);
	builder = builder.option("-Xcheck:jni").option_encoded(c"-Xmx64m").option("");
	builder.try_option("-Dtest=value\0").expect("valid option");
	let args: InitArgs<'_> = builder.build().expect("valid options");
	assert_raw_options(
		&args,
		&[
			(c"-Xcheck:jni", ptr::null_mut()),
			(c"-Xmx64m", ptr::null_mut()),
			(c"", ptr::null_mut()),
			(c"-Dtest=value", ptr::null_mut()),
		],
	);
}

#[test]
fn extra_info_survives_build_with_mixed_owned_and_borrowed_options() {
	let exit: *mut c_void = exit_hook as *mut c_void;
	let abort: *mut c_void = abort_hook as *mut c_void;
	let borrowed: CString = CString::new("exit").expect("valid C string");
	// Safety: The hooks have the expected ABI and static lifetime, do not unwind, and use no state.
	let mut builder: InitArgsBuilder<'_> = unsafe {
		InitArgsBuilder::new()
			.option("-Xcheck:jni")
			.option_with_extra_info(String::from("exit"), exit)
			.option_encoded_with_extra_info(CString::new("abort").expect("valid C string"), abort)
			.option_encoded(c"-Xmx64m")
			.option_encoded_with_extra_info(borrowed.as_c_str(), exit)
	};
	// Safety: Same static exit hook as above, including the optional string terminator.
	unsafe { builder.try_option_with_extra_info("exit\0", exit) }.expect("valid callback option");
	// Safety: Null extraInfo is valid for these ordinary options, including empty strings.
	builder = unsafe {
		builder
			.option_with_extra_info("", ptr::null_mut())
			.option_with_extra_info("\0", ptr::null_mut())
			.option_encoded_with_extra_info(c"-Dtest=value", ptr::null_mut())
	};
	let expected: &[(&CStr, *mut c_void)] = &[
		(c"-Xcheck:jni", ptr::null_mut()),
		(c"exit", exit),
		(c"abort", abort),
		(c"-Xmx64m", ptr::null_mut()),
		(c"exit", exit),
		(c"exit", exit),
		(c"", ptr::null_mut()),
		(c"", ptr::null_mut()),
		(c"-Dtest=value", ptr::null_mut()),
	];
	let strings: &[Cow<'_, CStr>] = builder.options().expect("no error");
	assert_eq!(strings.len(), expected.len());
	for (string, (expected_string, _)) in strings.iter().zip(expected) {
		assert_eq!(string.as_ref(), *expected_string);
	}
	// Moving the built arguments must not invalidate the raw string or option pointers.
	let args: Box<InitArgs<'_>> = Box::new(builder.build().expect("valid options"));
	assert_raw_options(&args, expected);
}

#[test]
fn explicit_null_callback_options_are_not_filtered() {
	// Safety: HotSpot accepts null hooks to retain its default behavior. This test only
	// inspects the arguments, without creating a JVM or invoking any callbacks.
	let mut builder: InitArgsBuilder<'_> = unsafe {
		InitArgsBuilder::new()
			.option_with_extra_info("vfprintf", ptr::null_mut())
			.option_encoded_with_extra_info(c"abort", ptr::null_mut())
	};
	// Safety: As above, null hooks are supported and the arguments are only inspected.
	unsafe { builder.try_option_with_extra_info("exit", ptr::null_mut()) }.expect("valid option");
	assert_raw_options(
		&builder.build().expect("valid options"),
		&[
			(c"vfprintf", ptr::null_mut()),
			(c"abort", ptr::null_mut()),
			(c"exit", ptr::null_mut()),
		],
	);
}

#[test]
fn extra_info_immediate_error_leaves_builder_unchanged() {
	let mut builder: InitArgsBuilder<'_> = InitArgsBuilder::new().option("-Xcheck:jni");
	// Safety: These ordinary options use null extraInfo; the invalid string is rejected.
	let error: JvmError = unsafe { builder.try_option_with_extra_info("-Dbad=\0value", ptr::null_mut()) }
		.expect_err("interior NUL is rejected");
	assert!(matches!(error, JvmError::NullOptString(_)));
	assert_eq!(error.opt_string(), Some("-Dbad=\0value"));
	// Safety: The static exit hook has the expected ABI and cannot unwind.
	unsafe { builder.try_option_with_extra_info("exit", exit_hook as *mut c_void) }.expect("builder remains usable");
	assert_raw_options(
		&builder.build().expect("no deferred error"),
		&[(c"-Xcheck:jni", ptr::null_mut()), (c"exit", exit_hook as *mut c_void)],
	);
}

#[test]
fn extra_info_deferred_errors_are_preserved() {
	for unsafe_first in [false, true] {
		let builder: InitArgsBuilder<'_> = InitArgsBuilder::new();
		let mut builder: InitArgsBuilder<'_> = if unsafe_first {
			// Safety: Null extraInfo for an ordinary option whose invalid string is rejected.
			unsafe { builder.option_with_extra_info("-Dfirst=\0bad", ptr::null_mut()) }
		} else {
			builder.option("-Dfirst=\0bad")
		};
		// Safety: The hooks have the expected ABI and static lifetime and cannot unwind.
		builder = unsafe {
			builder
				.option_with_extra_info("exit", exit_hook as *mut c_void)
				.option_encoded_with_extra_info(c"abort", abort_hook as *mut c_void)
		};
		// Safety: Null extraInfo for an ordinary option. The deferred error makes this a no-op.
		unsafe { builder.try_option_with_extra_info("-Dsecond=\0bad", ptr::null_mut()) }
			.expect("existing deferred error makes this a no-op");
		builder = builder.option("-Xcheck:jni").option_encoded(c"-Xmx64m");
		assert_eq!(
			builder.options().expect_err("deferred error").opt_string(),
			Some("-Dfirst=\0bad")
		);
		assert_eq!(
			builder.build().expect_err("deferred error").opt_string(),
			Some("-Dfirst=\0bad")
		);
	}
}

#[test]
fn default_builder_has_no_raw_options() {
	assert_raw_options(&InitArgsBuilder::new().build().expect("empty arguments"), &[]);
}
