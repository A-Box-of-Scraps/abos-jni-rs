//! Keep the committed seed corpus useful for reaching successful parsing paths.

use abos_cafebabe::{AccessFlags, ClassFile, parse_class};

fn parse_seed(bytes: &[u8]) -> ClassFile {
	let class: ClassFile = parse_class(bytes).expect("seed must parse successfully");
	assert_eq!(class.major_version, 52);
	assert_eq!(class.minor_version, 0);
	class
}

#[test]
fn minimal() {
	let class: ClassFile = parse_seed(include_bytes!("../seeds/cafebabe_parse_class/minimal.class"));
	assert_eq!(class.this_class, "java/lang/Object");
	assert!(class.super_class.is_none());
	assert!(class.interfaces.is_empty());
	assert!(class.fields.is_empty());
	assert!(class.methods.is_empty());
}

#[test]
fn members() {
	let class: ClassFile = parse_seed(include_bytes!("../seeds/cafebabe_parse_class/members.class"));
	assert_eq!(class.this_class, "sample/Members");
	assert_eq!(class.super_class.as_deref(), Some("java/lang/Object"));
	assert_eq!(class.interfaces, ["java/lang/Runnable"]);
	assert_eq!(class.fields.len(), 1);
	assert_eq!(class.fields[0].name, "value");
	assert_eq!(class.fields[0].descriptor, "I");
	assert!(class.fields[0].access_flags.contains(AccessFlags::STATIC));
	assert_eq!(class.methods.len(), 1);
	assert_eq!(class.methods[0].name, "run");
	assert_eq!(class.methods[0].descriptor, "()V");
	assert!(class.methods[0].access_flags.contains(AccessFlags::NATIVE));
}

#[test]
fn unicode() {
	let class: ClassFile = parse_seed(include_bytes!("../seeds/cafebabe_parse_class/unicode.class"));
	assert_eq!(class.this_class, "sample/Café");
	assert_eq!(class.super_class.as_deref(), Some("java/lang/Object"));
	assert!(class.interfaces.is_empty());
	assert_eq!(class.fields.len(), 1);
	assert_eq!(class.fields[0].name, "𐐀");
	assert_eq!(class.fields[0].descriptor, "Ljava/lang/String;");
	assert!(class.methods.is_empty());
}
