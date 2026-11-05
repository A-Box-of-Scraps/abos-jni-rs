#![no_std]

extern crate std;

use abos_jni_sys_macros::jni_to_union;

#[jni_to_union]
pub struct Table {
	#[jni_added("reserved")]
	pub reserved: *mut (),
	pub first: unsafe extern "system" fn(),
	#[jni_added("1.2")]
	pub second: unsafe extern "system" fn(),
	#[jni_added("999")]
	pub future: unsafe extern "system" fn(),
}

fn main() {
	assert_eq!(core::mem::size_of::<Table_1_1>(), 2 * core::mem::size_of::<*mut ()>());
	assert_eq!(core::mem::size_of::<Table_999>(), 4 * core::mem::size_of::<*mut ()>());
}
