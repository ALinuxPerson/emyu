use darling::FromAttributes;
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{quote, ToTokens};
use syn::{
    Attribute, Block, Signature, Token, Type, Visibility, braced,
    parse::{Parse, ParseStream},
};
use crate::utils;

pub struct MaybeStubFn {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    _semi_token: Option<Token![;]>,
    pub block: Option<Block>,
}

impl Parse for MaybeStubFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse standard parts: attributes, visibility, signature
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let sig: Signature = input.parse()?;

        // LOOKAHEAD: Check if the next token is a semicolon
        if input.peek(Token![;]) {
            let semi_token: Token![;] = input.parse()?;
            Ok(MaybeStubFn {
                attrs,
                vis,
                sig,
                _semi_token: Some(semi_token),
                block: None,
            })
        } else {
            // Otherwise, expect a standard block
            let block: Block = input.parse()?;
            Ok(MaybeStubFn {
                attrs,
                vis,
                sig,
                _semi_token: None,
                block: Some(block),
            })
        }
    }
}

pub struct InterfaceImpl {
    pub vis: Visibility,
    pub self_ty: Type,
    pub items: Vec<MaybeStubFn>,
}

impl Parse for InterfaceImpl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.call(Attribute::parse_outer)?;
        let vis = input.parse::<Visibility>()?;
        input.parse::<Token![impl]>()?;
        let self_ty: Type = input.parse()?;
        let content;
        braced!(content in input);

        let mut items = Vec::new();
        while !content.is_empty() {
            items.push(content.parse()?);
        }

        Ok(InterfaceImpl { vis, self_ty, items })
    }
}

/// Parses attributes into type T, returning the parsed value and the remaining attributes
/// (excluding the ones consumed by T, marked by "emyu").
pub fn extract_emyu_attrs<T: FromAttributes>(
    attributes: &[Attribute],
) -> syn::Result<(Vec<&Attribute>, T)> {
    let value = T::from_attributes(attributes)?;
    let remaining_attributes = attributes
        .iter()
        .filter(|attr| !attr.path().is_ident("emyu"))
        .collect();
    Ok((remaining_attributes, value))
}

#[derive(Clone)]
pub struct ThisCrate(TokenStream);

impl Default for ThisCrate {
    fn default() -> Self {
        match crate_name("emyu").expect("`emyu` crate should be present in `Cargo.toml`") {
            FoundCrate::Itself => Self(quote! { crate }),
            FoundCrate::Name(name) => {
                let ident = Ident::new(&name, Span::call_site());
                Self(quote! { #ident })
            }
        }
    }
}

impl ToTokens for ThisCrate {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

pub fn frb(tokens: TokenStream, crate_: &ThisCrate) -> TokenStream {
    quote! { #crate_::__macros::flutter_rust_bridge::frb(#tokens) }
}

pub fn frb_sync(crate_: &ThisCrate) -> TokenStream {
    frb(quote! { sync }, crate_)
}

pub fn frb_opaque(crate_: &ThisCrate) -> TokenStream {
    utils::frb(quote! { opaque }, crate_)
}


