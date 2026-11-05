use crate::api::jvalue;

impl Clone for jvalue {
	fn clone(&self) -> Self {
		*self
	}
}
impl core::fmt::Debug for jvalue {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		// The active member is unknown; other members may contain uninitialized bytes.
		f.debug_struct("jvalue").finish_non_exhaustive()
	}
}
