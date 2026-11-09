//! Monitor ownership, failure, and cleanup tests without a JVM.
#![allow(unsafe_code, reason = "Initialized mock native tables exercise monitor ownership.")]

use std::{
	cell::{Cell, RefCell},
	ffi::c_char,
	ptr,
};

use abos_jni::jni_str;
use abos_jvmti::{Env, Error, JvmtiVersion, RawMonitor, RawMonitorGuard, Result, sys};

static_assertions::assert_impl_all!(RawMonitor<'static, 'static>: Send, Sync);
static_assertions::assert_not_impl_any!(RawMonitor<'static, 'static>: Clone, Copy);
static_assertions::assert_not_impl_any!(RawMonitorGuard<'static, 'static, 'static>: Send, Sync, Clone, Copy);

#[repr(C)]
struct Context {
	table: sys::jvmtiEnv,
	calls: RefCell<Vec<&'static str>>,
	fail: Cell<&'static str>,
}

const fn monitor_id() -> sys::jrawMonitorID {
	ptr::without_provenance_mut(0x1234)
}

unsafe fn record(env: *mut sys::jvmtiEnv, operation: &'static str) -> sys::jvmtiError {
	// SAFETY: The test keeps this repr(C) context alive through all native calls.
	let context: &Context = unsafe { &*env.cast::<Context>() };
	context.calls.borrow_mut().push(operation);
	if context.fail.get() == operation {
		sys::JVMTI_ERROR_INTERNAL
	} else {
		sys::JVMTI_ERROR_NONE
	}
}

unsafe extern "system" fn create(
	env: *mut sys::jvmtiEnv,
	name: *const c_char,
	output: *mut sys::jrawMonitorID,
) -> sys::jvmtiError {
	// SAFETY: Wrapper passes a terminated name and writable output to the live mock.
	unsafe {
		assert_eq!(std::ffi::CStr::from_ptr(name), c"test-monitor");
		*output = monitor_id();
		record(env, "create")
	}
}

unsafe extern "system" fn destroy(env: *mut sys::jvmtiEnv, monitor: sys::jrawMonitorID) -> sys::jvmtiError {
	assert_eq!(monitor, monitor_id());
	// SAFETY: All monitor calls originate from the live test context.
	unsafe { record(env, "destroy") }
}

unsafe extern "system" fn enter(env: *mut sys::jvmtiEnv, monitor: sys::jrawMonitorID) -> sys::jvmtiError {
	assert_eq!(monitor, monitor_id());
	// SAFETY: All monitor calls originate from the live test context.
	unsafe { record(env, "enter") }
}

unsafe extern "system" fn exit(env: *mut sys::jvmtiEnv, monitor: sys::jrawMonitorID) -> sys::jvmtiError {
	assert_eq!(monitor, monitor_id());
	// SAFETY: All monitor calls originate from the live test context.
	unsafe { record(env, "exit") }
}

unsafe extern "system" fn wait(
	env: *mut sys::jvmtiEnv,
	monitor: sys::jrawMonitorID,
	milliseconds: sys::jlong,
) -> sys::jvmtiError {
	assert_eq!(monitor, monitor_id());
	assert_eq!(milliseconds, 23);
	// SAFETY: All monitor calls originate from the live test context.
	unsafe { record(env, "wait") }
}

unsafe extern "system" fn notify(env: *mut sys::jvmtiEnv, monitor: sys::jrawMonitorID) -> sys::jvmtiError {
	assert_eq!(monitor, monitor_id());
	// SAFETY: All monitor calls originate from the live test context.
	unsafe { record(env, "notify") }
}

unsafe extern "system" fn notify_all(env: *mut sys::jvmtiEnv, monitor: sys::jrawMonitorID) -> sys::jvmtiError {
	assert_eq!(monitor, monitor_id());
	// SAFETY: All monitor calls originate from the live test context.
	unsafe { record(env, "notify_all") }
}

fn table() -> sys::jvmtiInterface_1_ {
	sys::jvmtiInterface_1_ {
		CreateRawMonitor: Some(create),
		DestroyRawMonitor: Some(destroy),
		RawMonitorEnter: Some(enter),
		RawMonitorExit: Some(exit),
		RawMonitorWait: Some(wait),
		RawMonitorNotify: Some(notify),
		RawMonitorNotifyAll: Some(notify_all),
		..Default::default()
	}
}

fn with_table(table: &sys::jvmtiInterface_1_, test: impl FnOnce(&Context, Env<'_>)) {
	let mut context = Context {
		table,
		calls: RefCell::new(Vec::new()),
		fail: Cell::new(""),
	};
	// SAFETY: The initialized native table and context outlive all owners in the closure.
	let env: Env<'_> = unsafe { Env::from_raw_with_version(&raw mut context.table, JvmtiVersion::V1_2) }.expect("env");
	test(&context, env);
}

#[test]
fn recursive_guards_exit_once_before_monitor_destruction() {
	with_table(&table(), |context, env| {
		// SAFETY: Mock operations accept this context; owner and guards remain local.
		unsafe {
			let monitor: RawMonitor<'_, '_> = env.create_raw_monitor(jni_str!("test-monitor")).expect("create");
			assert_eq!(monitor.as_raw(), monitor_id());
			let guard: RawMonitorGuard<'_, '_, '_> = monitor.enter().expect("enter");
			drop(monitor.enter().expect("recursive enter"));
			guard.wait(23).expect("wait");
			guard.notify().expect("notify");
			guard.notify_all().expect("notify all");
			guard.unlock().expect("unlock");
			monitor.destroy().expect("destroy");
		}
		assert_eq!(
			*context.calls.borrow(),
			[
				"create",
				"enter",
				"enter",
				"exit",
				"wait",
				"notify",
				"notify_all",
				"exit",
				"destroy"
			]
		);
	});
}

#[test]
fn transfer_and_adoption_preserve_exclusive_ownership() {
	with_table(&table(), |context, env| {
		// SAFETY: Ownership is transferred once to a replacement owner of the same environment.
		unsafe {
			let raw: sys::jrawMonitorID = env
				.create_raw_monitor(jni_str!("test-monitor"))
				.expect("create")
				.into_raw();
			assert_eq!(*context.calls.borrow(), ["create"]);
			let adopted: RawMonitor<'_, '_> = RawMonitor::from_raw(&env, raw).expect("adopt");
			drop(adopted.enter().expect("enter"));
			drop(adopted);
			assert_eq!(
				RawMonitor::from_raw(&env, ptr::null_mut()).expect_err("null"),
				Error::NullPointer
			);
		}
		assert_eq!(*context.calls.borrow(), ["create", "enter", "exit", "destroy"]);
	});
}

#[test]
fn failures_do_not_double_exit_destroy_or_adopt_failed_outputs() {
	for operation in ["create", "enter", "exit", "destroy", "wait", "notify", "notify_all"] {
		with_table(&table(), |context, env| {
			context.fail.set(operation);
			// SAFETY: Mock failures do not create native resources. Every actual mock owner
			// is cleaned up locally; errors must not manufacture owners or retry cleanup.
			let result: Result<()> = unsafe {
				(|| {
					let monitor: RawMonitor<'_, '_> = env.create_raw_monitor(jni_str!("test-monitor"))?;
					let guard: RawMonitorGuard<'_, '_, '_> = monitor.enter()?;
					guard.wait(23)?;
					guard.notify()?;
					guard.notify_all()?;
					guard.unlock()?;
					monitor.destroy()
				})()
			};
			assert_eq!(result, Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL)));
			let calls: std::cell::Ref<'_, Vec<&str>> = context.calls.borrow();
			assert_eq!(calls.iter().filter(|&&call| call == operation).count(), 1);
			assert_eq!(
				calls.iter().filter(|&&call| call == "destroy").count(),
				usize::from(operation != "create")
			);
			assert_eq!(
				calls.iter().filter(|&&call| call == "exit").count(),
				usize::from(!["create", "enter"].contains(&operation))
			);
		});
	}
}

#[test]
fn cleanup_functions_are_checked_before_acquiring_resources() {
	let mut table: sys::jvmtiInterface_1_ = table();
	table.DestroyRawMonitor = None;
	with_table(&table, |context, env| {
		assert_eq!(
			// SAFETY: No native create is attempted when destruction is unavailable.
			unsafe { env.create_raw_monitor(jni_str!("test-monitor")) }.expect_err("missing cleanup"),
			Error::MissingFunction("DestroyRawMonitor")
		);
		assert!(context.calls.borrow().is_empty());
	});
	table.DestroyRawMonitor = Some(destroy);
	table.RawMonitorExit = None;
	with_table(&table, |context, env| {
		// SAFETY: Monitor owns no entries because enter fails before acquiring the lock.
		unsafe {
			let monitor: RawMonitor<'_, '_> = env.create_raw_monitor(jni_str!("test-monitor")).expect("create");
			assert_eq!(
				monitor.enter().expect_err("missing exit"),
				Error::MissingFunction("RawMonitorExit")
			);
			drop(monitor);
		}
		assert_eq!(*context.calls.borrow(), ["create", "destroy"]);
	});
}
