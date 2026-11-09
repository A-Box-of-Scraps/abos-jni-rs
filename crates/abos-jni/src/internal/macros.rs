//! Low-level JNI calls with exception and null checks.
//!
//! Macro-local `return` and `?` must not return from the caller's function.

/// Call an exception-safe `Env` FFI function without additional checks.
///
/// # Safety
///
/// This may only be used with a JNI function that is considered to be safe
/// to call with a pending exception. From the JNI design spec, this includes:
///
/// - `ExceptionOccurred()`
/// - `ExceptionDescribe()`
/// - `ExceptionClear()`
/// - `ExceptionCheck()`
/// - `ReleaseStringChars()`
/// - `ReleaseStringUTFChars()`
/// - `ReleaseStringCritical()`
/// - `Release<Type>ArrayElements()`
/// - `ReleasePrimitiveArrayCritical()`
/// - `DeleteLocalRef()`
/// - `DeleteGlobalRef()`
/// - `DeleteWeakGlobalRef()`
/// - `MonitorExit()`
/// - `PushLocalFrame()`
/// - `PopLocalFrame()`
/// - `DetachCurrentThread()`
///
/// Arguments must satisfy the JNI function's requirements, and the function must be
/// available in the current JNI version.
macro_rules! ex_safe_jni_call_no_post_check_ex {
    ( $jnienv:expr, $version:tt, $name:ident $(, $args:expr )*) => {{
        // SAFETY: `Env::from_raw()` checks that the environment pointer is non-null.
        let env: *mut abos_jni_sys::JNIEnv = $jnienv.get_raw();
        let interface: *const abos_jni_sys::JNINativeInterface_ = *env;

        ((*interface).$version.$name)(env $(, $args)*)
    }};
}

/// Evaluates arguments, checks for a pending exception, then calls an Env FFI function
///
/// Returns `Err(Error::JavaException)` without calling JNI if an exception is pending.
/// This prevents calling exception-unsafe functions with a pending exception.
///
/// Arguments are evaluated exactly once before this check, since user-defined
/// conversions may raise Java exceptions. Argument temporaries remain alive
/// until after the call.
///
/// # Safety
///
/// Arguments must satisfy the JNI function's requirements, and the function must be
/// available in the current JNI version.
macro_rules! jni_call_no_post_check_ex {
    ( $jnienv:expr, $version:tt, $name:ident $(, $args:expr )*) => {{
        jni_call_no_post_check_ex!(@bind $jnienv, $version, $name, [] ; $($args),*)
    }};
    // Bind each expression once before checking exceptions. Match scrutinees keep
    // argument temporaries alive through the JNI call, including owners of raw pointers.
    (@bind $jnienv:expr, $version:tt, $name:ident, [$($bound:ident,)*] ; $arg:expr $(, $rest:expr)*) => {{
        match $arg {
            arg => jni_call_no_post_check_ex!(@bind $jnienv, $version, $name, [$($bound,)* arg,] ; $($rest),*)
        }
    }};
    (@bind $jnienv:expr, $version:tt, $name:ident, [$($bound:ident,)*] ;) => {{
        let jnienv = &$jnienv;
        $crate::__must_use(if jnienv.exception_check() {
            Err(crate::errors::Error::JavaException)
        } else {
            // SAFETY: `Env::from_raw()` checks that the environment pointer is non-null.
            let env: *mut abos_jni_sys::JNIEnv = jnienv.get_raw();
            let interface: *const abos_jni_sys::JNINativeInterface_ = *env;

            Ok(((*interface).$version.$name)(env $(, $bound)*))
        })
    }};
}

/// Call an `Env` function with exception checks before and after the call.
///
/// Returns `Err(Error::JavaException)` for a pending exception without clearing it.
/// The caller or Java must handle it; `jni_call_with_catch!` clears and maps exceptions
/// raised by the call instead.
macro_rules! jni_call_post_check_ex {
    ( $jnienv:expr, $version:tt, $name:ident $(, $args:expr )* ) => ({
        jni_call_no_post_check_ex!($jnienv, $version, $name $(, $args)*).and_then(|ret| {
            if $jnienv.exception_check() {
                Err(crate::errors::Error::JavaException)
            } else {
                Ok(ret)
            }
        })
    })
}

/// Call an `Env` function with exception checks, then reject a null result.
///
/// Like `jni_call_post_check_ex!`, leaves exceptions pending and returns
/// `Err(Error::JavaException)`. Returns `Err(Error::NullPtr)` for a null result
/// without an exception.
macro_rules! jni_call_post_check_ex_and_null_ret {
    ( $jnienv:expr, $version:tt, $name:ident $(, $args:expr )* ) => ({
        jni_call_post_check_ex!($jnienv, $version, $name $(, $args)*).and_then(|ret| {
            if ret.is_null() {
                Err($crate::errors::Error::NullPtr(concat!(stringify!($name), " result")))
            } else {
                Ok(ret)
            }
        })
    })
}

/// Call an `Env` function after checking for a pending exception, then reject a null result.
///
/// Returns `Err(Error::JavaException)` without calling JNI if an exception is already
/// pending, or `Err(Error::NullPtr)` if the result is null. Exceptions raised by the call
/// are neither checked nor cleared; use `jni_call_with_catch!` to handle them.
macro_rules! jni_call_only_check_null_ret {
    ( $jnienv:expr, $version:tt, $name:ident $(, $args:expr )* ) => ({
        jni_call_no_post_check_ex!($jnienv, $version, $name $(, $args)*).and_then(|ret| {
            if ret.is_null() {
                Err($crate::errors::Error::NullPtr(concat!(stringify!($name), " result")))
            } else {
                Ok(ret)
            }
        })
    })
}

/// Helper for building an exception-handler chain based on `catch {}` patterns
///
/// Each entry has the form `<path> => <expr>`, matched in order via
/// `<ExType>::matches()`. A required terminal `else => <expr>` entry acts as
/// a catch-all: its expression is evaluated unconditionally for any exception that
/// was not claimed by an earlier handler.
macro_rules! __jni_match_exceptions {
    ($env:expr, $ex:expr $(,)?) => {
        compile_error!("Missing 'else' expression in catch block")
    };
    // Match the fallback before typed handlers so else is not parsed as a path.
    ($env:expr, $ex:expr, else => $ex_result:expr $(,)?) => {
        $ex_result
    };
    // Single typed handler (with optional trailing comma) - implies a missing
    // 'else' expression in the catch block
    ($env:expr, $ex:expr, $ex_type:path => $ex_result:expr $(,)?) => {
        compile_error!("Missing 'else' expression in catch block")
    };
    // Discard this match's Cast value; recurse on the remaining handlers.
    ($env:expr, $ex:expr, $ex_type:path => $ex_result:expr, $($rest:tt)+) => {
        if let Some(_) = <$ex_type>::matches($env, &$ex)? {
            $ex_result
        } else {
            __jni_match_exceptions!($env, $ex, $($rest)+)
        }
    };
    // Bind this match's Cast value; recurse on the remaining handlers.
    ($env:expr, $ex:expr, $ex_name:ident: $ex_type:path => $ex_result:expr, $($rest:tt)+) => {
        if let Some($ex_name) = <$ex_type>::matches($env, &$ex)? {
            $ex_result
        } else {
            __jni_match_exceptions!($env, $ex, $($rest)+)
        }
    };
}

/// Call an `Env` function, then clear and map any exception raised by the call.
///
/// A `catch |env| {}` block maps exceptions to `Result` values:
///
/// ```ignore
/// jni_call_with_catch!(
///     catch |env| {
///         ExceptionType1 => Err(Error::Type1),
///         exception: ExceptionType2 => {
///             let cause = exception.get_cause(env)?;
///             let cause = env.new_global_ref(cause)?;
///             Err(Error::Type2(cause))
///         },
///         // required catch-all
///         else => Err(Error::JniCall(JniError::Unknown)),
///     },
///     env, version, FunctionName, args...
/// )
/// ```
/// If an exception is pending before the call, returns `Err(Error::JavaException)`
/// without clearing it.
/// Exceptions raised by the call are cleared before handlers run. Types are matched in
/// order with `::matches()`; the first matching handler supplies the result. A named
/// handler (`exception: ExceptionType => ...`) receives the cast exception. The final
/// `else => ...` handler is required for unmatched exceptions.
///
/// Handlers run in a local frame with a mutable `Env` named by `catch |env|`, even when
/// the original environment reference is immutable.
macro_rules! jni_call_with_catch {
    ( catch |$env_mut:ident| { $($ex_handlers:tt)* }, $jnienv:expr, $version:tt, $name:ident $(, $args:expr )* ) => ({
        // The closure keeps `?` from returning from the caller.
        $crate::__must_use((|| -> $crate::errors::Result<_> {
            // Pre-existing exceptions bypass the handlers and remain pending.
            let ret = jni_call_no_post_check_ex!($jnienv, $version, $name $(, $args)*) ?;
            if $jnienv.exception_check() {
                $jnienv.with_local_frame(
                    $crate::DEFAULT_LOCAL_FRAME_CAPACITY,
                    |$env_mut| -> $crate::errors::Result<_> {
                        let e = $env_mut
                            .exception_occurred()
                            .expect("Expected an exception after ExceptionCheck");
                        $env_mut.exception_clear();
                        __jni_match_exceptions!($env_mut, e, $($ex_handlers)*)
                    },
                )
            } else {
                Ok(ret)
            }
        })())
    });
}

/// Apply `jni_call_with_catch!`, then return `Err(Error::NullPtr)` for a null result.
macro_rules! jni_call_with_catch_and_null_check {
    ( catch |$env_mut:ident| { $($ex_handlers:tt)* }, $jnienv:expr, $version:tt, $name:ident $(, $args:expr )* ) => ({
        jni_call_with_catch! {
            catch |$env_mut| { $($ex_handlers)* },
            $jnienv,
            $version,
            $name $(, $args)*
        }.and_then(|ret| {
            if ret.is_null() {
                Err($crate::errors::Error::NullPtr(concat!(stringify!($name), " result")))
            } else {
                Ok(ret)
            }
        })
    });
}

/// Run a block of JNI code, then clear and map any pending exception.
///
/// Uses the handler syntax of `jni_call_with_catch!`:
///
/// ```ignore
/// jni_try!(
///     (env) -> ResultType {
///         // JNI code here
///     },
///     catch |env| {
///         ExceptionType1 => Err(Error::Type1),
///         exception: ExceptionType2 => {
///             let cause = exception.get_cause(env)?;
///             let cause = env.new_global_ref(cause)?;
///             Err(Error::Type2(cause))
///         },
///         // required catch-all
///         else => Err(Error::JniCall(JniError::Unknown)),
///     },
/// )
/// ```
///
/// Pending exceptions are cleared and dispatched using the same ordered, typed handlers
/// and required fallback as `jni_call_with_catch!`. Handlers receive a mutable `Env` in a
/// local frame. If no exception is pending, returns the block's result unchanged.
macro_rules! jni_try {
    ( ($jnienv:expr) -> $ret_ty:ty { $($jni_code:tt)* } catch |$env_mut:ident| { $($ex_handlers:tt)* }) => ({
        // Wrap everything in a closure so any use of '?' can't return from the encompassing function.
        let ret = (|| -> $ret_ty {$($jni_code)*})();
        if $jnienv.exception_check() {
            $jnienv.with_local_frame(
                $crate::DEFAULT_LOCAL_FRAME_CAPACITY,
                |$env_mut| -> $ret_ty {
                    let e = $env_mut
                        .exception_occurred()
                        .expect("Expected an exception after ExceptionCheck");
                    $env_mut.exception_clear();
                    __jni_match_exceptions!($env_mut, e, $($ex_handlers)*)
                },
            )
        } else {
            ret
        }
    });
}

/// Map a pointer to `Ok(ptr)` or `Err(Error::NullPtr)` without returning from the caller.
macro_rules! null_check {
	( $obj:expr, $ctx:expr ) => {
		if $obj.is_null() {
			Err($crate::errors::Error::NullPtr($ctx))
		} else {
			Ok($obj)
		}
	};
}

/// Directly calls a `JavaVM` function, nothing else
///
/// # Safety
///
/// You must ensure that the arguments you pass are valid for the particular JNI
/// function you are calling.
macro_rules! java_vm_call_unchecked {
    ( $jvm:expr, $version:tt, $name:ident $(, $args:expr )*) => {{
        // SAFETY: `JavaVM::from_raw()` checks that the pointer is non-null.
        let jvm: *mut abos_jni_sys::JavaVM = $jvm.get_raw();
        ((*(*jvm)).$version.$name)(jvm $(, $args)*)
    }};
}

#[cfg(test)]
#[path = "../../tests/unit/internal/macros.rs"]
mod tests;
