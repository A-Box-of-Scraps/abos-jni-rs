use syn::{Ident, Token};

/// The default name for the `abos-jni` crate
///
/// Note: This used to be resolved using `proc_macro_crate` but we later decided
/// to remove the dependency (it brings in numerous sub dependencies and adds
/// complexity that's not strictly necessary if the `jni=<path>` can be used to
/// specify the path explicitly). It should be very rare to need a non-default
/// path.
///
/// We now assume that the name will either be overridden explicitly or else we
/// use the default '`abos_jni`' name
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
