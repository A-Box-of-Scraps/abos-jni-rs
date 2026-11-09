use super::MethodReferenceCache;
use crate::{errors::Error, objects::JClass, refs::Global};
use std::sync::{
	Barrier,
	atomic::{AtomicUsize, Ordering},
};

#[test]
fn successful_initialization_is_reused() {
	let cache = MethodReferenceCache::new();
	let calls: AtomicUsize = AtomicUsize::new(0);
	for _ in 0..3 {
		let parameters: &[Global<JClass<'static>>] = cache
			.get_or_try_init(|| {
				calls.fetch_add(1, Ordering::Relaxed);
				Ok(vec![Global::null()])
			})
			.expect("cache initialization succeeds");
		assert_eq!(parameters.len(), 1);
	}
	assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn failed_initialization_can_be_retried() {
	let cache = MethodReferenceCache::new();
	assert!(matches!(
		cache.get_or_try_init(|| Err(Error::WrongObjectType)),
		Err(Error::WrongObjectType)
	));
	assert!(cache.parameters.get().is_none());
	assert!(
		cache
			.get_or_try_init(|| Ok(Vec::new()))
			.expect("retry succeeds")
			.is_empty()
	);
}

#[test]
fn initialization_can_reenter_without_deadlocking() {
	let cache = MethodReferenceCache::new();
	let parameters: &[Global<JClass<'static>>] = cache
		.get_or_try_init(|| {
			cache.get_or_try_init(|| Ok(vec![Global::null()]))?;
			Ok(vec![Global::null(), Global::null()])
		})
		.expect("reentrant initialization succeeds");
	// The nested initializer publishes first; the outer result must not replace it.
	assert_eq!(parameters.len(), 1);
}

#[test]
fn concurrent_initializers_publish_one_result() {
	let cache = MethodReferenceCache::new();
	let barrier = Barrier::new(4);
	let initialize = || {
		barrier.wait();
		Ok(vec![Global::null()])
	};
	std::thread::scope(|scope| {
		for _ in 0..4 {
			scope.spawn(|| {
				let parameters: &[Global<JClass<'static>>] = cache
					.get_or_try_init(initialize)
					.expect("concurrent initialization succeeds");
				assert_eq!(
					parameters.as_ptr(),
					cache.parameters.get().expect("published cache").as_ptr()
				);
			});
		}
	});
}
