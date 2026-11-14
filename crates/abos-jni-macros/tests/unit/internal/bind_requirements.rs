use super::{Requirement, TokenStream, quote};
use crate::internal::bind_java_type::{BindClassInput, bind_java_type_impl};

fn parse_requirements() -> BindClassInput {
	syn::parse2(quote! {
		Example => example.Example,
		constructors {
			fn new(),
			fn optional { requires = available, sig = (jint) },
		},
		methods {
			fn required(),
			fn optional { sig = (), requires = available },
			fn gated { requires = |_| Ok(false), sig = () },
		},
		fields {
			required: jint,
			optional { requires = available, sig = jint },
			gated { sig = jint, requires = |_| Ok(true) },
		},
		native_methods {
			pub fn optional { requires = available, sig = () },
			pub static fn gated { sig = (), requires = gate },
		},
	})
	.expect("requirements are accepted on all members")
}

#[test]
fn constructors_parse_requirements() {
	let input: BindClassInput = parse_requirements();
	assert!(input.constructors[0].requirement.is_required());
	assert!(matches!(input.constructors[1].requirement, Requirement::Available));
}

#[test]
fn methods_parse_requirements() {
	let input: BindClassInput = parse_requirements();
	assert!(input.methods[0].requirement.is_required());
	assert!(matches!(input.methods[1].requirement, Requirement::Available));
	assert!(matches!(input.methods[2].requirement, Requirement::Predicate(_)));
}

#[test]
fn fields_parse_requirements() {
	let input: BindClassInput = parse_requirements();
	assert!(input.fields[0].requirement.is_required());
	assert!(matches!(input.fields[1].requirement, Requirement::Available));
	assert!(matches!(input.fields[2].requirement, Requirement::Predicate(_)));
}

#[test]
fn native_call_wrappers_reuse_the_registration_requirement() {
	let input: BindClassInput = parse_requirements();
	assert!(matches!(input.native_methods[0].requirement, Requirement::Available));
	assert!(matches!(input.native_methods[1].requirement, Requirement::Predicate(_)));
	for method in &input.methods[3..] {
		let Requirement::RegisteredNative(flag) = &method.requirement else {
			panic!("native call wrappers must reuse the registration decision");
		};
		assert_eq!(*flag, Requirement::native_flag(&method.rust_name));
	}
}

#[test]
fn gated_natives_do_not_export_symbols_that_bypass_registration() {
	let output: TokenStream = bind_java_type_impl(quote! {
		Example => example.Example,
		native_methods_export = true,
		native_methods {
			fn required(),
			fn optional { sig = (), requires = available },
			fn gated { sig = (), requires = |_| Ok(false) },
		},
	})
	.expect("gated natives suppress default exports");
	let file: syn::File = syn::parse2(output).expect("valid generated Rust");
	let exports: Vec<String> = file
		.items
		.iter()
		.filter_map(|item| match item {
			syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
			_ => None,
		})
		.collect();
	assert_eq!(exports.len(), 1);
	assert!(exports[0].contains("required"));
}

#[test]
fn explicit_exports_cannot_bypass_requirements() {
	for export in [quote!(true), quote!("custom_export")] {
		let error: syn::Error = bind_java_type_impl(quote! {
			Example => example.Example,
			native_methods { fn optional { sig = (), requires = available, export = #export } },
		})
		.expect_err("explicit export bypasses runtime registration requirements");
		assert!(
			error
				.to_string()
				.contains("Runtime-gated native methods cannot be exported")
		);
	}
}

#[test]
fn optional_ids_honor_the_jni_crate_override() {
	let output: TokenStream = bind_java_type_impl(quote! {
		jni = ::renamed_jni,
		Example => example.Example,
		constructors { fn new { sig = (), requires = available } },
		methods { fn value { sig = () -> jint, requires = |_| Ok(true) } },
		fields { field { sig = JObject, requires = available } },
	})
	.expect("requirements support renamed JNI crates");
	let text: String = output.to_string();
	assert!(text.contains("renamed_jni :: errors :: Error :: MemberUnavailable"));
	assert!(!text.contains("abos_jni"));
	syn::parse2::<syn::File>(output).expect("optional IDs generate valid Rust syntax");
}
