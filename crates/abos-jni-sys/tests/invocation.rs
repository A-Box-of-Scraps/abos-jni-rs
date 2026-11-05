#![allow(
	unsafe_code,
	reason = "This integration test exercises raw JNI pointers and JVM entry points."
)]
#![allow(missing_docs, reason = "Integration test crate with no public API.")]

use std::{env, ffi::c_void, path::PathBuf, ptr};

use abos_jni_sys as sys;
use libloading::{Library, Symbol};

type CreateJavaVm = unsafe extern "system" fn(*mut *mut sys::JavaVM, *mut *mut c_void, *mut c_void) -> sys::jint;

#[test]
#[ignore = "requires JAVA_HOME pointing to a host JDK"]
fn invocation_and_native_calls() {
	let home: PathBuf = PathBuf::from(env::var_os("JAVA_HOME").expect("JAVA_HOME must point to a host JDK"));
	let library_path: PathBuf = if cfg!(target_os = "windows") {
		home.join("bin/server/jvm.dll")
	} else if cfg!(target_os = "macos") {
		home.join("lib/server/libjvm.dylib")
	} else {
		home.join("lib/server/libjvm.so")
	};
	// SAFETY: JAVA_HOME selects a trusted host JVM library whose initialization is valid here.
	// Keep it loaded until process exit because the JVM can retain state after DestroyJavaVM.
	let library: &Library = Box::leak(Box::new(unsafe {
		Library::new(&library_path).expect("load the host JVM")
	}));
	// SAFETY: The entry-point signature matches jni.h, and initialization arguments and output
	// pointers remain valid for the call. After successful creation, table accesses use the live
	// VM on its creating thread; no JNI calls are made after destruction.
	unsafe {
		let create: Symbol<'_, CreateJavaVm> = library.get(b"JNI_CreateJavaVM\0").expect("JNI_CreateJavaVM");
		let mut option = sys::JavaVMOption {
			optionString: c"-Xcheck:jni".as_ptr().cast_mut(),
			extraInfo: ptr::null_mut(),
		};
		let mut args = sys::JavaVMInitArgs {
			version: sys::JNI_VERSION_1_8,
			nOptions: 1,
			options: &raw mut option,
			ignoreUnrecognized: sys::JNI_FALSE,
		};
		let mut vm: *mut sys::JavaVM = ptr::null_mut();
		let mut env: *mut sys::JNIEnv = ptr::null_mut();
		assert_eq!(
			create(&raw mut vm, (&raw mut env).cast(), (&raw mut args).cast()),
			sys::JNI_OK
		);
		assert!(!vm.is_null());
		assert!(!env.is_null());
		let guard: JavaVm = JavaVm(vm);
		let mut attached: *mut c_void = ptr::null_mut();
		assert_eq!(
			((**vm).v1_2.GetEnv)(vm, &raw mut attached, sys::JNI_VERSION_1_8),
			sys::JNI_OK
		);
		assert_eq!(attached, env.cast());
		assert_eq!(
			((**vm).v1_1.AttachCurrentThread)(vm, &raw mut attached, ptr::null_mut()),
			sys::JNI_OK
		);
		let value: sys::jint = integer_roundtrip(env);
		assert_eq!(((**vm).v1_1.DetachCurrentThread)(vm), sys::JNI_OK);
		let destroyed: sys::jint = ((**vm).v1_1.DestroyJavaVM)(vm);
		std::mem::forget(guard);
		assert_eq!(destroyed, sys::JNI_OK);
		assert_eq!(value, 42);
	}
}

// Ensure a failed assertion still tears down the VM before the test process exits.
struct JavaVm(*mut sys::JavaVM);

impl Drop for JavaVm {
	fn drop(&mut self) {
		// SAFETY: This guard owns a successfully created VM and is forgotten after explicit
		// destruction. Otherwise it destroys the still-live VM on the creating thread.
		unsafe {
			((**self.0).v1_1.DestroyJavaVM)(self.0);
		}
	}
}

unsafe fn assert_no_exception(env: *mut sys::JNIEnv) {
	// SAFETY: The caller supplies the live JNI environment for the current attached thread.
	// Exception inspection and clearing do not invalidate that environment.
	unsafe {
		if ((**env).v1_2.ExceptionCheck)(env) != sys::JNI_FALSE {
			((**env).v1_1.ExceptionDescribe)(env);
			((**env).v1_1.ExceptionClear)(env);
			panic!("unexpected Java exception");
		}
	}
}

unsafe fn integer_roundtrip(env: *mut sys::JNIEnv) -> sys::jint {
	// SAFETY: The caller supplies the current thread's live JNI environment. Class and method
	// lookups are checked for null, signatures match java.lang.Integer, and argument storage
	// lives through each call. Local references are deleted only after their last use.
	unsafe {
		let api: &sys::JNINativeInterface__1_1 = &(**env).v1_1;
		assert!((api.GetVersion)(env) >= sys::JNI_VERSION_1_8);
		let class: sys::jclass = (api.FindClass)(env, c"java/lang/Integer".as_ptr());
		assert!(!class.is_null());
		let constructor: sys::jmethodID = (api.GetMethodID)(env, class, c"<init>".as_ptr(), c"(I)V".as_ptr());
		assert!(!constructor.is_null());
		let values = [sys::jvalue { i: 42 }];
		let object: sys::jobject = (api.NewObjectA)(env, class, constructor, values.as_ptr());
		assert_no_exception(env);
		assert!(!object.is_null());
		let method: sys::jmethodID = (api.GetMethodID)(env, class, c"intValue".as_ptr(), c"()I".as_ptr());
		assert!(!method.is_null());
		let value: sys::jint = (api.CallIntMethodA)(env, object, method, ptr::null());
		assert_no_exception(env);
		(api.DeleteLocalRef)(env, object);
		(api.DeleteLocalRef)(env, class);
		value
	}
}
