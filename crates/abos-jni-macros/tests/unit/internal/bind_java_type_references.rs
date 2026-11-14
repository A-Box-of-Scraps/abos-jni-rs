use super::{
	Ident, IsInstanceOfEntry, TokenStream, TypeMappings, generate_is_instance_of_assignable_checks,
	generate_is_instance_of_code, quote,
};

fn entry(alias: &str, stem: Option<&str>) -> IsInstanceOfEntry {
	IsInstanceOfEntry {
		type_alias: alias.to_owned(),
		stem: stem.map(str::to_owned),
	}
}

#[test]
fn empty_is_instance_of_generates_no_checks_or_conversions() {
	let type_name: Ident = syn::parse_quote!(Example);
	let jni: syn::Path = syn::parse_quote!(::renamed_jni);
	let mappings: TypeMappings = TypeMappings::new(&jni);
	let checks: TokenStream =
		generate_is_instance_of_assignable_checks(&type_name, "com.example.Example", &[], &mappings, &jni)
			.expect("empty checks");
	assert!(checks.is_empty());
	let code: TokenStream = generate_is_instance_of_code(&type_name, &[], &mappings, &jni).expect("empty code");
	assert_eq!(code.to_string(), quote! { impl<'local> Example<'local> {} }.to_string());
}

#[test]
fn is_instance_of_generates_named_casts_and_conversions() {
	let type_name: Ident = syn::parse_quote!(Example);
	let jni: syn::Path = syn::parse_quote!(::renamed_jni);
	let mappings: TypeMappings = TypeMappings::new(&jni);
	let entries: [IsInstanceOfEntry; 2] = [entry("JList", Some("list")), entry("JCollection", None)];
	let code: TokenStream = generate_is_instance_of_code(&type_name, &entries, &mappings, &jni).expect("valid code");
	let file: syn::File = syn::parse2(code).expect("valid Rust");
	assert_eq!(file.items.len(), 5);
	let syn::Item::Impl(methods) = &file.items[0] else {
		panic!("expected inherent impl");
	};
	assert_eq!(methods.items.len(), 1);
	let syn::ImplItem::Fn(method) = &methods.items[0] else {
		panic!("expected cast method");
	};
	assert_eq!(method.sig.ident, "as_list");
	let output: &syn::ReturnType = &method.sig.output;
	let expected: syn::ReturnType =
		syn::parse_quote!(-> ::renamed_jni::refs::Cast<'local, '_, ::renamed_jni::objects::JList<'local>>);
	assert_eq!(quote!(#output).to_string(), quote!(#expected).to_string());
	let (conversions, _) = file.items[1..].as_chunks::<2>();
	for (items, target) in conversions.iter().zip([
		syn::parse_quote!(::renamed_jni::objects::JList),
		syn::parse_quote!(::renamed_jni::objects::JCollection),
	]) {
		assert_conversion_impls(items, &target);
	}
}

fn assert_conversion_impls(items: &[syn::Item], target: &syn::Path) {
	let expected: syn::File = syn::parse_quote! {
		impl<'local> ::core::convert::From<Example<'local>> for #target<'local> {
			fn from(value: Example<'local>) -> #target<'local> {
				let raw = value.into_raw();
				unsafe { <#target as ::renamed_jni::refs::Reference>::kind_from_raw(raw) }
			}
		}
		impl<'local> ::core::convert::AsRef<#target<'local>> for Example<'local> {
			fn as_ref(&self) -> &#target<'local> {
				const fn assert_is_instance_of_type_is_ffi_safe<T: ::renamed_jni::refs::TransparentReference>() {}
				const _: () = assert_is_instance_of_type_is_ffi_safe::<#target<'_>>();
				unsafe { &*(self as *const Self).cast::<#target<'local>>() }
			}
		}
	};
	assert_eq!(quote!(#(#items)*).to_string(), quote!(#expected).to_string());
}

#[test]
fn jobject_entries_do_not_duplicate_base_conversion_impls() {
	let type_name: Ident = syn::parse_quote!(Example);
	let jni: syn::Path = syn::parse_quote!(::abos_jni);
	let mappings: TypeMappings = TypeMappings::new(&jni);
	for alias in ["JObject", "abos_jni::objects::JObject"] {
		let code: TokenStream =
			generate_is_instance_of_code(&type_name, &[entry(alias, Some("object"))], &mappings, &jni)
				.expect("JObject conversion");
		let file: syn::File = syn::parse2(code).expect("valid Rust");
		assert_eq!(file.items.len(), 1);
		let syn::Item::Impl(methods) = &file.items[0] else {
			panic!("expected inherent impl");
		};
		assert_eq!(methods.items.len(), 1);
	}
}

#[test]
fn assignability_checks_preserve_order_and_exception_guard() {
	let type_name: Ident = syn::parse_quote!(Example);
	let jni: syn::Path = syn::parse_quote!(::renamed_jni);
	let mappings: TypeMappings = TypeMappings::new(&jni);
	let entries: [IsInstanceOfEntry; 2] = [entry("JList", None), entry("JCollection", None)];
	let checks: TokenStream =
		generate_is_instance_of_assignable_checks(&type_name, "com.example.Example", &entries, &mappings, &jni)
			.expect("valid checks");
	let body: syn::Block = syn::parse2(quote! {{ #checks }}).expect("valid check blocks");
	assert_eq!(body.stmts.len(), 2);
	for (statement, (target, java_class)) in body.stmts.iter().zip([
		(syn::parse_quote!(::renamed_jni::objects::JList), "java.util.List"),
		(
			syn::parse_quote!(::renamed_jni::objects::JCollection),
			"java.util.Collection",
		),
	]) {
		assert_assignability_check(statement, &target, java_class);
	}
}

fn assert_assignability_check(statement: &syn::Stmt, target: &syn::Path, java_class: &str) {
	let target_string: String = quote!(#target).to_string().replace(' ', "");
	assert_eq!(
		quote!(#statement).to_string(),
		quote! {
			{
				let is_instance_of_class = <#target as ::renamed_jni::refs::Reference>::lookup_class(env, loader)?;
				let is_instance_of_class: &::renamed_jni::objects::JClass = unsafe { is_instance_of_class.as_ref() };
				if env.exception_check() {
					return Err(::renamed_jni::errors::Error::JavaException);
				}
				let is_assignable = unsafe {
					use ::renamed_jni::refs::Reference as _;
					let env_ptr: *mut ::renamed_jni::sys::JNIEnv = env.get_raw();
					let interface: *const ::renamed_jni::sys::JNINativeInterface_ = *env_ptr;
					((*interface).v1_1.IsAssignableFrom)(env_ptr, class.as_raw(), is_instance_of_class.as_raw(),)
				};
				assert!(
					is_assignable != ::renamed_jni::sys::JNI_FALSE,
					"{}({}) is not a subtype of {}({}) and is not a valid 'is_instance_of' type for {}",
					#target_string,
					#java_class,
					stringify!(Example),
					"com.example.Example",
					stringify!(Example)
				);
			}
		}
		.to_string()
	);
}

#[test]
fn is_instance_of_generators_report_the_same_mapping_errors() {
	let type_name: Ident = syn::parse_quote!(Example);
	let jni: syn::Path = syn::parse_quote!(::renamed_jni);
	let mut mappings: TypeMappings = TypeMappings::new(&jni);
	mappings
		.insert_ref_type(
			"Invalid",
			"()",
			syn::parse_str("com.example.Invalid").expect("Java class"),
			false,
		)
		.expect("mapping with invalid Rust path");
	for (alias, expected) in [
		("jint", "Primitive type 'jint' cannot be used in is_instance_of"),
		(
			"Missing",
			"Type 'Missing' used in is_instance_of is not defined in types mapping",
		),
		("Invalid", "Invalid Rust type path: ()"),
	] {
		let entries: [IsInstanceOfEntry; 1] = [entry(alias, None)];
		let checks: syn::Result<TokenStream> =
			generate_is_instance_of_assignable_checks(&type_name, "com.example.Example", &entries, &mappings, &jni);
		let code: syn::Result<TokenStream> = generate_is_instance_of_code(&type_name, &entries, &mappings, &jni);
		assert_eq!(checks.expect_err("invalid mapping").to_string(), expected);
		assert_eq!(code.expect_err("invalid mapping").to_string(), expected);
	}
}

#[test]
fn is_instance_of_resolves_custom_aliases_to_canonical_paths() {
	let type_name: Ident = syn::parse_quote!(Example);
	let jni: syn::Path = syn::parse_quote!(::renamed_jni);
	let mut mappings: TypeMappings = TypeMappings::new(&jni);
	mappings
		.insert_ref_type(
			"ParentAlias",
			"crate::Parent",
			syn::parse_str("com.example.Parent").expect("Java class"),
			false,
		)
		.expect("custom mapping");
	let entries: [IsInstanceOfEntry; 1] = [entry("ParentAlias", Some("parent"))];
	let target: syn::Path = syn::parse_quote!(crate::Parent);
	let checks: TokenStream =
		generate_is_instance_of_assignable_checks(&type_name, "com.example.Example", &entries, &mappings, &jni)
			.expect("custom assignability check");
	let body: syn::Block = syn::parse2(quote! {{ #checks }}).expect("valid check block");
	assert_eq!(body.stmts.len(), 1);
	assert_assignability_check(&body.stmts[0], &target, "com.example.Parent");
	let code: TokenStream =
		generate_is_instance_of_code(&type_name, &entries, &mappings, &jni).expect("custom cast and conversions");
	let file: syn::File = syn::parse2(code).expect("valid Rust");
	assert_eq!(file.items.len(), 3);
	assert_conversion_impls(&file.items[1..], &target);
}
