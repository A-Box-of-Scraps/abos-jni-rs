use syn::{
	LitStr,
	parse::{Parse, ParseStream},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum JniVersion {
	Number { major: u16, minor: u16 },
	Reserved,
}

impl Default for JniVersion {
	fn default() -> Self {
		Self::Number { major: 1, minor: 1 }
	}
}

impl JniVersion {
	pub fn suffix(self) -> String {
		match self {
			Self::Number { major, minor: 0 } => major.to_string(),
			Self::Number { major, minor } => format!("{major}_{minor}"),
			Self::Reserved => "reserved".to_owned(),
		}
	}
}

impl Parse for JniVersion {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		let literal: LitStr = input.parse()?;
		let value: String = literal.value();
		if value == "reserved" {
			return Ok(Self::Reserved);
		}
		let (major, minor): (&str, &str) = value.split_once('.').unwrap_or((&value, "0"));
		let invalid = || {
			syn::Error::new(
				literal.span(),
				"Expected a JNI version such as \"1.2\", \"9\", or \"reserved\"",
			)
		};
		Ok(Self::Number {
			major: major.parse().map_err(|_| invalid())?,
			minor: minor.parse().map_err(|_| invalid())?,
		})
	}
}

#[cfg(test)]
#[path = "../../tests/unit/internal/version.rs"]
mod tests;
