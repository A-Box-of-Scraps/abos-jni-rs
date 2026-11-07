//! Handwritten JVM contracts, not snapshots derived from either parser.
use super::api::{FieldFlags, MethodFlags, RuntimeApi};
use std::collections::{BTreeMap, BTreeSet};

const PUBLIC: MethodFlags = MethodFlags {
	is_static: false,
	is_native: false,
	is_public: true,
};
const NON_PUBLIC: MethodFlags = MethodFlags {
	is_public: false,
	..PUBLIC
};
const STATIC: MethodFlags = MethodFlags {
	is_static: true,
	..PUBLIC
};
const NATIVE: MethodFlags = MethodFlags {
	is_native: true,
	..PUBLIC
};
const NATIVE_NON_PUBLIC: MethodFlags = MethodFlags {
	is_public: false,
	..NATIVE
};
const FIELD: FieldFlags = FieldFlags {
	is_static: false,
	is_final: false,
};
const FINAL: FieldFlags = FieldFlags {
	is_final: true,
	..FIELD
};
const STATIC_FIELD: FieldFlags = FieldFlags {
	is_static: true,
	..FIELD
};
const CONSTANT: FieldFlags = FieldFlags {
	is_final: true,
	..STATIC_FIELD
};

fn class(name: &str, parents: &[&str], methods: &[(&str, MethodFlags)], fields: &[(&str, FieldFlags)]) -> RuntimeApi {
	let mut api = RuntimeApi {
		class_name: name.to_owned(),
		parents: parents.iter().map(|name| (*name).to_owned()).collect::<BTreeSet<_>>(),
		methods: BTreeMap::new(),
		fields: BTreeMap::new(),
	};
	for (signature, flags) in methods {
		assert!(api.methods.insert(format!("L{name};->{signature}"), *flags).is_none());
	}
	for (signature, flags) in fields {
		assert!(api.fields.insert(format!("L{name};->{signature}"), *flags).is_none());
	}
	api
}

pub fn all() -> BTreeMap<String, RuntimeApi> {
	let classes: Vec<RuntimeApi> = [
		generics(),
		enclosing(),
		records(),
		enums(),
		members(),
		identifiers(),
		overloads(),
	]
	.concat();
	let mut result: BTreeMap<String, RuntimeApi> = BTreeMap::new();
	for api in classes {
		assert!(result.insert(api.class_name.clone(), api).is_none());
	}
	result
}

fn generics() -> Vec<RuntimeApi> {
	vec![class(
		"com/example/GenericTypes",
		&[],
		&[
			("<init>(Ljava/lang/Number;Ljava/lang/Object;)V", PUBLIC),
			("echo(Ljava/lang/Number;)Ljava/lang/Number;", PUBLIC),
			("unbounded(Ljava/lang/Object;)Ljava/lang/Object;", PUBLIC),
			("echoDependent(Ljava/lang/Number;)Ljava/lang/Number;", PUBLIC),
			("echoArray([Ljava/lang/Number;)[Ljava/lang/Number;", PUBLIC),
			("matrix([[Ljava/lang/Object;)[[Ljava/lang/Object;", PUBLIC),
			("related(Ljava/lang/Number;)Ljava/lang/Number;", PUBLIC),
			("identity(Ljava/lang/Object;)Ljava/lang/Object;", STATIC),
			("bounded(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", STATIC),
			("recursive(Ljava/lang/Comparable;)Ljava/lang/Comparable;", PUBLIC),
			("methodArray([[Ljava/lang/Number;)[[Ljava/lang/Number;", PUBLIC),
			("echoList(Ljava/util/List;)Ljava/util/List;", PUBLIC),
			("entry(Ljava/util/Map$Entry;)Ljava/util/Map$Entry;", PUBLIC),
		],
		&[
			("number:Ljava/lang/Number;", FIELD),
			("value:Ljava/lang/Object;", FIELD),
			("dependent:Ljava/lang/Number;", FIELD),
			("numbers:[Ljava/lang/Number;", FIELD),
			("values:[[Ljava/lang/Object;", FIELD),
			("list:Ljava/util/List;", FIELD),
		],
	)]
}

fn enclosing() -> Vec<RuntimeApi> {
	vec![
		class("com/example/EnclosingTypes", &[], &[("<init>(I)V", PUBLIC)], &[]),
		class(
			"com/example/EnclosingTypes$Inner",
			&[],
			&[("<init>(Lcom/example/EnclosingTypes;I)V", PUBLIC), ("value()I", PUBLIC)],
			&[],
		),
		class(
			"com/example/EnclosingTypes$Inner$Deep",
			&[],
			&[
				("<init>(Lcom/example/EnclosingTypes$Inner;I)V", PUBLIC),
				("value()I", PUBLIC),
			],
			&[],
		),
		class(
			"com/example/EnclosingTypes$Implicit",
			&[],
			&[("<init>(Lcom/example/EnclosingTypes;)V", PUBLIC), ("value()I", PUBLIC)],
			&[],
		),
		class(
			"com/example/EnclosingTypes$Derived",
			&["com/example/EnclosingTypes$Inner"],
			&[("<init>(Lcom/example/EnclosingTypes;I)V", PUBLIC)],
			&[],
		),
		class(
			"com/example/EnclosingTypes$StaticNested",
			&[],
			&[("<init>(I)V", PUBLIC), ("value()I", PUBLIC)],
			&[],
		),
		class("com/example/EnclosingTypes$Container", &[], &[], &[]),
		class(
			"com/example/EnclosingTypes$Container$Member",
			&[],
			&[("<init>(I)V", PUBLIC), ("value()I", PUBLIC)],
			&[],
		),
	]
}

fn record(name: &str, constructor: &str, accessor: &str) -> RuntimeApi {
	class(
		name,
		&["java/lang/Record"],
		&[
			(constructor, PUBLIC),
			(accessor, PUBLIC),
			("equals(Ljava/lang/Object;)Z", PUBLIC),
			("hashCode()I", PUBLIC),
			("toString()Ljava/lang/String;", PUBLIC),
		],
		&[],
	)
}

fn records() -> Vec<RuntimeApi> {
	let mut top: RuntimeApi = record("com/example/RecordTypes", "<init>(I)V", "value()I");
	top.methods.insert(
		"Lcom/example/RecordTypes;->marker()Lcom/example/Marker;".to_owned(),
		STATIC,
	);
	vec![
		top,
		record(
			"com/example/RecordTypes$Nested",
			"<init>([Lcom/example/RecordTypes;)V",
			"values()[Lcom/example/RecordTypes;",
		),
		class(
			"com/example/RecordTypes$Member",
			&[],
			&[
				("<init>()V", PUBLIC),
				("echo(Lcom/example/RecordTypes;)Lcom/example/RecordTypes;", PUBLIC),
			],
			&[],
		),
		class(
			"com/example/Marker",
			&["java/lang/annotation/Annotation"],
			&[("value()I", PUBLIC)],
			&[],
		),
		class(
			"com/example/Marker$Nested",
			&["java/lang/annotation/Annotation"],
			&[("value()[Lcom/example/Marker;", PUBLIC)],
			&[],
		),
		record(
			"com/example/Marker$Member",
			"<init>([Lcom/example/Marker;)V",
			"values()[Lcom/example/Marker;",
		),
	]
}

fn enums() -> Vec<RuntimeApi> {
	[
		("EnumTypes", &["RED", "BLUE"][..]),
		("EnumTypes$State", &["ON", "OFF"][..]),
		("EnumTypes$Empty", &[][..]),
	]
	.into_iter()
	.map(|(simple, constants)| {
		let name: String = format!("com/example/{simple}");
		let mut api: RuntimeApi = class(
			&name,
			&["java/lang/Enum"],
			&[
				(&format!("values()[L{name};"), STATIC),
				(&format!("valueOf(Ljava/lang/String;)L{name};"), STATIC),
			],
			&[],
		);
		for constant in constants {
			api.fields.insert(format!("L{name};->{constant}:L{name};"), CONSTANT);
		}
		if simple == "EnumTypes" {
			api.methods.insert(format!("L{name};->code()I"), PUBLIC);
			api.fields.insert(format!("L{name};->id:I"), FINAL);
		}
		api
	})
	.collect()
}

fn members() -> Vec<RuntimeApi> {
	vec![
		class(
			"com/example/ParityVisibility",
			&[],
			&[
				("<init>()V", PUBLIC),
				("<init>(I)V", NON_PUBLIC),
				("<init>(Ljava/lang/String;)V", NON_PUBLIC),
				("publicMethod()I", PUBLIC),
				("protectedMethod()I", NON_PUBLIC),
				("packageMethod()I", NON_PUBLIC),
				("publicNative()I", NATIVE),
				("protectedNative()I", NATIVE_NON_PUBLIC),
				("packageNative()I", NATIVE_NON_PUBLIC),
				("privateNative()I", NATIVE_NON_PUBLIC),
			],
			&[
				("publicField:I", FIELD),
				("protectedField:I", FIELD),
				("packageField:I", FIELD),
			],
		),
		class(
			"com/example/ParityMembers",
			&[],
			&[
				("<init>(I)V", PUBLIC),
				("choose(I)I", PUBLIC),
				("choose(Ljava/lang/String;)I", PUBLIC),
				("choose([I)I", STATIC),
				("choose([Ljava/lang/String;)I", STATIC),
				("matrix([[I)[[I", PUBLIC),
			],
			&[
				("shared:I", STATIC_FIELD),
				("CONSTANT:I", CONSTANT),
				("value:I", FIELD),
				("fixed:I", FINAL),
			],
		),
		class("com/example/ParityInheritance", &[], &[("<init>()V", PUBLIC)], &[]),
		class(
			"com/example/ParityInheritance$Named",
			&[],
			&[("defaultValue()I", PUBLIC)],
			&[],
		),
		class(
			"com/example/ParityInheritance$Base",
			&[],
			&[
				("<init>()V", PUBLIC),
				("echo(Ljava/lang/Object;)Ljava/lang/Object;", PUBLIC),
				("number()Ljava/lang/Number;", PUBLIC),
				("inherited()I", PUBLIC),
			],
			&[],
		),
		class(
			"com/example/ParityInheritance$Child",
			&[
				"com/example/ParityInheritance$Base",
				"com/example/ParityInheritance$Named",
			],
			&[
				("<init>()V", PUBLIC),
				("echo(Ljava/lang/String;)Ljava/lang/String;", PUBLIC),
				("number()Ljava/lang/Integer;", PUBLIC),
			],
			&[],
		),
	]
}

fn identifiers() -> Vec<RuntimeApi> {
	vec![
		class(
			"I",
			&[],
			&[("<init>()V", PUBLIC), ("echo([LI;)[LI;", PUBLIC)],
			&[("peer:LI;", FIELD)],
		),
		class(
			"DefaultPeer",
			&[],
			&[
				("<init>(LDefaultType;)V", PUBLIC),
				("echoOwner(LDefaultType;)LDefaultType;", PUBLIC),
			],
			&[("owner:LDefaultType;", FIELD)],
		),
		class(
			"DefaultType",
			&[],
			&[
				("<init>()V", PUBLIC),
				("echo(LDefaultType;)LDefaultType;", PUBLIC),
				("echoPeer(LDefaultPeer;)LDefaultPeer;", PUBLIC),
				("echoPeers([LDefaultPeer;)[LDefaultPeer;", PUBLIC),
				("echoMatrix([[LDefaultType;)[[LDefaultType;", PUBLIC),
				("echoChild(LDefaultType$Child;)LDefaultType$Child;", PUBLIC),
			],
			&[("peer:LDefaultPeer;", FIELD)],
		),
		class(
			"DefaultType$Child",
			&["DefaultType"],
			&[("<init>()V", PUBLIC), ("parent()LDefaultType;", PUBLIC)],
			&[],
		),
		class(
			"com/type/Identifiers",
			&[],
			&[
				("<init>()V", PUBLIC),
				("match(IIIII)I", PUBLIC),
				("gen()I", PUBLIC),
				("self()I", PUBLIC),
				("dollars(I)I", PUBLIC),
				("λ()I", PUBLIC),
				("echo(Lcom/type/match;)Lcom/type/match;", PUBLIC),
			],
			&[
				("type:I", FIELD),
				("type_:I", FIELD),
				("non_null:I", FIELD),
				("café:I", FIELD),
			],
		),
		class(
			"com/type/match",
			&[],
			&[("<init>()V", PUBLIC), ("value()I", PUBLIC)],
			&[],
		),
		class(
			"com/example/UnsupportedNames",
			&[],
			&[("<init>()V", PUBLIC), ("foo$bar()I", PUBLIC)],
			&[("$value:I", FIELD)],
		),
	]
}

fn overloads() -> Vec<RuntimeApi> {
	let constructors = [
		"()V",
		"(I)V",
		"(Ljava/lang/String;)V",
		"(ILjava/lang/String;)V",
		"(Ljava/lang/String;I)V",
		"(Ljava/lang/String;Ljava/lang/String;)V",
		"(III)V",
		"(IIII)V",
		"(ILjava/lang/String;II)V",
		"(ILjava/lang/String;Ljava/lang/String;I)V",
		"(ILjava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
	];
	let mut api: RuntimeApi = class(
		"com/example/Overloads",
		&[],
		&[
			("process()V", PUBLIC),
			("process(I)V", PUBLIC),
			("process(Ljava/lang/String;)V", PUBLIC),
			("calculate(II)I", PUBLIC),
			("calculate(ILjava/lang/String;)I", PUBLIC),
			("calculate(Ljava/lang/String;I)I", PUBLIC),
			("calculate(Ljava/lang/String;Ljava/lang/String;)I", PUBLIC),
			("transform(ILjava/lang/String;Z)V", PUBLIC),
			("transform(ILjava/lang/String;I)V", PUBLIC),
			("transform(IIZ)V", PUBLIC),
			("update([I)V", PUBLIC),
			("update([Ljava/lang/String;)V", PUBLIC),
			("update([[I)V", PUBLIC),
			("set(I)V", PUBLIC),
			("set(II)V", PUBLIC),
			("set(III)V", PUBLIC),
			("format(Ljava/lang/String;)Ljava/lang/String;", STATIC),
			("format(Ljava/lang/String;Ljava/lang/Object;)Ljava/lang/String;", STATIC),
			(
				"format(Ljava/lang/String;Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/String;",
				STATIC,
			),
			("method1()V", PUBLIC),
			("method1(I)V", PUBLIC),
			("method1(II)V", PUBLIC),
			("combine(ILjava/lang/String;)V", PUBLIC),
			("combine(Ljava/lang/Integer;Ljava/lang/String;)V", PUBLIC),
			("combine(II)V", PUBLIC),
		],
		&[],
	);
	for descriptor in constructors {
		api.methods
			.insert(format!("Lcom/example/Overloads;-><init>{descriptor}"), PUBLIC);
	}
	vec![api]
}

// Full access flags: 0x1010 = synthetic/final; 0x101a = synthetic/private/static/final;
// 0x100a = synthetic/private/static; 0x1041 = synthetic/bridge/public.
pub fn artifacts(name: &str) -> BTreeMap<String, u16> {
	let members: Vec<(String, u16)> = match name {
		"com/example/EnclosingTypes$Inner" | "com/example/EnclosingTypes$Implicit" => {
			vec![("this$0:Lcom/example/EnclosingTypes;".to_owned(), 0x1010)]
		}
		"com/example/EnclosingTypes$Inner$Deep" => {
			vec![("this$1:Lcom/example/EnclosingTypes$Inner;".to_owned(), 0x1010)]
		}
		"com/example/EnumTypes" | "com/example/EnumTypes$State" | "com/example/EnumTypes$Empty" => vec![
			(format!("$VALUES:[L{name};"), 0x101a),
			(format!("$values()[L{name};"), 0x100a),
		],
		"com/example/ParityInheritance$Child" => vec![
			("echo(Ljava/lang/Object;)Ljava/lang/Object;".to_owned(), 0x1041),
			("number()Ljava/lang/Number;".to_owned(), 0x1041),
		],
		_ => Vec::new(),
	};
	members
		.into_iter()
		.map(|(signature, flags)| (format!("L{name};->{signature}"), flags))
		.collect()
}
