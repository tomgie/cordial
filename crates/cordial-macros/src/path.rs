//! The path of the framework crate in the code of the user.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

/// The paths that the generated code uses.
pub(crate) struct Paths {
    /// The framework crate.
    pub(crate) framework: TokenStream,
    /// The model crate, through the framework.
    pub(crate) model: TokenStream,
}

impl Paths {
    /// Finds the paths. An override from a `crate = "..."` key has priority.
    pub(crate) fn new(framework_override: Option<syn::Path>) -> Self {
        let framework = match framework_override {
            Some(path) => quote!(#path),
            None => find_framework(),
        };
        let model = quote!(#framework::__private::model);
        Self { framework, model }
    }
}

/// Finds the framework crate in the manifest of the user.
///
/// A direct dependency on `cordial-framework` has priority, because the facade can have the
/// `framework` feature off.
fn find_framework() -> TokenStream {
    match crate_name("cordial-framework") {
        Ok(FoundCrate::Itself) => return quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            return quote!(::#ident);
        }
        Err(_) => {}
    }

    match crate_name("cordial") {
        Ok(FoundCrate::Itself) => quote!(crate::framework),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident::framework)
        }
        Err(_) => quote!(::cordial_framework),
    }
}
