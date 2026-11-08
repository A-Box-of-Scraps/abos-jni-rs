use std::{collections::BTreeMap, fmt::Write};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Field, FnArg, ForeignItem, Ident, Item, ItemStruct, LitStr, TypeFnPtr};

use crate::types::{c_name, c_type, function_type};

type Version = (u16, u16);
const RESERVED: Version = (u16::MAX, u16::MAX);

#[derive(Default)]
pub struct Probe {
	pub signatures: String,
	pub measurements: String,
	pub expected: TokenStream,
}

impl Probe {
	pub fn add_source(&mut self, source: &str) {
		let file: syn::File = syn::parse_file(source).expect("parse JNI bindings");
		for item in file.items {
			self.item(item);
		}
	}

	fn item(&mut self, item: Item) {
		match item {
			Item::Type(alias) => self.alias(&alias),
			Item::Const(constant) => {
				let name = constant.ident;
				self.measure(&name.to_string(), &name.to_string(), &quote!(i64::from(sys::#name)));
			}
			Item::Struct(record) if record.attrs.iter().any(|attr| attr.path().is_ident("jni_to_union")) => {
				self.table(&record);
			}
			Item::Struct(record) => self.record(&record.ident, record.fields.iter()),
			Item::Union(record) => self.record(&record.ident, record.fields.named.iter()),
			Item::Enum(enumeration) if !enumeration.variants.is_empty() => {
				let name = enumeration.ident;
				self.layout(&name, &c_name(&name.to_string()));
				for variant in enumeration.variants {
					let variant = variant.ident;
					self.measure(
						&variant.to_string(),
						&variant.to_string(),
						&quote!(sys::#name::#variant as i64),
					);
				}
			}
			Item::ForeignMod(block) => self.foreign_functions(&block),
			_ => {}
		}
	}

	fn foreign_functions(&mut self, block: &syn::ItemForeignMod) {
		for item in &block.items {
			let ForeignItem::Fn(function) = item else {
				panic!("Unsupported foreign item")
			};
			let abi = &block.abi;
			let output = &function.sig.output;
			let inputs = function.sig.inputs.iter().map(|input| match input {
				FnArg::Typed(argument) => &argument.ty,
				FnArg::Receiver(_) => panic!("Unexpected JNI receiver"),
			});
			let variadic = &function.sig.variadic;
			let ty: TypeFnPtr =
				syn::parse2(quote!(unsafe #abi fn(#(#inputs,)* #variadic) #output)).expect("foreign function pointer");
			let name = function.sig.ident.to_string();
			self.signature(&name, &format!("&{name}"), &function_type(&ty));
		}
	}

	fn alias(&mut self, alias: &syn::ItemType) {
		let name = &alias.ident;
		self.layout(name, &name.to_string());
		// Compare parameter types, allowing C array typedefs such as va_list to decay.
		self.signature(
			&name.to_string(),
			&format!("(void (*)({name}))0"),
			&format!("void (*)({})", c_type(&alias.ty)),
		);
	}

	fn record<'a>(&mut self, name: &Ident, fields: impl Iterator<Item = &'a Field>) {
		let c_record = c_name(&name.to_string());
		self.layout(name, &c_record);
		for field in fields {
			self.field_type(&c_record, field);
			self.field_offset(name, &c_record, field);
		}
	}

	fn table(&mut self, table: &ItemStruct) {
		let name = &table.ident;
		let c_record = c_name(&name.to_string());
		self.layout(name, &c_record);
		let fields: Vec<(&Field, Version)> = table.fields.iter().map(|field| (field, version(field))).collect();
		let views: BTreeMap<Version, String> = fields.iter().map(|(_, version)| (*version, suffix(*version))).collect();
		for field in &table.fields {
			self.field_type(&c_record, field);
		}
		for (version, suffix) in views {
			let view = format_ident!("{name}_{suffix}");
			let last = fields
				.iter()
				.rfind(|(_, added)| *added <= version)
				.expect("nonempty JNI view")
				.0;
			let last_name = last.ident.as_ref().expect("named field");
			// All table slots are pointers; a historical view ends immediately after its
			// last available field, even if newer entries create holes in the middle.
			let size = format!("offsetof({c_record}, {last_name}) + sizeof((({c_record} *)0)->{last_name})");
			self.measure_layout(&view, &c_record, &size);
			for (field, _) in fields.iter().filter(|(_, added)| *added <= version) {
				self.field_offset(&view, &c_record, field);
			}
		}
	}

	fn layout(&mut self, name: &Ident, c_type: &str) {
		self.measure_layout(name, c_type, &format!("sizeof({c_type})"));
	}

	fn measure_layout(&mut self, name: &Ident, c_type: &str, size: &str) {
		self.measure_usize(&format!("sizeof({name})"), size, &quote!(size_of::<sys::#name>()));
		self.measure_usize(
			&format!("alignof({name})"),
			&format!("_Alignof({c_type})"),
			&quote!(align_of::<sys::#name>()),
		);
	}

	fn field_type(&mut self, c_record: &str, field: &Field) {
		let name = field.ident.as_ref().expect("named field");
		self.signature(
			&format!("{c_record}.{name}"),
			&format!("(({c_record} *)0)->{name}"),
			&c_type(&field.ty),
		);
	}

	fn field_offset(&mut self, rust_record: &Ident, c_record: &str, field: &Field) {
		let name = field.ident.as_ref().expect("named field");
		self.measure_usize(
			&format!("offsetof({rust_record}, {name})"),
			&format!("offsetof({c_record}, {name})"),
			&quote!(offset_of!(sys::#rust_record, #name)),
		);
	}

	fn signature(&mut self, label: &str, expression: &str, expected: &str) {
		writeln!(
			self.signatures,
			"_Static_assert(_Generic(({expression}), {expected}: 1, default: 0), \"{label}\");"
		)
		.expect("write type assertion");
	}

	fn measure_usize(&mut self, label: &str, expression: &str, expected: &TokenStream) {
		self.measure(
			label,
			expression,
			&quote!(i64::try_from(#expected).expect("layout fits i64")),
		);
	}

	fn measure(&mut self, label: &str, expression: &str, expected: &TokenStream) {
		writeln!(self.measurements, "(int64_t)({expression}), /* {label} */").expect("write measurement");
		self.expected.extend(quote!((#label, #expected),));
	}
}

fn version(field: &Field) -> Version {
	let Some(attribute) = field.attrs.iter().find(|attr| attr.path().is_ident("jni_added")) else {
		return (1, 1);
	};
	let value = attribute.parse_args::<LitStr>().expect("JNI version").value();
	if value == "reserved" {
		return RESERVED;
	}
	let (major, minor) = value.split_once('.').unwrap_or((&value, "0"));
	(
		major.parse().expect("major JNI version"),
		minor.parse().expect("minor JNI version"),
	)
}

fn suffix(version: Version) -> String {
	match version {
		RESERVED => "reserved".to_owned(),
		(major, 0) => major.to_string(),
		(major, minor) => format!("{major}_{minor}"),
	}
}
