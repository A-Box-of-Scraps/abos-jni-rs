//! Fault-injected JVMTI outputs with an allocation and cleanup-order oracle.

use std::{
	alloc::{Layout, alloc, dealloc, handle_alloc_error},
	cell::{Cell, RefCell},
	ffi::c_char,
	ptr,
};

use abos_jni::strings::JNIString;
use abos_jvmti::{Env, Error, JvmtiVersion, sys};

use crate::Input;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
	Normal,
	Empty,
	EmptyAllocated,
	Negative,
	NativeError,
	NullOutput,
	NullChild,
}

struct Allocation {
	pointer: *mut u8,
	layout: Layout,
	live: bool,
}

// The first field is the JVMTI environment's function-table pointer. All state
// belongs to this single-threaded invocation, not to a process-global mock.
#[repr(C)]
struct Context {
	table: sys::jvmtiEnv,
	mode: Mode,
	length: usize,
	bad_child: usize,
	optional: u8,
	text: JNIString,
	bytes: Vec<u8>,
	fail_at: usize,
	queries: Cell<usize>,
	allocations: RefCell<Vec<Allocation>>,
	attempts: RefCell<Vec<*mut u8>>,
	invalid_free: Cell<bool>,
}

impl Context {
	fn allocate<T: Copy>(&self, values: &[T]) -> *mut T {
		let layout = Layout::array::<T>(values.len().max(1)).expect("bounded layout");
		// SAFETY: The nonzero layout is retained until matching deallocation.
		let pointer = unsafe { alloc(layout) }.cast::<T>();
		if pointer.is_null() {
			handle_alloc_error(layout);
		}
		// SAFETY: The fresh allocation fits the source and cannot overlap it.
		unsafe { ptr::copy_nonoverlapping(values.as_ptr(), pointer, values.len()) };
		self.allocations.borrow_mut().push(Allocation {
			pointer: pointer.cast(),
			layout,
			live: true,
		});
		pointer
	}

	fn string(&self) -> *mut c_char {
		self.allocate(self.text.as_cstr().to_bytes_with_nul()).cast()
	}

	fn has_elements(&self) -> bool {
		matches!(self.mode, Mode::Normal | Mode::NullChild)
	}

	fn output_length(&self) -> usize {
		if self.has_elements() { self.length } else { 0 }
	}

	unsafe fn output<T: Copy>(&self, values: &[T], count: *mut i32, output: *mut *mut T) -> sys::jvmtiError {
		self.queries.set(self.queries.get() + 1);
		let (length, pointer) = match self.mode {
			Mode::Empty => (0, ptr::null_mut()),
			Mode::EmptyAllocated => (0, self.allocate::<T>(&[])),
			Mode::Negative => (-1, self.allocate::<T>(&[])),
			Mode::NativeError => (i32::MAX, ptr::dangling_mut()),
			Mode::NullOutput => (1, ptr::null_mut()),
			_ => (values.len() as i32, self.allocate(values)),
		};
		// SAFETY: Wrapper-provided output slots are writable, even on native error.
		unsafe {
			*count = length;
			*output = pointer;
		}
		if self.mode == Mode::NativeError {
			sys::JVMTI_ERROR_INTERNAL
		} else {
			sys::JVMTI_ERROR_NONE
		}
	}
}

impl Drop for Context {
	fn drop(&mut self) {
		// Failed cleanup and into_raw intentionally leave allocations live. Reclaim
		// only after checking the oracle, so harness teardown cannot hide leaks.
		for allocation in self.allocations.get_mut() {
			if allocation.live {
				// SAFETY: Live entries retain their original pointer and layout.
				unsafe { dealloc(allocation.pointer, allocation.layout) };
			}
		}
	}
}

unsafe fn context<'a>(env: *mut sys::jvmtiEnv) -> &'a Context {
	// SAFETY: Callbacks are installed only in a live repr(C) Context.
	unsafe { &*env.cast::<Context>() }
}

unsafe extern "system" fn deallocate(env: *mut sys::jvmtiEnv, pointer: *mut u8) -> sys::jvmtiError {
	// SAFETY: This callback receives its live mock environment.
	let context = unsafe { context(env) };
	let mut attempts = context.attempts.borrow_mut();
	attempts.push(pointer);
	let mut allocations = context.allocations.borrow_mut();
	let Some(allocation) = allocations
		.iter_mut()
		.find(|allocation| allocation.pointer == pointer && allocation.live)
	else {
		// Do not panic across the native boundary. Report after returning to Rust.
		context.invalid_free.set(true);
		return sys::JVMTI_ERROR_ILLEGAL_ARGUMENT;
	};
	if attempts.len() == context.fail_at {
		return sys::JVMTI_ERROR_INTERNAL;
	}
	allocation.live = false;
	// SAFETY: The registry verifies ownership and prevents duplicate deallocation.
	unsafe { dealloc(pointer, allocation.layout) };
	sys::JVMTI_ERROR_NONE
}

unsafe extern "system" fn properties(
	env: *mut sys::jvmtiEnv,
	count: *mut i32,
	output: *mut *mut *mut c_char,
) -> sys::jvmtiError {
	// SAFETY: The live context and writable slots come from the wrapper.
	unsafe {
		let context = context(env);
		let values: Vec<_> = (0..context.output_length())
			.map(|index| {
				if context.mode == Mode::NullChild && index == context.bad_child {
					ptr::null_mut()
				} else {
					context.string()
				}
			})
			.collect();
		context.output(&values, count, output)
	}
}

unsafe extern "system" fn locals(
	env: *mut sys::jvmtiEnv,
	_method: sys::jmethodID,
	count: *mut i32,
	output: *mut *mut sys::jvmtiLocalVariableEntry,
) -> sys::jvmtiError {
	// SAFETY: All child allocations are distinct and valid terminated MUTF-8.
	unsafe {
		let context = context(env);
		let values: Vec<_> = (0..context.output_length())
			.map(|index| {
				let invalid = context.mode == Mode::NullChild && index == context.bad_child;
				sys::jvmtiLocalVariableEntry {
					start_location: index as i64,
					length: context.bytes.len() as i32,
					name: if invalid && context.optional & 1 == 0 {
						ptr::null_mut()
					} else {
						context.string()
					},
					signature: if invalid && context.optional & 1 != 0 {
						ptr::null_mut()
					} else {
						context.string()
					},
					generic_signature: if context.optional & (1 << index) == 0 {
						ptr::null_mut()
					} else {
						context.string()
					},
					slot: index as i32,
				}
			})
			.collect();
		context.output(&values, count, output)
	}
}

unsafe extern "system" fn bytecodes(
	env: *mut sys::jvmtiEnv,
	_method: sys::jmethodID,
	count: *mut i32,
	output: *mut *mut u8,
) -> sys::jvmtiError {
	// SAFETY: The context owns the source bytes and wrapper outputs are writable.
	unsafe {
		let context = context(env);
		let values = if context.has_elements() {
			&context.bytes[..context.length]
		} else {
			&[]
		};
		context.output(values, count, output)
	}
}

fn exercise<T>(
	result: abos_jvmti::Result<T>,
	expected_error: Option<Error>,
	context: &Context,
	action: u8,
	inspect: impl FnOnce(&T),
	free: impl FnOnce(T) -> abos_jvmti::Result<()>,
	into_raw: impl FnOnce(T) -> (*mut u8, usize),
) {
	let owner = match (result, expected_error) {
		(Err(actual), Some(expected)) => {
			assert_eq!(actual, expected);
			return;
		}
		(Ok(owner), None) => owner,
		(Err(error), None) => panic!("unexpected error: {error:?}"),
		(Ok(_), Some(error)) => panic!("expected error: {error:?}"),
	};
	inspect(&owner);
	match action {
		0 => drop(owner),
		1 => {
			let fails = context.fail_at != 0 && context.fail_at <= context.allocations.borrow().len();
			assert_eq!(
				free(owner),
				if fails {
					Err(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL))
				} else {
					Ok(())
				}
			);
		}
		_ => {
			let (pointer, length) = into_raw(owner);
			let expected_pointer = context
				.allocations
				.borrow()
				.last()
				.map_or(ptr::null_mut(), |allocation| allocation.pointer);
			assert_eq!(pointer, expected_pointer);
			assert_eq!(length, context.output_length());
		}
	}
}

/// Exercise one bounded output and its drop/free/ownership-transfer path.
pub fn run(data: &[u8]) {
	let mut input = Input(data);
	let kind = input.byte() % 3;
	let mode = match input.byte() % 7 {
		0 => Mode::Normal,
		1 => Mode::Empty,
		2 => Mode::EmptyAllocated,
		3 => Mode::Negative,
		4 => Mode::NativeError,
		5 => Mode::NullOutput,
		_ => Mode::NullChild,
	};
	let action = input.byte() % 3;
	let missing = input.byte() % 3;
	let fail_at = usize::from(input.byte());
	let length = usize::from(input.byte() % 8) + 1;
	let bad_child = usize::from(input.byte()) % length;
	let optional = input.byte();
	let payload = &input.0[..input.0.len().min(128)];
	let mut bytes = payload.to_vec();
	bytes.resize(bytes.len().max(length), 0);
	let mut table = sys::jvmtiInterface_1_ {
		Deallocate: if missing == 1 { None } else { Some(deallocate) },
		GetSystemProperties: Some(properties),
		GetLocalVariableTable: Some(locals),
		GetBytecodes: Some(bytecodes),
		..Default::default()
	};
	let function_name = match kind {
		0 => {
			if missing == 2 {
				table.GetSystemProperties = None;
			}
			"GetSystemProperties"
		}
		1 => {
			if missing == 2 {
				table.GetLocalVariableTable = None;
			}
			"GetLocalVariableTable"
		}
		_ => {
			if missing == 2 {
				table.GetBytecodes = None;
			}
			"GetBytecodes"
		}
	};
	let mut context = Context {
		table: &table,
		mode,
		length,
		bad_child,
		optional,
		text: JNIString::new(String::from_utf8_lossy(payload)),
		bytes,
		fail_at,
		queries: Cell::new(0),
		allocations: RefCell::new(Vec::new()),
		attempts: RefCell::new(Vec::new()),
		invalid_free: Cell::new(false),
	};
	let expected_error = match missing {
		1 => Some(Error::MissingFunction("Deallocate")),
		2 => Some(Error::MissingFunction(function_name)),
		_ => match mode {
			Mode::Negative => Some(Error::InvalidCount(-1)),
			Mode::NativeError => Some(Error::Jvmti(sys::JVMTI_ERROR_INTERNAL)),
			Mode::NullOutput => Some(Error::NullPointer),
			Mode::NullChild if kind != 2 => Some(Error::NullPointer),
			_ => None,
		},
	};
	// SAFETY: This mock implements the baseline table for the lifetime of env;
	// its callbacks need no JVM, attachment, or real method ID. Successful outputs
	// have bounded initialized allocations, except explicitly handled error shapes.
	unsafe {
		let env = Env::from_raw_with_version(&raw mut context.table, JvmtiVersion::V1_2).expect("mock env");
		match kind {
			0 => exercise(
				env.system_properties(),
				expected_error,
				&context,
				action,
				|owner| {
					assert_eq!(owner.len(), context.output_length());
					assert_eq!(owner.is_empty(), context.output_length() == 0);
					assert_eq!(owner.iter().len(), owner.len());
					for value in owner.iter().rev() {
						assert_eq!(value.to_bytes(), context.text.as_cstr().to_bytes());
					}
				},
				|owner| owner.free(),
				|owner| {
					let (p, n) = owner.into_raw();
					(p.cast(), n)
				},
			),
			1 => exercise(
				env.local_variable_table(ptr::null_mut()),
				expected_error,
				&context,
				action,
				|owner| {
					assert_eq!(owner.len(), context.output_length());
					assert_eq!(owner.is_empty(), context.output_length() == 0);
					assert_eq!(owner.iter().len(), owner.len());
					for (index, entry) in owner.iter().enumerate().rev() {
						assert_eq!(entry.start_location, index as i64);
						assert_eq!(entry.length, context.bytes.len() as i32);
						assert_eq!(entry.slot, index as i32);
						assert_eq!(entry.name.to_bytes(), context.text.as_cstr().to_bytes());
						assert_eq!(entry.signature.to_bytes(), context.text.as_cstr().to_bytes());
						assert_eq!(entry.generic_signature.is_some(), optional & (1 << index) != 0);
						if let Some(value) = entry.generic_signature {
							assert_eq!(value.to_bytes(), context.text.as_cstr().to_bytes());
						}
					}
				},
				|owner| owner.free(),
				|owner| {
					let (p, n) = owner.into_raw();
					(p.cast(), n)
				},
			),
			_ => exercise(
				env.bytecodes(ptr::null_mut()),
				expected_error,
				&context,
				action,
				|owner| assert_eq!(&**owner, &context.bytes[..context.output_length()]),
				|owner| owner.free(),
				|owner| owner.into_raw(),
			),
		}
	}
	assert_eq!(context.queries.get(), usize::from(missing == 0));
	assert!(!context.invalid_free.get(), "unknown or duplicate deallocation");
	let allocations = context.allocations.borrow();
	let attempts = context.attempts.borrow();
	let transferred = expected_error.is_none() && action == 2;
	if transferred {
		assert!(attempts.is_empty(), "into_raw must transfer all allocations");
	} else {
		// Allocations are created child-first and outer-last. Comparing the entire
		// sequence catches skipped siblings, wrong order, and retries after errors.
		assert_eq!(
			*attempts,
			allocations
				.iter()
				.map(|allocation| allocation.pointer)
				.collect::<Vec<_>>()
		);
	}
	for (index, allocation) in allocations.iter().enumerate() {
		assert_eq!(
			allocation.live,
			transferred || fail_at == index + 1,
			"unexpected leak or cleanup retry"
		);
	}
}
