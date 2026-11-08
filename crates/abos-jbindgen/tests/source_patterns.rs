//! Source class patterns accept both dotted and binary nested-class names.
use abos_jbindgen::{Bindings, Builder, ModuleEvent};
use std::path::PathBuf;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn generate(pattern: &str) -> abos_jbindgen::Result<Bindings> {
	let source: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/java/PatternOuter.java");
	Builder::new()
		.input_sources(vec![source], Vec::new(), vec![pattern.to_owned()])
		.generate()
}

fn class_names(bindings: &Bindings) -> Vec<String> {
	let mut names: Vec<String> = Vec::new();
	bindings.walk(&mut |module, event| {
		if event == ModuleEvent::BeginModule {
			names.extend(module.bindings.iter().map(|binding| binding.java_type_name.clone()));
		}
	});
	names.sort();
	names
}

#[test]
fn dotted_and_binary_nested_patterns_select_the_same_exact_class() -> TestResult {
	for (dotted, binary) in [
		("PatternOuter.Inner", "PatternOuter$Inner"),
		("PatternOuter.Inner.Deep", "PatternOuter$Inner$Deep"),
		("PatternOuter$Inner.Deep", "PatternOuter$Inner$Deep"),
		("PatternOuter.Sibling", "PatternOuter$Sibling"),
	] {
		let dotted_bindings: Bindings = generate(&format!("com.example.{dotted}"))?;
		let binary_bindings: Bindings = generate(&format!("com.example.{binary}"))?;
		assert_eq!(class_names(&dotted_bindings), [binary], "{dotted}");
		assert_eq!(class_names(&binary_bindings), [binary], "{binary}");
		assert_eq!(dotted_bindings.to_string(), binary_bindings.to_string());
	}
	Ok(())
}

#[test]
fn outer_class_and_wildcard_patterns_select_expected_classes() -> TestResult {
	let all: [&str; 5] = [
		"PatternOuter",
		"PatternOuter$Inner",
		"PatternOuter$Inner$Deep",
		"PatternOuter$InnerSibling",
		"PatternOuter$Sibling",
	];
	for pattern in ["com.example.PatternOuter", "com.example.*", "*"] {
		assert_eq!(class_names(&generate(pattern)?), all, "{pattern}");
	}
	assert_eq!(class_names(&generate("com.example.PatternOuter$*")?), all[1..]);
	assert_eq!(class_names(&generate("com.example.PatternOuter.Inner*")?), all[1..4]);
	Ok(())
}

#[test]
fn comma_separated_patterns_accept_both_nested_name_spellings() -> TestResult {
	let bindings: Bindings = generate("com.example.PatternOuter.Inner, com.example.PatternOuter$Sibling")?;
	assert_eq!(class_names(&bindings), ["PatternOuter$Inner", "PatternOuter$Sibling"]);
	Ok(())
}
