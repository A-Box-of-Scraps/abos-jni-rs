use super::{TokenStream, bind_java_type_impl};
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
