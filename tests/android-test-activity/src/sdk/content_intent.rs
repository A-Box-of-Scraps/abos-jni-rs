include!(concat!(env!("OUT_DIR"), "/content_intent_bindings.rs"));

use crate::test_error::{TestResult, require};

pub fn test_content_intent<'local>(env: &mut abos_jni::Env<'local>) -> TestResult {
	// Just verify the bindings were generated successfully by accessing a constant
	let action_view_str = android::content::AndroidIntent::ACTION_VIEW(env)?;
	let action_str = action_view_str.to_string();

	require(
		action_str == "android.intent.action.VIEW",
		format!("unexpected ACTION_VIEW: {action_str}"),
	)?;

	Ok(format!("Intent bindings: ACTION_VIEW = {}", action_str))
}
