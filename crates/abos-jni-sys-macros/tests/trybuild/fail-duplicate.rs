use abos_jni_sys_macros::jni_to_union;

#[jni_to_union]
pub struct Table {
	#[jni_added("1.1")]
	#[jni_added("1.2")]
	pub field: *mut (),
}

fn main() {}
