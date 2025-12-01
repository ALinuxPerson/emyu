use std::mem;
use crate::crate_;
use convert_case::ccase;
use darling::FromMeta;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Block, Field, FieldMutability, FnArg, Generics, ImplItem, ItemImpl, Pat, PatIdent,
    PatType, PathArguments, ReturnType, Token, Type, TypePath, Visibility,
};

#[derive(FromMeta)]
#[darling(derive_syn_parse)]
pub struct DispatcherArgs {}

struct DispatcherContext {
    crate_: TokenStream,
    attrs: Vec<Attribute>,
    model_ty: Type,
    items: Vec<DispatcherItem>,
}

impl DispatcherContext {
    fn new(value: ItemImpl) -> syn::Result<Self> {
        Ok(Self {
            crate_: crate_(),
            attrs: value.attrs,
            model_ty: *value.self_ty,
            items: value
                .items
                .into_iter()
                .map(DispatcherItem::new)
                .collect::<syn::Result<Vec<_>>>()?,
        })
    }

    fn expand(self) -> syn::Result<TokenStream> {
        let model_ty = &self.model_ty;
        let items = self
            .items
            .into_iter()
            .map(|item| item.expand(&self.crate_, model_ty))
            .collect::<syn::Result<Vec<_>>>()?;

        Ok(quote! {
            #(#items)*
        })
    }
}

enum DispatcherItemKind {
    Updater,
    Getter { data_ty: Box<Type> },
}

struct DispatcherItem {
    kind: DispatcherItemKind,
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,

    // todo: generics support for dispatchers
    generics: Generics,

    inputs: Punctuated<FnArg, Token![,]>,
    block: Block,
}

impl DispatcherItem {
    fn new(value: ImplItem) -> syn::Result<Self> {
        let value = match value {
            ImplItem::Fn(value) => value,
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "Only functions are allowed in `#[vye::dispatcher]` blocks",
                ));
            }
        };

        // if &mut self, then updater; if &self, then getter
        let mut kind = None;
        for input in &value.sig.inputs {
            if let FnArg::Receiver(receiver) = input {
                if receiver.mutability.is_some() {
                    kind = Some(DispatcherItemKind::Updater);
                    break;
                } else {
                    let data_ty = match &value.sig.output {
                        ReturnType::Type(_, ty) => ty.clone(),
                        ReturnType::Default => {
                            return Err(syn::Error::new_spanned(
                                value.sig.output,
                                "Getter functions must have a return type",
                            ));
                        }
                    };

                    kind = Some(DispatcherItemKind::Getter { data_ty });
                }
            }
        }
        let kind = kind.ok_or_else(|| {
            syn::Error::new_spanned(
                &value.sig.inputs,
                "Dispatcher functions must have a self parameter",
            )
        })?;

        Ok(Self {
            kind,
            attrs: value.attrs,
            vis: value.vis,
            name: value.sig.ident,
            generics: value.sig.generics,
            inputs: value.sig.inputs,
            block: value.block,
        })
    }
}

// must be &mut UpdateContext<App>
fn is_update_context(pat_ty: &PatType) -> Option<&Ident> {
    if let Pat::Ident(PatIdent { ident, .. }) = &*pat_ty.pat
        && let Type::Reference(ty) = &*pat_ty.ty
        && ty.mutability.is_some()
        && let Type::Path(TypePath { path, .. }) = &*ty.elem
        && let Some(segment) = path.segments.last()
        && matches!(segment.arguments, PathArguments::AngleBracketed(_))
        && segment.ident == "UpdateContext"
    {
        Some(ident)
    } else {
        None
    }
}

impl DispatcherItem {
    fn make_fields(&mut self, ctx_name: &mut Option<Ident>) -> Vec<Field> {
        mem::take(&mut self.inputs)
            .into_iter()
            .filter_map(|fn_arg| match fn_arg {
                // self type, skip
                FnArg::Receiver(_) => None,
                FnArg::Typed(pat_type) => {
                    // `&mut UpdateContext<App>`, skip
                    if let DispatcherItemKind::Updater = self.kind
                        && let Some(ident) = is_update_context(&pat_type)
                    {
                        *ctx_name = Some(ident.clone());
                        return None;
                    }

                    // todo: more sophisticated error handling for this case
                    let Pat::Ident(ident) = *pat_type.pat else {
                        return None;
                    };
                    let ident_span = ident.span();
                    Some(Field {
                        attrs: pat_type.attrs,

                        // todo: make visibility configurable via proc macro attribute
                        vis: Visibility::Inherited,

                        mutability: FieldMutability::None,
                        ident: Some(ident.ident),
                        colon_token: Some(Token![:](ident_span)),
                        ty: *pat_type.ty,
                    })
                }
            })
            .collect()
    }

    fn expand(mut self, crate_: &TokenStream, model_ty: &Type) -> syn::Result<TokenStream> {
        let mut ctx_name = None;
        let fields = self.make_fields(&mut ctx_name);
        let field_names = fields
            .iter()
            .map(|f| f.ident.as_ref().expect("expected ident for field to exist"))
            .collect::<Vec<_>>();
        let name = Ident::new(&ccase!(pascal, self.name.to_string()), Span::call_site());
        let attrs = &self.attrs;
        let vis = &self.vis;
        let block = &self.block;
        let struct_decl = quote! {
            #(#attrs)*
            #vis struct #name {
                // todo: add ability to specify visibility and attributes of fields
                #(#fields),*
            }
        };
        match self.kind {
            DispatcherItemKind::Updater => Ok(quote! {
                #struct_decl
                impl #crate_::ModelMessage for #name {}
                impl #crate_::ModelHandler<#name> for #model_ty {
                    fn update(
                        &mut self,
                        #name { #(#field_names),* }: #name,
                        #ctx_name: &mut #crate_::UpdateContext<<#model_ty as #crate_::Model>::ForApp>,
                    ) {
                        #block
                    }
                }
            }),
            DispatcherItemKind::Getter { data_ty } => Ok(quote! {
                #struct_decl
                impl #crate_::ModelGetterMessage for #name {
                    type Data = #data_ty;
                }
                impl #crate_::ModelGetterHandler<#name> for #model_ty {
                    fn getter(&self, #name { #(#field_names),* }: #name) -> #data_ty {
                        #block
                    }
                }
            })
        }
    }
}

pub fn build(value: ItemImpl) -> syn::Result<TokenStream> {
    DispatcherContext::new(value)?.expand()
}
