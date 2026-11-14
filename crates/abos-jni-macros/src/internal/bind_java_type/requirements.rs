use super::{Ident, ParseStream, Result, Token, TokenStream, format_ident, quote, requires};

/// Runtime availability of a binding member, independent of the JNI interface version.
#[derive(Clone, Default)]
pub(super) enum Requirement {
	#[default]
	Required,
	Available,
	Predicate(syn::Expr),
	/// A callable native binding reuses its registration result, not a second predicate evaluation.
	RegisteredNative(Ident),
}

impl Requirement {
	pub(super) fn parse_property(input: ParseStream) -> Result<Self> {
		input.parse::<requires>()?;
		input.parse::<Token![=]>()?;
		let expression: syn::Expr = input.parse()?;
		if matches!(&expression, syn::Expr::Path(path) if path.path.is_ident("available")) {
			Ok(Self::Available)
		} else {
			Ok(Self::Predicate(expression))
		}
	}

	pub(super) const fn is_required(&self) -> bool {
		matches!(self, Self::Required)
	}

	pub(super) fn native_flag(name: &Ident) -> Ident {
		format_ident!("jni_native_{}_available", name, span = proc_macro2::Span::mixed_site())
	}

	pub(super) fn for_native_call(&self, name: &Ident) -> Self {
		if self.is_required() {
			Self::Required
		} else {
			Self::RegisteredNative(Self::native_flag(name))
		}
	}

	pub(super) fn id_type(&self, ty: &TokenStream) -> TokenStream {
		if self.is_required() {
			quote! { #ty }
		} else {
			quote! { ::core::option::Option<#ty> }
		}
	}

	pub(super) fn initialize_id(&self, lookup: &TokenStream, jni: &syn::Path) -> TokenStream {
		match self {
			Self::Required => quote! { #lookup? },
			Self::Available => quote! {
				match #lookup {
					Ok(id) => Some(id),
					Err(#jni::errors::Error::MethodNotFound { .. } | #jni::errors::Error::FieldNotFound { .. }) => None,
					Err(error) => return Err(error),
				}
			},
			Self::Predicate(_) | Self::RegisteredNative(_) => {
				let enabled: TokenStream = self.condition();
				quote! { if #enabled { Some(#lookup?) } else { None } }
			}
		}
	}

	pub(super) fn condition(&self) -> TokenStream {
		match self {
			Self::Required | Self::Available => quote! { true },
			Self::RegisteredNative(flag) => quote! { #flag },
			Self::Predicate(predicate) => quote! { Self::__check_requirement(env, #predicate)? },
		}
	}

	pub(super) fn local_id(field: &Ident) -> Ident {
		format_ident!("jni_{}", field, span = proc_macro2::Span::mixed_site())
	}

	pub(super) fn access_id(&self, field: &Ident, name: &str, jni: &syn::Path) -> TokenStream {
		let local: Ident = Self::local_id(field);
		if self.is_required() {
			quote! { let #local = api.#field; }
		} else {
			quote! {
				if env.exception_check() { return Err(#jni::errors::Error::JavaException); }
				let #local = api.#field.ok_or(#jni::errors::Error::MemberUnavailable { name: #name })?;
			}
		}
	}
}

/// Type predicate closures without introducing helper names into the user's expression scope.
pub(super) fn generate_requirement_checker(jni: &syn::Path) -> TokenStream {
	quote! {
		#[allow(dead_code, reason = "Only runtime predicate requirements use this helper.")]
		fn __check_requirement<'local, F>(env: &mut #jni::Env<'local>, predicate: F) -> #jni::errors::Result<bool>
		where F: ::core::ops::FnOnce(&mut #jni::Env<'local>) -> #jni::errors::Result<bool> {
			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}
			let enabled = predicate(env)?;
			if env.exception_check() {
				return Err(#jni::errors::Error::JavaException);
			}
			Ok(enabled)
		}
	}
}

#[cfg(test)]
#[path = "../../../tests/unit/internal/bind_requirements.rs"]
mod tests;
