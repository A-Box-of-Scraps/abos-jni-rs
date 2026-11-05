#![allow(missing_docs, reason = "Public items are macro test fixtures.")]

use std::mem::{offset_of, size_of};

use abos_jni_sys_macros::jni_to_union;

// No c_void import: generated padding must resolve its type independently.
#[jni_to_union]
pub struct Table {
	#[jni_added("reserved")]
	pub reserved0: *mut (),
	pub first: unsafe extern "system" fn(),
	#[jni_added("1.2")]
	pub second: unsafe extern "system" fn(),
	pub third: unsafe extern "system" fn(),
	#[jni_added("9")]
	pub fourth: unsafe extern "system" fn(),
	#[jni_added("24")]
	pub fifth: unsafe extern "system" fn(),
	#[jni_added("reserved")]
	pub reserved1: *mut (),
}

#[test]
fn versioned_tables_keep_interior_padding_and_trim_trailing_fields() {
	let slot: usize = size_of::<*mut ()>();
	assert_eq!(size_of::<Table_1_1>(), 4 * slot);
	assert_eq!(size_of::<Table_1_2>(), 4 * slot);
	assert_eq!(size_of::<Table_9>(), 5 * slot);
	assert_eq!(size_of::<Table_24>(), 6 * slot);
	assert_eq!(size_of::<Table_reserved>(), 7 * slot);
	assert_eq!(size_of::<Table>(), 7 * slot);
	assert_eq!(offset_of!(Table_1_1, first), slot);
	assert_eq!(offset_of!(Table_1_1, third), 3 * slot);
	assert_eq!(offset_of!(Table_1_2, second), 2 * slot);
	assert_eq!(offset_of!(Table_9, fourth), 4 * slot);
	assert_eq!(offset_of!(Table_24, fifth), 5 * slot);
}

#[test]
fn macro_diagnostics() {
	let cases = trybuild::TestCases::new();
	cases.pass("tests/trybuild/pass.rs");
	cases.compile_fail("tests/trybuild/fail-*.rs");
}
