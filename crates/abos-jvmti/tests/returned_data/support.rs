use std::{
	alloc::{Layout, alloc, dealloc},
	cell::{Cell, RefCell},
	collections::HashMap,
	ffi::c_char,
	ptr,
};

use abos_jvmti::{Env, JvmtiVersion, sys};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
	Normal,
	Empty,
	EmptyAllocated,
	Negative,
	NativeError,
	NullOutput,
	NullChild,
}

#[repr(C)]
pub struct Context {
	table: sys::jvmtiEnv,
	pub mode: Cell<Mode>,
	pub calls: RefCell<Vec<usize>>,
	pub fail_at: Cell<Option<usize>>,
	pub invalid_free: Cell<bool>,
	allocations: RefCell<HashMap<usize, Layout>>,
}

impl Context {
	pub fn allocate<T: Copy>(&self, values: &[T]) -> *mut T {
		let layout: Layout = Layout::array::<T>(values.len().max(1)).expect("layout");
		// SAFETY: Layout is nonzero and the test registry retains it for matching deallocation.
		let pointer: *mut T = unsafe { alloc(layout) }.cast();
		assert!(!pointer.is_null());
		// SAFETY: This new allocation has room and alignment for the source values.
		unsafe { ptr::copy_nonoverlapping(values.as_ptr(), pointer, values.len()) };
		self.allocations.borrow_mut().insert(pointer as usize, layout);
		pointer
	}

	pub fn live(&self) -> usize {
		self.allocations.borrow().len()
	}
}

impl Drop for Context {
	fn drop(&mut self) {
		assert!(!self.invalid_free.get(), "wrong environment or double deallocation");
		// Reclaim intentionally failed deallocations after tests inspect the native error.
		for (pointer, layout) in self.allocations.get_mut().drain() {
			// SAFETY: These allocations remain exclusively owned by the mock allocator.
			unsafe { dealloc(pointer as *mut u8, layout) };
		}
	}
}

pub fn table() -> sys::jvmtiInterface_1_ {
	sys::jvmtiInterface_1_ {
		Deallocate: Some(deallocate),
		GetSystemProperties: Some(properties),
		GetSystemProperty: Some(property),
		GetLineNumberTable: Some(lines),
		GetLocalVariableTable: Some(locals),
		..Default::default()
	}
}

pub fn with_env(test: impl FnOnce(&Context, Env<'_>)) {
	with_table(&table(), test);
}

pub fn with_table(table: &sys::jvmtiInterface_1_, test: impl FnOnce(&Context, Env<'_>)) {
	let mut context = Context {
		table,
		mode: Cell::new(Mode::Normal),
		calls: RefCell::new(Vec::new()),
		fail_at: Cell::new(None),
		invalid_free: Cell::new(false),
		allocations: RefCell::new(HashMap::new()),
	};
	// SAFETY: The initialized table and repr(C) context outlive the closure.
	let env: Env<'_> = unsafe { Env::from_raw_with_version(&raw mut context.table, JvmtiVersion::V1_2) }.expect("env");
	test(&context, env);
}

unsafe fn context<'a>(env: *mut sys::jvmtiEnv) -> &'a Context {
	// SAFETY: All mock calls use the live repr(C) context with table as its first field.
	unsafe { &*env.cast::<Context>() }
}

pub unsafe extern "system" fn deallocate(env: *mut sys::jvmtiEnv, pointer: *mut u8) -> sys::jvmtiError {
	// SAFETY: This function is only installed in with_table's context.
	let context: &Context = unsafe { context(env) };
	context.calls.borrow_mut().push(pointer as usize);
	if context.fail_at.get() == Some(context.calls.borrow().len()) {
		return sys::JVMTI_ERROR_INTERNAL;
	}
	let Some(layout) = context.allocations.borrow_mut().remove(&(pointer as usize)) else {
		context.invalid_free.set(true);
		return sys::JVMTI_ERROR_ILLEGAL_ARGUMENT;
	};
	// SAFETY: The pointer and layout come from this mock allocator; removal prevents reuse.
	unsafe { dealloc(pointer, layout) };
	sys::JVMTI_ERROR_NONE
}

unsafe fn output<T: Copy>(
	context: &Context,
	values: &[T],
	count: *mut sys::jint,
	pointer: *mut *mut T,
) -> sys::jvmtiError {
	let (length, raw): (sys::jint, *mut T) = match context.mode.get() {
		Mode::Empty => (0, ptr::null_mut()),
		Mode::EmptyAllocated => (0, context.allocate::<T>(&[])),
		Mode::Negative => (-1, context.allocate::<T>(&[])),
		Mode::NativeError => (i32::MAX, ptr::dangling_mut()),
		Mode::NullOutput => (1, ptr::null_mut()),
		_ => (
			sys::jint::try_from(values.len()).expect("small test slice"),
			context.allocate(values),
		),
	};
	// SAFETY: The wrapper supplies writable output slots, even on error.
	unsafe {
		*count = length;
		*pointer = raw;
	}
	if context.mode.get() == Mode::NativeError {
		sys::JVMTI_ERROR_OUT_OF_MEMORY
	} else {
		sys::JVMTI_ERROR_NONE
	}
}

unsafe extern "system" fn properties(
	env: *mut sys::jvmtiEnv,
	count: *mut sys::jint,
	pointer: *mut *mut *mut c_char,
) -> sys::jvmtiError {
	// SAFETY: All parameters come from the wrapper and the live mock context.
	unsafe {
		let context: &Context = context(env);
		let mut strings: Vec<*mut c_char> = Vec::new();
		if matches!(context.mode.get(), Mode::Normal | Mode::NullChild) {
			strings.push(context.allocate(b"java.vm.name\0").cast());
			strings.push(if context.mode.get() == Mode::NullChild {
				ptr::null_mut()
			} else {
				context.allocate(b"java.version\0").cast()
			});
		}
		output(context, &strings, count, pointer)
	}
}

unsafe extern "system" fn property(
	env: *mut sys::jvmtiEnv,
	_key: *const c_char,
	pointer: *mut *mut c_char,
) -> sys::jvmtiError {
	// SAFETY: The live context owns the string and the wrapper provides a writable output.
	unsafe {
		let context: &Context = context(env);
		*pointer = match context.mode.get() {
			Mode::NullOutput => ptr::null_mut(),
			Mode::NativeError => ptr::dangling_mut(),
			_ => context.allocate(b"value\xc0\x80\xed\xa0\xbd\xed\xb8\x80\0").cast(),
		};
		if context.mode.get() == Mode::NativeError {
			sys::JVMTI_ERROR_NOT_AVAILABLE
		} else {
			sys::JVMTI_ERROR_NONE
		}
	}
}

unsafe extern "system" fn lines(
	env: *mut sys::jvmtiEnv,
	_method: sys::jmethodID,
	count: *mut sys::jint,
	pointer: *mut *mut sys::jvmtiLineNumberEntry,
) -> sys::jvmtiError {
	// SAFETY: The live context and writable outputs come from the wrapper.
	unsafe {
		output(
			context(env),
			&[sys::jvmtiLineNumberEntry {
				start_location: 3,
				line_number: 7,
			}],
			count,
			pointer,
		)
	}
}

unsafe extern "system" fn locals(
	env: *mut sys::jvmtiEnv,
	_method: sys::jmethodID,
	count: *mut sys::jint,
	pointer: *mut *mut sys::jvmtiLocalVariableEntry,
) -> sys::jvmtiError {
	// SAFETY: The live context owns all allocations; the wrapper provides writable outputs.
	unsafe {
		let context: &Context = context(env);
		let mut entries: Vec<sys::jvmtiLocalVariableEntry> = Vec::new();
		if matches!(context.mode.get(), Mode::Normal | Mode::NullChild) {
			for slot in 0..2 {
				entries.push(sys::jvmtiLocalVariableEntry {
					start_location: 3,
					length: 8,
					name: context.allocate(b"x\0").cast(),
					signature: if slot == 1 && context.mode.get() == Mode::NullChild {
						ptr::null_mut()
					} else {
						context.allocate(b"Ljava/util/List;\0").cast()
					},
					generic_signature: if slot == 0 {
						ptr::null_mut()
					} else {
						context.allocate(b"Ljava/util/List<Ljava/lang/String;>;\0").cast()
					},
					slot,
				});
			}
		}
		output(context, &entries, count, pointer)
	}
}
