/// Configuration for filtering hidden APIs
#[derive(Debug, Clone, Default)]
pub struct HiddenApiFilter {
	/// Allow APIs marked as "unsupported"
	pub allow_unsupported: bool,
	/// Maximum target level (e.g., "o", "p", "q") for conditional API support
	/// When set, APIs with `max-target-<level>` are included if level >= this value
	pub max_target: Option<String>,
}
