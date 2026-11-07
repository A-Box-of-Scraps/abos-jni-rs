#[test]
fn ui() {
	for directory in ["src", "tests/integration"] {
		dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), format!("ui/fixture/{directory}"))
			.rustc_flags(["--edition=2024"])
			.run();
	}
}
