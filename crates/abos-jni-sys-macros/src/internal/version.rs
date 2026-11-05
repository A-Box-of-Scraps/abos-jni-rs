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
mod tests {
	use super::JniVersion;

	#[test]
	fn versions_are_ordered_numerically_with_reserved_last() {
		let mut versions: Vec<JniVersion> = ["reserved", "24", "1.10", "1.2", "999"]
			.into_iter()
			.map(|version| syn::parse_str(&format!("\"{version}\"")).expect("valid version"))
			.collect();
		versions.sort();
		let suffixes: Vec<String> = versions.into_iter().map(JniVersion::suffix).collect();
		assert_eq!(suffixes, ["1_2", "1_10", "24", "999", "reserved"]);
	}

	#[test]
	fn rejects_invalid_versions() {
		for version in ["", "1.", ".2", "1.2.3", "-1", "65536", "1.65536", "latest"] {
			assert!(
				syn::parse_str::<JniVersion>(&format!("\"{version}\"")).is_err(),
				"{version}"
			);
		}
	}
}
