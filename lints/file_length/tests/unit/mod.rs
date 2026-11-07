use std::fs;
use std::path::Path;

#[test]
fn ui() {
	let temporary = tempfile::Builder::new().prefix("file-length-").tempdir().unwrap();
	let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
	// Keep the checked-in fixtures readable; pad physical files only for the UI run.
	for (name, lines, terminated) in [
		("main.rs", 0, true),
		("below_limit.rs", 599, true),
		("at_limit.rs", 600, true),
		("crlf.rs", 601, true),
		("too_long.rs", 601, true),
		("unterminated.rs", 602, false),
		("allowed.rs", 601, true),
		("auxiliary/at_limit.rs", 600, false),
		("auxiliary/too_long.rs", 601, true),
		("auxiliary/empty.rs", 0, true),
		("auxiliary/blank.rs", 601, true),
		("auxiliary/allowed.rs", 601, true),
		("auxiliary/inner_allowed.rs", 601, true),
		("auxiliary/included.rs", 601, true),
		("auxiliary/target/generated.rs", 601, true),
		("auxiliary/.hidden.rs", 601, true),
	] {
		let source = fixtures.join(match name {
			"auxiliary/target/generated.rs" | "auxiliary/.hidden.rs" => "auxiliary/included.rs",
			_ => name,
		});
		let target = temporary.path().join(name);
		fs::create_dir_all(target.parent().unwrap()).unwrap();
		let mut content = fs::read_to_string(&source).unwrap();
		assert!(lines == 0 || content.lines().count() <= lines);
		for _ in content.lines().count()..lines {
			content.push_str(if name == "auxiliary/blank.rs" {
				"\n"
			} else {
				"// padding\n"
			});
		}
		if !terminated {
			assert_eq!(content.pop(), Some('\n'));
		}
		if name == "crlf.rs" {
			content = content.replace('\n', "\r\n");
		}
		fs::write(&target, content).unwrap();
		let snapshot = source.with_extension("stderr");
		if snapshot.exists() {
			fs::copy(snapshot, target.with_extension("stderr")).unwrap();
		}
	}
	dylint_testing::ui_test(env!("CARGO_PKG_NAME"), temporary.path());
}

#[test]
fn skips_generated_hidden_and_non_rust_paths() {
	for path in [
		"target/generated.rs",
		"nested/target/generated.rs",
		".references/source.rs",
		"src/.hidden.rs",
		"notes.txt",
	] {
		assert!(super::excluded(Path::new(path)), "{path}");
	}
	for path in ["src/lib.rs", "tests/unit/mod.rs", "../source.rs", "src/target_name.rs"] {
		assert!(!super::excluded(Path::new(path)), "{path}");
	}
}
