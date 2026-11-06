use super::{Global, JObject};

#[test]
fn test_global_ref_send() {
	fn assert_send<T: Send>() {}
	assert_send::<Global<JObject<'static>>>();
}

#[test]
fn test_global_ref_sync() {
	fn assert_sync<T: Sync>() {}
	assert_sync::<Global<JObject<'static>>>();
}
