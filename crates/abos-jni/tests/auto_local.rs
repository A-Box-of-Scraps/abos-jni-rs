#![allow(
	unsafe_code,
	reason = "Exercise cleanup of raw JNI references outside library guards."
)]
#![allow(missing_docs, reason = "Integration tests are not library API.")]
#![cfg(feature = "invocation")]

mod util;

use std::panic::{AssertUnwindSafe, catch_unwind};

use abos_jni::{
	JavaVM,
	errors::{Error, Result},
	jni_sig, jni_str,
	objects::{JObject, JString, JThrowable},
	refs::{Auto, IntoAuto as _, Reference as _},
	sys,
};
use rusty_fork::rusty_fork_test;

rusty_fork_test! {
#[test]
fn null_cleanup_needs_no_vm_or_attachment() {
	assert!(JavaVM::singleton().is_err());
	drop(JObject::null().auto());
	std::thread::spawn(|| drop(JString::null().auto())).join().expect("null cleanup on another thread");
	assert!(JavaVM::singleton().is_err());
	assert_eq!(JavaVM::thread_attach_guard_level(), 0);
}

#[test]
fn auto_cleanup_preserves_other_local_references_and_unwrap() {
	util::attach_current_thread(|env| {
		let original = env.new_string("still live")?.auto();
		let duplicate = env.new_local_ref(&original)?;
		let level = JavaVM::thread_attach_guard_level();
		drop(original);
		assert_eq!(JavaVM::thread_attach_guard_level(), level);
		assert_eq!(duplicate.try_to_string(env)?, "still live");
		let unwrapped = duplicate.auto().unwrap();
		assert_eq!(unwrapped.try_to_string(env)?, "still live");
		env.delete_local_ref(unwrapped);
		Ok(())
	}).expect("cleanup deletes only the owned local reference");
}

#[test]
fn outer_local_cleanup_does_not_change_nested_frame_checks() {
	util::attach_current_thread(|env| {
		let outer = env.new_string("outer frame")?.auto();
		let survivor = env.new_local_ref(&outer)?;
		let outer_level = JavaVM::thread_attach_guard_level();
		env.get_java_vm()?.with_local_frame(4, |inner| -> Result<()> {
			let inner_level = JavaVM::thread_attach_guard_level();
			assert!(inner_level > outer_level);
			drop(outer);
			assert_eq!(JavaVM::thread_attach_guard_level(), inner_level);
			assert_eq!(survivor.try_to_string(inner)?, "outer frame");
			// Deletion may use an outer reference, but creating one through an
			// outer Env must still fail while the inner frame is active.
			let panic = catch_unwind(AssertUnwindSafe(|| env.new_string("wrong frame")));
			assert!(panic.is_err());
			assert_eq!(JavaVM::thread_attach_guard_level(), inner_level);
			drop(inner.new_string("inner frame")?.auto());
			Ok(())
		})?;
		assert_eq!(JavaVM::thread_attach_guard_level(), outer_level);
		drop(env.new_string("outer frame remains usable")?.auto());
		env.delete_local_ref(survivor);
		Ok(())
	}).expect("cleanup preserves local-frame ownership and guard nesting");
}

#[test]
fn auto_cleanup_during_unwinding_preserves_pending_exception() {
	util::attach_current_thread(|env| {
		let exception = env.new_object(jni_str!("java/lang/RuntimeException"), jni_sig!("()V"), &[])?;
		let exception = env.cast_local::<JThrowable>(exception)?;
		let level = JavaVM::thread_attach_guard_level();
		let panic = catch_unwind(AssertUnwindSafe(|| {
			let _local = env.new_string("drop during unwind").expect("allocate string").auto();
			assert!(matches!(env.throw(&exception), Err(Error::JavaException)));
			panic!("unwind with a pending Java exception");
		}));
		assert!(panic.is_err());
		assert_eq!(JavaVM::thread_attach_guard_level(), level);
		assert!(env.exception_check());
		let pending = env.exception_occurred().expect("exception remains pending");
		env.exception_clear();
		assert!(env.is_same_object(&exception, &pending)?);
		env.delete_local_ref(pending);
		env.delete_local_ref(exception);
		drop(env.new_string("recovered")?.auto());
		Ok(())
	}).expect("cleanup during panic leaves the original Java exception untouched");
}

#[test]
fn native_local_cleanup_works_without_a_library_guard() {
	let vm = util::jvm();
	assert_eq!(JavaVM::thread_attach_guard_level(), 0);
	let raw_env = raw_attached_env(vm);
	// SAFETY: VM creation attached this thread; the raw references remain on its
	// original JNI frame until explicitly deleted below, before any detach.
	let (local, duplicate) = unsafe {
		let local = ((**raw_env).v1_1.NewStringUTF)(raw_env, c"native local".as_ptr());
		assert!(!local.is_null());
		let duplicate = ((**raw_env).v1_2.NewLocalRef)(raw_env, local);
		assert!(!duplicate.is_null());
		(local, duplicate)
	};
	// SAFETY: This uniquely wraps the local above, on its owning thread. It is
	// dropped immediately while its original JNI frame is still live.
	let local = unsafe { JObject::kind_from_raw(local) };
	drop(local.auto());
	assert_eq!(JavaVM::thread_attach_guard_level(), 0);
	vm.attach_current_thread_for_scope(|env| -> Result<()> {
		// SAFETY: The duplicate is still owned by the live outer JNI frame and has
		// not been wrapped or deleted. The shorter guard lifetime is valid for it.
		let duplicate = unsafe { JString::from_raw(env, duplicate) };
		assert_eq!(duplicate.try_to_string(env)?, "native local");
		env.delete_local_ref(duplicate);
		Ok(())
	}).expect("fallback cleanup preserves other native locals");
	assert_eq!(JavaVM::thread_attach_guard_level(), 0);
}

#[test]
fn cleanup_allows_scoped_detach_and_reattach() {
	let vm = util::jvm();
	std::thread::scope(|scope| {
		scope.spawn(|| {
			for _ in 0..3 {
				assert!(!vm.is_thread_attached().expect("query attachment"));
				vm.attach_current_thread_for_scope(|env| -> Result<()> {
					drop(env.new_string("scoped")?.auto());
					assert!(matches!(vm.detach_current_thread(), Err(Error::ThreadAttachmentGuarded)));
					Ok(())
				}).expect("scoped cleanup");
				assert_eq!(JavaVM::thread_attach_guard_level(), 0);
				drop(JObject::null().auto());
				assert!(!vm.is_thread_attached().expect("cleanup must not attach"));
			}
		}).join().expect("worker succeeds");
	});
}
}

fn raw_attached_env(vm: &JavaVM) -> *mut sys::JNIEnv {
	let mut raw: *mut std::ffi::c_void = std::ptr::null_mut();
	// SAFETY: GetEnv queries this thread without attaching it; raw is writable
	// output storage, and the returned status is checked before using the pointer.
	let result: sys::jint = unsafe { ((**vm.get_raw()).v1_2.GetEnv)(vm.get_raw(), &raw mut raw, sys::JNI_VERSION_1_4) };
	assert_eq!(result, sys::JNI_OK);
	raw.cast()
}

#[test]
fn auto_keeps_the_local_wrapper_size() {
	assert_eq!(size_of::<Auto<'_, JObject<'_>>>(), size_of::<JObject<'_>>());
}
