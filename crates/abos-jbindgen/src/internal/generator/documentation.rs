use super::names::java_name_to_rust;
use std::fmt::Write as _;
/// Convert Javadoc to Rustdoc comments with parameter and return sections.
/// Removes paragraph and line-break tags, decodes basic HTML entities, and skips other Javadoc tags.
pub fn format_javadoc_as_rustdoc(javadoc: &str) -> String {
	let mut output = String::new();
	let mut in_param_section = false;
	let mut in_return_section = false;

	for line in javadoc.lines() {
		let trimmed: &str = line.trim();

		if trimmed.is_empty() {
			if !in_param_section && !in_return_section {
				output.push_str("///\n");
			}
			continue;
		}

		if trimmed.starts_with("@param") {
			if !in_param_section {
				output.push_str("///\n");
				output.push_str("/// # Parameters\n");
				in_param_section = true;
			}
			if let Some((param_name, desc)) = trimmed
				.strip_prefix("@param")
				.and_then(|rest| rest.trim().split_once(char::is_whitespace))
			{
				let rust_param_name: String = java_name_to_rust(param_name.trim()).0;
				writeln!(output, "/// * `{}` - {}", rust_param_name, desc.trim()).expect("writing to String");
			}
			continue;
		}

		if trimmed.starts_with("@return") {
			in_param_section = false;
			in_return_section = true;
			if let Some(desc) = trimmed.strip_prefix("@return") {
				output.push_str("///\n");
				output.push_str("/// # Returns\n");
				writeln!(output, "/// {}", desc.trim()).expect("writing to String");
			}
			continue;
		}

		if trimmed.starts_with("@throws") || trimmed.starts_with("@exception") {
			in_param_section = false;
			in_return_section = false;
			continue;
		}

		if trimmed.starts_with('@') {
			in_param_section = false;
			in_return_section = false;
			continue;
		}

		in_param_section = false;
		in_return_section = false;

		let cleaned: String = trimmed
			.replace("<p>", "")
			.replace("</p>", "")
			.replace("<br>", "")
			.replace("<br/>", "")
			.replace("&lt;", "<")
			.replace("&gt;", ">")
			.replace("&amp;", "&");

		writeln!(output, "/// {cleaned}").expect("writing to String");
	}

	output
}
