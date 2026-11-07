use super::is_binding_class_entry;

#[test]
fn distinguishes_metadata_from_bindable_class_entries() {
	for name in [
		"Example.class",
		"sample/Example.class",
		"sample/Outer$Inner.class",
		"sample/module-info/Example.class",
	] {
		assert!(is_binding_class_entry(name), "{name}");
	}
	for name in [
		"module-info.class",
		"package-info.class",
		"sample/package-info.class",
		"META-INF/versions/17/sample/Example.class",
		"META-INF/Example.class",
		"sample/Example.java",
		"sample/Example.CLASS",
		"sample/Example.class/",
	] {
		assert!(!is_binding_class_entry(name), "{name}");
	}
}
