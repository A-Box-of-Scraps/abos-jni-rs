use super::{JvmError, checked_option_count};

#[test]
fn option_count_respects_jni_integer_range() {
	let limit: usize = usize::try_from(i32::MAX).expect("JNI integer limit fits usize");
	assert_eq!(checked_option_count(0).expect("zero options fit"), 0);
	assert_eq!(
		checked_option_count(limit).expect("maximum option count fits"),
		i32::MAX
	);
	for count in [limit + 1, usize::MAX] {
		let error: JvmError = checked_option_count(count).expect_err("oversized option count is rejected");
		assert!(matches!(error, JvmError::TooManyOptions(actual) if actual == count));
		assert!(error.opt_string().is_none());
	}
}
