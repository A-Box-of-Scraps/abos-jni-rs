use syn::{Ident, Token};

/// Default crate path when no `jni = <path>` override is supplied.
fn jni_crate_default() -> syn::Path {
	syn::parse_quote!(::abos_jni)
}

pub fn parse_jni_crate_override(input: syn::parse::ParseStream) -> Result<syn::Path, syn::Error> {
	if !input.peek(Ident) {
		return Ok(jni_crate_default());
	}
	let fork: syn::parse::ParseBuffer<'_> = input.fork();
	let property: Ident = fork.parse()?;
	if property != "jni" || !fork.peek(Token![=]) {
		return Ok(jni_crate_default());
	}
	input.parse::<Ident>()?;
	input.parse::<Token![=]>()?;
	let path: syn::Path = input.parse()?;
	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}
	Ok(path)
}
