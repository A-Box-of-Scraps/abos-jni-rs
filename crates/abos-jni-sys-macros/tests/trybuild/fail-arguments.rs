use abos_jni_sys_macros::jni_to_union;

#[jni_to_union(unexpected)]
pub struct Table {
	pub field: *mut (),
}

fn main() {}
