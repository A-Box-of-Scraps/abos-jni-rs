use super::names::java_name_to_rust;
use std::fmt::Write as _;
///
/// This takes a Javadoc comment string and converts it to Rustdoc format.
/// - Converts `@param` tags to parameter documentation
/// - Converts `@return` tags to Returns section
/// - Strips HTML tags
/// - Formats as `///` style comments
pub fn format_javadoc_as_rustdoc(javadoc: &str) -> String {
	let mut output = String::new();
	let mut in_param_section = false;
	let mut in_return_section = false;

	for line in javadoc.lines() {
		let trimmed: &str = line.trim();

		// Skip empty lines in special sections
		if trimmed.is_empty() {
			if !in_param_section && !in_return_section {
				output.push_str("///\n");
			}
			continue;
		}

		// Handle @param tags
		if trimmed.starts_with("@param") {
			if !in_param_section {
				output.push_str("///\n");
				output.push_str("/// # Parameters\n");
				in_param_section = true;
			}
			// Extract parameter name and description
			if let Some((param_name, desc)) = trimmed
				.strip_prefix("@param")
				.and_then(|rest| rest.trim().split_once(char::is_whitespace))
			{
				// Convert parameter name to snake_case
				let rust_param_name: String = java_name_to_rust(param_name.trim()).0;
				writeln!(output, "/// * `{}` - {}", rust_param_name, desc.trim()).expect("writing to String");
			}
			continue;
		}

		// Handle @return tags
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

		// Handle @throws/@exception tags
		if trimmed.starts_with("@throws") || trimmed.starts_with("@exception") {
			in_param_section = false;
			in_return_section = false;
			continue; // Skip for now, could add Errors section
		}

		// Handle other @ tags by skipping them
		if trimmed.starts_with('@') {
			in_param_section = false;
			in_return_section = false;
			continue;
		}

		// Regular documentation line
		in_param_section = false;
		in_return_section = false;

		// Basic HTML tag stripping (simple version)
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
