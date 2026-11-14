use super::{AbiCheck, BindClassInput, TokenStream, VisibilitySpec, bind_java_type_impl};
use quote::quote;
use std::collections::BTreeMap;

fn documentation(attrs: &[syn::Attribute]) -> String {
	let attr: &syn::Attribute = attrs
		.iter()
		.find(|attr| attr.path().is_ident("doc"))
		.expect("documentation");
	let value: &syn::Expr = &attr.meta.require_name_value().expect("doc value").value;
	let syn::Expr::Lit(syn::ExprLit {
		lit: syn::Lit::Str(doc),
		..
	}) = value
	else {
		panic!("generated documentation must be a string literal, not a downstream macro call");
	};
	doc.value()
}

fn generated_documentation(tokens: TokenStream) -> BTreeMap<String, String> {
	let file: syn::File = syn::parse2(tokens).expect("generated Rust");
	let mut docs: BTreeMap<String, String> = BTreeMap::new();
	for item in file.items {
		if let syn::Item::Struct(item) = &item
			&& item.ident == "Example"
		{
			docs.insert(item.ident.to_string(), documentation(&item.attrs));
			continue;
		}
		let syn::Item::Impl(item) = item else { continue };
		for item in item.items {
			let syn::ImplItem::Fn(method) = item else { continue };
			if method.attrs.iter().any(|attr| attr.path().is_ident("doc")) {
				docs.insert(method.sig.ident.to_string(), documentation(&method.attrs));
			}
		}
	}
	docs
}

#[test]
fn generated_documentation_preserves_formatting_and_resolves_names() {
	let tokens: TokenStream = bind_java_type_impl(quote! {
		jni = ::renamed_jni,
		rust_type = Example,
		java_type = "com.example.Example",
		is_instance_of = { list = JList },
	})
	.expect("valid binding");
	let docs: BTreeMap<String, String> = generated_documentation(tokens);
	assert_eq!(docs.len(), 6);
	assert_eq!(
		docs["Example"],
		indoc::indoc! {"
			A `com.example.Example` reference, tied to a JNI local reference frame.

			See the [`JObject`] documentation for more information about object references,
			how to cast them, and local reference frame lifetimes.

			[`JObject`]: ::renamed_jni::objects::JObject
		"}
	);
	assert!(docs["from_raw"].starts_with("Creates a [`Example`] that wraps the given `raw` [jobject]\n\n# Safety\n"));
	assert!(
		docs["from_raw"]
			.lines()
			.any(|line| line == "  JNI stack frame associated with the [Env] `'local` lifetime.")
	);
	assert!(docs["from_raw"].ends_with("[Env]: ::renamed_jni::Env\n"));
	assert_eq!(
		docs["null"],
		indoc::indoc! {"
			Creates a new null reference.

			Null references are always valid and do not belong to a local reference frame. Therefore,
			the returned [`Example`] always has the `'static` lifetime."}
	);
	assert!(docs["cast_local"].starts_with("Cast a local reference to a [`Example`]\n"));
	assert!(
		docs["cast_local"]
			.lines()
			.any(|line| line == "- [Env::as_cast](::renamed_jni::Env::as_cast)")
	);
	assert!(docs["cast_local"].ends_with("[Error::WrongObjectType]: ::renamed_jni::errors::Error::WrongObjectType\n"));
	assert_eq!(
		docs["as_list"],
		indoc::indoc! {"
			Casts this `Example` to a `::renamed_jni::objects::JList`

			This does not require a runtime type check since any `Example` is also a `::renamed_jni::objects::JList`"}
	);
	assert!(docs.values().all(|doc| !doc.contains('\t')));
}

#[test]
fn reference_validation_caches_are_per_binding_and_honor_the_jni_path() {
	let tokens: TokenStream = bind_java_type_impl(quote! {
		jni = ::renamed_jni,
		rust_type = Example,
		java_type = "com.example.Example",
		constructors { fn new(value: JString), },
		methods {
			fn consume(value: JString) -> jint,
			static fn compare(first: JObject, second: JObject) -> jint,
			fn primitive(value: jint) -> jint,
		},
	})
	.expect("valid binding");
	let file: syn::File = syn::parse2(tokens).expect("generated Rust");
	let mut cached: Vec<String> = Vec::new();
	for item in file.items {
		let syn::Item::Impl(item) = item else { continue };
		for item in item.items {
			let syn::ImplItem::Fn(method) = item else { continue };
			if let Some(ty) = reference_cache_type(&method) {
				assert_eq!(quote!(#ty).to_string(), ":: renamed_jni :: __MethodReferenceCache");
				cached.push(method.sig.ident.to_string());
			}
		}
	}
	cached.sort();
	assert_eq!(cached, ["compare", "consume", "new"]);
}

fn reference_cache_type(method: &syn::ImplItemFn) -> Option<&syn::Type> {
	method.block.stmts.iter().find_map(|statement| match statement {
		syn::Stmt::Item(syn::Item::Static(item)) if item.ident == "__JNI_REFERENCE_TYPES" => Some(item.ty.as_ref()),
		_ => None,
	})
}

#[test]
fn class_properties_parse_type_names_and_blocks() {
	let binding: BindClassInput = syn::parse2(quote! {
		jni = ::renamed_jni,
		#[doc = "Example binding"]
		rust_type = Example,
		rust_type_vis = pub,
		java_type = "com.example.Example",
		api = ExampleApi,
		priv_type = ExamplePriv,
		native_trait = ExampleNative,
		is_instance_of = { list = JList },
		type_map = {},
		constructors {},
		methods = {},
		fields {},
		native_methods = {},
	})
	.expect("type names and blocks should parse");
	assert_eq!(binding.type_name, "Example");
	assert!(matches!(binding.type_visibility, Some(VisibilitySpec::Public)));
	assert_eq!(documentation(&binding.type_attrs), "Example binding");
	assert_eq!(binding.api_name.expect("API name"), "ExampleApi");
	assert_eq!(binding.priv_type.expect("private type"), "ExamplePriv");
	assert_eq!(binding.native_trait_name.expect("native trait"), "ExampleNative");
	assert_eq!(binding.is_instance_of.len(), 1);
	assert_eq!(binding.is_instance_of[0].stem.as_deref(), Some("list"));
	let jni_crate: syn::Path = binding.jni_crate;
	assert_eq!(quote!(#jni_crate).to_string(), ":: renamed_jni");
}

#[test]
fn class_properties_parse_options_and_private_names() {
	let binding: BindClassInput = syn::parse2(quote! {
		Example => com.example.Example,
		hooks { load_class = |env| env, init_priv = || () },
		native_methods_export = false,
		native_methods_error_policy = crate::ErrorPolicy,
		abi_check = Always,
		native_methods_catch_unwind = false,
		__jni_core = true,
		__sys_type = jobject,
	})
	.expect("options and private properties should parse");
	assert!(binding.load_class_closure.is_some());
	assert!(binding.init_priv_closure.is_some());
	assert!(!binding.native_methods_export);
	assert!(!binding.native_methods_catch_unwind);
	assert!(matches!(binding.abi_check, AbiCheck::Always));
	let error_policy: syn::Path = binding.native_methods_error_policy.expect("error policy");
	assert_eq!(quote!(#error_policy).to_string(), "crate :: ErrorPolicy");
	assert!(binding.jni_core);
	assert_eq!(binding.sys_type.expect("JNI type"), "jobject");
}

#[test]
fn unknown_class_properties_suggest_only_public_names() {
	let error: syn::Error = syn::parse2::<BindClassInput>(quote! { unknown = true })
		.err()
		.expect("unknown property should fail");
	assert_eq!(
		error.to_string(),
		concat!(
			"expected one of: `rust_type`, `rust_type_vis`, `java_type`, `api`, `priv_type`, ",
			"`native_trait`, `is_instance_of`, `type_map`, `hooks`, `constructors`, `methods`, ",
			"`fields`, `native_methods`, `native_methods_export`, `native_methods_error_policy`, ",
			"`abi_check`, `native_methods_catch_unwind`"
		)
	);
}

#[test]
fn class_properties_preserve_duplicate_and_misplaced_jni_errors() {
	for (tokens, expected) in [
		(
			quote! { rust_type = Example, rust_type = Other },
			"Rust type name already specified",
		),
		(
			quote! { java_type = "com.example.Example", java_type = "com.example.Other" },
			"Java class name already specified",
		),
		(
			quote! { rust_type_vis = pub, rust_type_vis = pub(crate) },
			"Type visibility already specified",
		),
		(
			quote! { rust_type = Example, jni = ::renamed_jni },
			"jni property must be the first property if specified",
		),
	] {
		let error: syn::Error = syn::parse2::<BindClassInput>(tokens)
			.err()
			.expect("invalid property should fail");
		assert_eq!(error.to_string(), expected);
	}
}

#[test]
fn class_property_keywords_preserve_identifier_errors() {
	for (tokens, keyword) in [
		(quote! { static = true }, "static"),
		(quote! { fn {} }, "fn"),
		(quote! { self = Example }, "self"),
	] {
		let error: syn::Error = syn::parse2::<BindClassInput>(tokens)
			.err()
			.expect("keyword property should fail");
		assert_eq!(
			error.to_string(),
			format!("expected identifier, found keyword `{keyword}`")
		);
	}
}
