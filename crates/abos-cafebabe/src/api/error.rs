/// A class-file parsing error with a byte offset into the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
	/// Position at which parsing failed.
	pub offset: usize,
	/// Description of the invalid or unsupported structure.
	pub message: &'static str,
}

impl std::fmt::Display for ParseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} at byte {}", self.message, self.offset)
	}
}

impl std::error::Error for ParseError {}
