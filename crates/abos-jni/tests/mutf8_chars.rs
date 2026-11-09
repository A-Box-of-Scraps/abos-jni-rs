#![allow(missing_docs, reason = "Integration tests are not library API.")]
#![cfg(feature = "invocation")]

use std::sync::atomic::{AtomicUsize, Ordering};

use abos_jni::{
	errors::{Error, Result},
	jni_str,
	objects::{Global, JString, JThrowable},
	strings::MUTF8Chars,
};

#[allow(unsafe_code, reason = "Shared JVM utilities include raw attachment helpers.")]
mod util;

struct StringDropWarnings(AtomicUsize);

impl log::Log for StringDropWarnings {
	fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
		metadata.level() <= log::Level::Warn && metadata.target().ends_with("mutf8_chars")
	}

	fn log(&self, record: &log::Record<'_>) {
		if self.enabled(record.metadata()) {
			self.0.fetch_add(1, Ordering::Relaxed);
		}
	}

	fn flush(&self) {}
}

#[test]
fn global_string_chars_drop_after_scoped_detachment() {
	static WARNINGS: StringDropWarnings = StringDropWarnings(AtomicUsize::new(0));
	log::set_logger(&WARNINGS).expect("install string guard cleanup logger");
	log::set_max_level(log::LevelFilter::Warn);

	let string: Global<JString<'static>> = util::attach_current_thread(|env| {
		let string: JString<'_> = env.new_string("scoped string")?;
		env.new_global_ref(string)
	})
	.expect("create global string");

	std::thread::spawn(move || {
		let vm: &std::sync::Arc<abos_jni::JavaVM> = util::jvm();
		assert!(!vm.is_thread_attached().expect("check initial attachment"));
		for _ in 0..8 {
			let chars: MUTF8Chars<'_, &JString<'_>> = vm
				.attach_current_thread_for_scope(|env| string.mutf8_chars(env))
				.expect("borrow global string characters");
			assert!(!vm.is_thread_attached().expect("check scoped detachment"));
			assert_eq!(chars.to_str(), "scoped string");

			drop(chars);
			assert_eq!(WARNINGS.0.load(Ordering::Relaxed), 0, "character cleanup failed");
			assert!(
				!vm.is_thread_attached()
					.expect("cleanup must not leave a permanent attachment")
			);
		}
		vm.attach_current_thread_for_scope(|_env| -> Result<()> {
			drop(string);
			Ok(())
		})
		.expect("release the global string while attached");
	})
	.join()
	.expect("worker releases character buffers after detachment");
}

#[test]
fn string_chars_drop_preserves_pending_exception() {
	util::attach_current_thread(|env| {
		let string: JString<'_> = env.new_string("pending exception")?;
		let chars: MUTF8Chars<'_, &JString<'_>> = string.mutf8_chars(env)?;
		assert!(matches!(
			env.throw_new(jni_str!("java/lang/RuntimeException"), jni_str!("keep pending")),
			Err(Error::JavaException)
		));
		let before: JThrowable<'_> = env.exception_occurred().expect("exception is pending");

		// Release from a nested frame while the string belongs to the outer frame.
		env.with_local_frame(4, |env| -> Result<()> {
			drop(chars);
			assert!(env.exception_check());
			Ok(())
		})?;

		let after: JThrowable<'_> = env
			.exception_occurred()
			.expect("cleanup preserves the pending exception");
		env.exception_clear();
		assert!(env.is_same_object(&before, &after)?);
		Ok(())
	})
	.expect("character cleanup is exception safe");
}
