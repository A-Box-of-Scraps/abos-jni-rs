use std::ptr;

use abos_jni::jni_str;
use abos_jvmti::{Error, JvmtiStringList, LocalVariable, LocalVariableTable, Result, sys};

use super::support::{Mode, with_env};

#[test]
fn string_list_frees_children_before_the_outer_array() {
	with_env(|context, env| {
		// SAFETY: The mock context supplies separate owned strings and remains alive.
		let strings: JvmtiStringList<'_, '_> = unsafe { env.system_properties() }.expect("properties");
		assert_eq!(strings.len(), 2);
		assert!(!strings.is_empty());
		assert_eq!(
			strings.iter().map(|s| s.to_str()).collect::<Vec<_>>(),
			["java.vm.name", "java.version"]
		);
		let children: Vec<usize> = strings.iter().map(|s| s.as_ptr() as usize).collect();
		strings.free().expect("free children and outer");
		assert_eq!(&context.calls.borrow()[..2], &children);
		assert_eq!(context.calls.borrow().len(), 3);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn nested_raw_transfer_does_not_free_any_allocation() {
	with_env(|context, env| {
		// SAFETY: All allocations stay in the mock's valid context until manually freed.
		unsafe {
			let (pointer, length): (*mut *mut std::ffi::c_char, usize) =
				env.system_properties().expect("list").into_raw();
			assert_eq!(length, 2);
			assert!(context.calls.borrow().is_empty());
			for &child in std::slice::from_raw_parts(pointer, length) {
				assert_eq!(
					super::support::deallocate(env.as_raw(), child.cast()),
					sys::JVMTI_ERROR_NONE
				);
			}
			assert_eq!(
				super::support::deallocate(env.as_raw(), pointer.cast()),
				sys::JVMTI_ERROR_NONE
			);
		}
		assert_eq!(context.calls.borrow().len(), 3);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn local_variables_borrow_strings_and_free_optional_fields() {
	with_env(|context, env| {
		// SAFETY: The mock ignores the method and supplies valid table allocations.
		let table: LocalVariableTable<'_, '_> = unsafe { env.local_variable_table(ptr::null_mut()) }.expect("locals");
		assert_eq!(table.len(), 2);
		assert!(!table.is_empty());
		let first: LocalVariable<'_> = table.iter().next().expect("first");
		assert_eq!((first.start_location, first.length, first.slot), (3, 8, 0));
		assert_eq!(first.name, jni_str!("x"));
		assert_eq!(first.signature, jni_str!("Ljava/util/List;"));
		assert!(first.generic_signature.is_none());
		assert_eq!(
			table.iter().next_back().expect("last").generic_signature,
			Some(jni_str!("Ljava/util/List<Ljava/lang/String;>;"))
		);
		let children: Vec<usize> = table
			.iter()
			.flat_map(|entry| [Some(entry.name), Some(entry.signature), entry.generic_signature])
			.flatten()
			.map(|s| s.as_ptr() as usize)
			.collect();
		drop(table);
		assert_eq!(&context.calls.borrow()[..5], &children);
		assert_eq!(context.calls.borrow().len(), 6);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn local_table_raw_transfer_preserves_every_nested_pointer() {
	with_env(|context, env| {
		// SAFETY: All transferred allocations are freed before their environment goes away.
		unsafe {
			let (pointer, length): (*mut sys::jvmtiLocalVariableEntry, usize) =
				env.local_variable_table(ptr::null_mut()).expect("table").into_raw();
			assert_eq!(length, 2);
			assert!(context.calls.borrow().is_empty());
			for entry in std::slice::from_raw_parts(pointer, length) {
				for child in [entry.name, entry.signature, entry.generic_signature] {
					if !child.is_null() {
						assert_eq!(
							super::support::deallocate(env.as_raw(), child.cast()),
							sys::JVMTI_ERROR_NONE
						);
					}
				}
			}
			assert_eq!(
				super::support::deallocate(env.as_raw(), pointer.cast()),
				sys::JVMTI_ERROR_NONE
			);
		}
		assert_eq!(context.calls.borrow().len(), 6);
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn partial_validation_failures_release_all_initialized_children() {
	with_env(|context, env| {
		context.mode.set(Mode::NullChild);
		// SAFETY: All non-null children are valid allocations; one required field is deliberately missing.
		unsafe {
			assert_eq!(env.system_properties().expect_err("null child"), Error::NullPointer);
			assert_eq!(context.calls.borrow().len(), 2);
			assert_eq!(
				env.local_variable_table(ptr::null_mut()).expect_err("null signature"),
				Error::NullPointer
			);
			assert_eq!(context.calls.borrow().len(), 7);
		}
		assert_eq!(context.live(), 0);
	});
}

#[test]
fn cleanup_failure_does_not_skip_siblings_or_outer_allocation() {
	for failed_call in 1..=6 {
		with_env(|context, env| {
			context.fail_at.set(Some(failed_call));
			// SAFETY: Mock returns valid nested allocations and injects one deallocation error.
			let table: LocalVariableTable<'_, '_> =
				unsafe { env.local_variable_table(ptr::null_mut()) }.expect("table");
			assert_eq!(table.free(), Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL)));
			assert_eq!(context.calls.borrow().len(), 6);
			assert_eq!(context.live(), 1);
		});
	}
	for failed_call in 1..=3 {
		with_env(|context, env| {
			context.fail_at.set(Some(failed_call));
			// SAFETY: Mock context and strings are valid until cleanup finishes.
			let strings: JvmtiStringList<'_, '_> = unsafe { env.system_properties() }.expect("strings");
			assert_eq!(strings.free(), Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL)));
			assert_eq!(context.calls.borrow().len(), 3);
			assert_eq!(context.live(), 1);
		});
	}
}

#[test]
fn empty_nested_results_and_native_errors_never_read_invalid_outputs() {
	for mode in [
		Mode::Empty,
		Mode::EmptyAllocated,
		Mode::NativeError,
		Mode::Negative,
		Mode::NullOutput,
	] {
		with_env(|context, env| {
			context.mode.set(mode);
			// SAFETY: Mocks supply the selected edge case; error outputs are deliberately dangling.
			unsafe {
				let strings: Result<JvmtiStringList<'_, '_>> = env.system_properties();
				let table: Result<LocalVariableTable<'_, '_>> = env.local_variable_table(ptr::null_mut());
				match mode {
					Mode::Empty | Mode::EmptyAllocated => {
						let strings: JvmtiStringList<'_, '_> = strings.expect("empty strings");
						let table: LocalVariableTable<'_, '_> = table.expect("empty table");
						assert!(strings.is_empty() && table.is_empty());
						assert_eq!(strings.iter().count(), 0);
						assert_eq!(table.iter().count(), 0);
					}
					_ => {
						let error: Error = match mode {
							Mode::Negative => Error::InvalidCount(-1),
							Mode::NullOutput => Error::NullPointer,
							_ => Error::Jvmti(sys::JVMTI_ERROR_OUT_OF_MEMORY),
						};
						assert_eq!(strings.expect_err("invalid result"), error);
						assert_eq!(table.expect_err("invalid result"), error);
					}
				}
			}
			assert_eq!(
				context.calls.borrow().len(),
				if matches!(mode, Mode::EmptyAllocated | Mode::Negative) {
					2
				} else {
					0
				}
			);
			assert_eq!(context.live(), 0);
		});
	}
}

#[test]
fn string_errors_do_not_inspect_undefined_output() {
	for (mode, error) in [
		(Mode::NativeError, Error::Jvmti(sys::JVMTI_ERROR_NOT_AVAILABLE)),
		(Mode::NullOutput, Error::NullPointer),
	] {
		with_env(|context, env| {
			context.mode.set(mode);
			assert_eq!(
				// SAFETY: The mock intentionally returns null or a dangling pointer on error.
				unsafe { env.system_property(jni_str!("key")) }.expect_err("property error"),
				error
			);
			assert_eq!(context.live(), 0);
			assert!(context.calls.borrow().is_empty());
		});
	}
}
