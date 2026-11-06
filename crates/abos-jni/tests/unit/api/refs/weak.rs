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

#[test]
fn weak_references_do_not_expose_strong_views() {
	use crate::objects::JString;
	static_assertions::assert_not_impl_any!(Weak<JObject<'static>>: AsRef<JObject<'static>>, std::ops::Deref, Into<JObject<'static>>);
	static_assertions::assert_not_impl_any!(Weak<JString<'static>>: AsRef<JString<'static>>, AsRef<JObject<'static>>, std::ops::Deref);
}
