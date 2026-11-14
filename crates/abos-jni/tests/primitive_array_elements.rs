#![allow(unsafe_code, reason = "Exercise JNI array element access and native test utilities.")]
#![allow(missing_docs, reason = "Executable examples and test fixtures are not library API.")]
#![cfg(feature = "invocation")]

use abos_jni::{
	elements::{AutoElements, AutoElementsCritical, ReleaseMode},
	errors::{Error, Result},
	jni_str,
	objects::{JIntArray, JThrowable},
	sys::jint,
};

mod util;
use util::attach_current_thread;

macro_rules! owned_elements_tests {
	($module:ident, $acquire:ident, $guard:ident) => {
		mod $module {
			use super::*;

			#[test]
			fn owns_reference_and_copies_back() {
				attach_current_thread(|env| {
					let array: JIntArray<'_> = env.new_int_array(3)?;
					array.set_region(env, 0, &[1, 2, 3])?;
					let mut elements: $guard<'_, jint, JIntArray<'_>> = {
						let owned: JIntArray<'_> = env.new_local_ref(&array)?;
						// SAFETY: Only this guard accesses the elements. No JNI calls or blocking
						// operations occur between acquisition and drop, including on critical paths.
						unsafe { owned.$acquire(env, ReleaseMode::CopyBack)? }
					};
					assert_eq!(&*elements, &[1, 2, 3]);
					elements.reverse();
					drop(elements);

					let mut result: [jint; 3] = [0; 3];
					array.get_region(env, 0, &mut result)?;
					assert_eq!(result, [3, 2, 1]);
					assert!(!env.exception_check());
					Ok(())
				})
				.expect("owned elements are released and copied back");
			}

			#[test]
			fn no_copy_back_and_discard() {
				attach_current_thread(|env| {
					for mode in [ReleaseMode::NoCopyBack, ReleaseMode::CopyBack] {
						let array: JIntArray<'_> = env.new_int_array(1)?;
						array.set_region(env, 0, &[7])?;
						let owned: JIntArray<'_> = env.new_local_ref(&array)?;
						// SAFETY: The guard has exclusive element access and is released before any JNI work.
						let elements: $guard<'_, jint, JIntArray<'_>> = unsafe { owned.$acquire(env, mode)? };
						assert_eq!(elements[0], 7);
						if matches!(mode, ReleaseMode::CopyBack) {
							elements.discard();
						} else {
							drop(elements);
						}

						let mut result: [jint; 1] = [0];
						array.get_region(env, 0, &mut result)?;
						assert_eq!(result, [7]);
					}
					Ok(())
				})
				.expect("owned guards preserve release modes");
			}

			#[test]
			fn empty_array_from_temporary() {
				attach_current_thread(|env| {
					let elements: $guard<'_, jint, JIntArray<'_>> = {
						// SAFETY: The newly allocated array has no other users. The critical section
						// contains only guard inspection and ends before the exception check.
						unsafe { env.new_int_array(0)?.$acquire(env, ReleaseMode::NoCopyBack)? }
					};
					assert!(elements.is_empty());
					assert_eq!(elements.len(), 0);
					drop(elements);
					assert!(!env.exception_check());
					Ok(())
				})
				.expect("temporary empty arrays can be consumed");
			}

			#[test]
			fn null_array_is_rejected() {
				attach_current_thread(|env| {
					let result: Result<$guard<'_, jint, JIntArray<'_>>> = {
						// SAFETY: Null references are rejected before acquiring elements or entering a critical section.
						unsafe { JIntArray::null().$acquire(env, ReleaseMode::NoCopyBack) }
					};
					assert!(matches!(result, Err(Error::NullPtr(_))));
					assert!(!env.exception_check());
					Ok(())
				})
				.expect("null arrays return an error");
			}

			#[test]
			fn pending_exception_is_preserved() {
				attach_current_thread(|env| {
					let array: JIntArray<'_> = env.new_int_array(1)?;
					assert!(matches!(
						env.throw_new(
							jni_str!("java/lang/IllegalStateException"),
							jni_str!("pending sentinel")
						),
						Err(Error::JavaException)
					));
					let original: JThrowable<'_> = env.exception_occurred().expect("pending exception");
					// SAFETY: The array is unshared. The pending exception prevents acquisition,
					// so no critical section is entered and subsequent exception inspection is allowed.
					let error: Error = unsafe { array.$acquire(env, ReleaseMode::NoCopyBack) }.unwrap_err();
					assert!(matches!(error, Error::JavaException));
					let pending: JThrowable<'_> = env.exception_occurred().expect("exception remains pending");
					env.exception_clear();
					assert!(env.is_same_object(original, pending)?);
					Ok(())
				})
				.expect("acquisition preserves pre-existing exceptions");
			}
		}
	};
}

owned_elements_tests!(regular, into_elements, AutoElements);
owned_elements_tests!(critical, into_elements_critical, AutoElementsCritical);

#[test]
fn regular_guard_can_outlive_acquisition_frame() {
	attach_current_thread(|env| {
		let array: JIntArray<'_> = env.new_int_array(1)?;
		// Only the acquisition environment belongs to the nested frame, not the array reference.
		let mut elements: AutoElements<'_, jint, JIntArray<'_>> = env.with_local_frame(1, |inner| -> Result<_> {
			// SAFETY: The array is unshared, and ordinary element guards allow unrelated JNI calls.
			unsafe { array.into_elements(inner, ReleaseMode::CopyBack) }
		})?;
		elements[0] = 42;
		assert_eq!(elements[0], 42);
		drop(elements);
		Ok(())
	})
	.expect("ordinary guards are tied to the array frame, not the acquisition frame");
}
