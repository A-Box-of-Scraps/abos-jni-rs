use std::{fmt::Write, fs, path::Path};

use quote::quote;
use syn::{Expr, ImplItem, Item, Token, punctuated::Punctuated};

pub fn generate(out: &Path) {
	let source = "../../crates/abos-jvmti-sys/src/internal/capability_bits.rs";
	println!("cargo:rerun-if-changed={source}");
	let file = syn::parse_file(&fs::read_to_string(source).expect("capability accessors")).expect("parse accessors");
	let mut setters = String::new();
	let mut getters = String::new();
	let mut rust = proc_macro2::TokenStream::new();
	let mut index = 0;
	for item in file.items {
		let Item::Impl(block) = item else { continue };
		for item in block.items {
			let ImplItem::Macro(item) = item else {
				panic!("Expected capability macro")
			};
			assert!(item.mac.path.is_ident("capability"));
			let args = item
				.mac
				.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
				.expect("capability args");
			let [Expr::Path(getter), Expr::Path(setter), Expr::Lit(_)] = args.iter().collect::<Vec<_>>()[..] else {
				panic!("Expected getter, setter, bit");
			};
			let getter = getter.path.get_ident().expect("getter name");
			let setter = setter.path.get_ident().expect("setter name");
			writeln!(getters, "case {index}: return caps->{getter};").expect("write getter");
			writeln!(setters, "case {index}: caps->{getter} = value; break;").expect("write setter");
			rust.extend(quote!((stringify!(#getter), sys::jvmtiCapabilities::#getter as Getter, sys::jvmtiCapabilities::#setter as Setter),));
			index += 1;
		}
	}
	assert_eq!(index, 45, "All JDK 25 capabilities must be tested");
	fs::write(out.join("getters.inc"), getters).expect("write C getters");
	fs::write(out.join("setters.inc"), setters).expect("write C setters");
	fs::write(out.join("capabilities.rs"), quote!([#rust]).to_string()).expect("write Rust capabilities");
}
