#![allow(unsafe_code, reason = "Exercise JNI FFI and native method exports.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]
//! This example demonstrates the key features of the `native_method!` macro
//!
//! The `native_method!` macro creates compile-time type-checked `NativeMethod` descriptors
//! that can be registered with the JVM.
//!
//! Run with: `cargo run --example native_method`

use abos_jni::errors::LogErrorAndDefault;
use abos_jni::objects::{JClass, JIntArray, JObject, JString};
use abos_jni::sys::jint;
use abos_jni::{Env, EnvUnowned, NativeMethod, jni_sig, jni_str, native_method};

#[path = "utils/lib.rs"]
mod utils;

struct RustThing {
	pub message: String,
}

// Illustrative pointer-as-long handle, not a production ownership design.
#[repr(transparent)]
#[derive(Copy, Clone)]
struct ThingHandle(*const RustThing);
impl ThingHandle {
	pub fn new(thing: RustThing) -> Self {
		let boxed: Box<RustThing> = Box::new(thing);
		Self(Box::into_raw(boxed))
	}

	const unsafe fn as_ref(&self) -> &RustThing {
		// SAFETY: The caller guarantees this handle points to a live RustThing for the borrow.
		unsafe { &*self.0 }
	}

	// Safety: only convert back to Box (to drop) when sure handle is no longer shared.
	pub unsafe fn into_box(self) -> Box<RustThing> {
		// SAFETY: The caller guarantees exclusive ownership of the pointer originally allocated by Box.
		unsafe { Box::from_raw(self.0.cast_mut()) }
	}
}
impl From<ThingHandle> for abos_jni::sys::jlong {
	fn from(handle: ThingHandle) -> Self {
		handle.0 as Self
	}
}

// extern with java_type exports a mangled JNI symbol for JVM lookup.
// This executable is not a JVM-loaded shared library, so it registers the methods explicitly.
const NATIVE_METHODS: &[NativeMethod] = &[
	// Instance methods - shorthand syntax
	// Without a `rust_type` the binding will use `JObject` for 'this'
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		extern fn native_add(a: jint, b: jint) -> jint,
	},
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		extern fn native_concatenate(a: JString, b: JString) -> JString,
	},
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		type_map = {
			unsafe ThingHandle => long,
		},
		extern fn native_process_handle(handle: ThingHandle) -> JString,
	},
	// Raw native method (no catch_unwind, receives EnvUnowned directly)
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		raw extern fn native_raw(value: jint) -> jint,
	},
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		extern fn native_with_function(value: jint) -> jint,
		fn = native_with_function_impl,
	},
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		extern fn native_no_unwind(value: jint) -> jint,
		fn = native_no_unwind,
		catch_unwind = false,
	},
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		extern fn native_custom_error_policy(value: jint) -> jint,
		fn = native_custom_error_policy,
		error_policy = LogErrorAndDefault,
	},
	// Static methods
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		static extern fn native_greet(name: JString) -> JString,
	},
	native_method! {
		java_type = "com.example.NativeMethodOverview",
		static extern fn native_echo_int_array(arr: jint[]) -> jint[],
	},
];

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn native_add<'local>(
	_env: &mut Env<'local>,
	_this: JObject<'local>,
	a: jint,
	b: jint,
) -> Result<jint, abos_jni::errors::Error> {
	Ok(a + b)
}

#[expect(
	clippy::needless_pass_by_value,
	reason = "The native-method wrapper requires JNI reference arguments by value."
)]
fn native_concatenate<'local>(
	env: &mut Env<'local>,
	_this: JObject<'local>,
	a: JString<'local>,
	b: JString<'local>,
) -> Result<JString<'local>, abos_jni::errors::Error> {
	let a_str: String = a.try_to_string(env)?;
	let b_str: String = b.try_to_string(env)?;
	JString::from_str(env, format!("{a_str}{b_str}"))
}

fn native_process_handle<'local>(
	env: &mut Env<'local>,
	_this: JObject<'local>,
	handle: ThingHandle,
) -> Result<JString<'local>, abos_jni::errors::Error> {
	// SAFETY: The example Java caller returns the handle allocated by Rust without freeing or changing it.
	let thing_ref: &RustThing = unsafe { handle.as_ref() };
	let response: String = format!("RustThing says: {}", thing_ref.message);
	JString::from_str(env, response)
}

const fn native_raw<'local>(_unowned_env: EnvUnowned<'local>, _this: JObject<'local>, value: jint) -> jint {
	value * 2
}

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn native_with_function_impl<'local>(
	_env: &mut Env<'local>,
	_this: JObject<'local>,
	value: jint,
) -> Result<jint, abos_jni::errors::Error> {
	Ok(value * 3)
}

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn native_no_unwind<'local>(
	_env: &mut Env<'local>,
	_this: JObject<'local>,
	value: jint,
) -> Result<jint, abos_jni::errors::Error> {
	Ok(value - 1)
}

const fn native_custom_error_policy<'local>(
	_env: &mut Env<'local>,
	_this: JObject<'local>,
	value: jint,
) -> Result<jint, abos_jni::errors::Error> {
	if value < 0 {
		Err(abos_jni::errors::Error::JniCall(abos_jni::errors::JniError::Unknown))
	} else {
		Ok(value)
	}
}

#[expect(
	clippy::needless_pass_by_value,
	reason = "The native-method wrapper requires JNI reference arguments by value."
)]
fn native_greet<'local>(
	env: &mut Env<'local>,
	_class: JClass<'local>,
	name: JString<'local>,
) -> Result<JString<'local>, abos_jni::errors::Error> {
	let name_str: String = name.try_to_string(env)?;
	JString::from_str(env, format!("Hello, {name_str}!"))
}

#[expect(
	clippy::unnecessary_wraps,
	reason = "The native-method wrapper requires a Result-returning callback."
)]
const fn native_echo_int_array<'local>(
	_env: &mut Env<'local>,
	_class: JClass<'local>,
	arr: JIntArray<'local>,
) -> Result<JIntArray<'local>, abos_jni::errors::Error> {
	Ok(arr)
}

fn main() {
	utils::attach_current_thread(|env| {
		utils::load_class(env, "NativeMethodOverview")?;

		println!("=== native_method! Overview Example ===\n");

		println!("--- Registering Native Methods ---");
		let class: JClass<'_> = env.find_class(jni_str!("com/example/NativeMethodOverview"))?;
		// SAFETY: The fixture class declares the names and JNI signatures supplied by these native-method descriptors.
		unsafe {
			env.register_native_methods(&class, NATIVE_METHODS)?;
		}
		println!("Registered {} native methods", NATIVE_METHODS.len());

		println!("\n--- Creating Instance ---");
		let obj: JObject<'_> = env.new_object(&class, &jni_sig!(()->void), &[])?;
		println!("Created NativeMethodOverview instance");

		println!("\n--- Testing Native Methods ---");
		let test_method: abos_jni::ids::JMethodID =
			env.get_method_id(&class, jni_str!("testAllNativeMethods"), &jni_sig!(()->JString))?;
		// SAFETY: testAllNativeMethods has the looked-up no-argument signature returning String.
		let result: JObject<'_> = unsafe {
			env.call_method_unchecked(&obj, test_method, abos_jni::signature::ReturnType::Object, &[])?
				.l()?
		};
		let result: JString<'_> = env.cast_local::<JString>(result)?;
		println!("Test results: {}", result.try_to_string(env)?);

		println!("\n--- Testing Handle Method ---");
		let thing = RustThing {
			message: "Hello from Rust!".to_string(),
		};
		let handle = ThingHandle::new(thing);

		let handle_value = abos_jni::sys::jvalue { j: handle.into() };
		let handle_method: abos_jni::ids::JMethodID =
			env.get_method_id(&class, jni_str!("nativeProcessHandle"), &jni_sig!((jlong)->JString))?;
		// SAFETY: nativeProcessHandle takes the live native handle as a long and returns String.
		let response: JObject<'_> = unsafe {
			env.call_method_unchecked(
				&obj,
				handle_method,
				abos_jni::signature::ReturnType::Object,
				&[handle_value],
			)?
			.l()?
		};
		let response: JString<'_> = env.cast_local::<JString>(response)?;
		println!("nativeProcessHandle response: {}", response.try_to_string(env)?);

		// Safety: we are done with the handle, so convert back to Box to drop
		unsafe {
			drop(handle.into_box());
		}

		println!("\n=== Example completed successfully ===");
		Ok(())
	})
	.expect("Failed to run example");
}
