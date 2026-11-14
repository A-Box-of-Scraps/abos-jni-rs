use std::borrow::Cow;

use abos_jni::{JValue, sys::jvalue};

// Keep the stack cost bounded, including when reference validation re-enters dynamic calls.
pub(super) const STACK_ARGS: usize = 8;

pub(super) fn with_jni_args<R>(args: &[JValue<'_>], call: impl FnOnce(&[jvalue]) -> R) -> R {
	let mut stack: [jvalue; STACK_ARGS] = [jvalue { j: 0 }; STACK_ARGS];
	let converted: Cow<'_, [jvalue]> = if args.is_empty() {
		Cow::Borrowed(&[])
	} else if args.len() <= STACK_ARGS {
		for (destination, argument) in stack.iter_mut().zip(args) {
			*destination = argument.as_jni();
		}
		Cow::Borrowed(&stack[..args.len()])
	} else {
		Cow::Owned(args.iter().map(JValue::as_jni).collect())
	};
	// Use one invocation site for both storage paths.
	call(&converted)
}
