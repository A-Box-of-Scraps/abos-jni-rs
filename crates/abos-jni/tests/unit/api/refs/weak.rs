use super::{JObject, Weak};

#[test]
fn test_weak_ref_send() {
	fn assert_send<T: Send>() {}
	assert_send::<Weak<JObject<'static>>>();
}

#[test]
fn test_weak_ref_sync() {
	fn assert_sync<T: Sync>() {}
	assert_sync::<Weak<JObject<'static>>>();
}
