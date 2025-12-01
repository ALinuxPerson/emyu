use crate::crate_;
use convert_case::ccase;
use darling::{FromAttributes, FromMeta};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, quote};
use std::mem;
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Block, Field, FieldMutability, FnArg, Generics, ImplItem, ItemImpl, Pat, PatIdent,
    PatType, PathArguments, ReturnType, Token, Type, TypePath, Visibility,
};

fn parse_then_filter<T: FromAttributes>(
    attributes: &[Attribute],
) -> syn::Result<(Vec<&Attribute>, T)> {
    let value = T::from_attributes(&attributes)?;
    let attributes = attributes
        .iter()
        .filter(|attr| !attr.path().is_ident("vye"))
        .collect();
    Ok((attributes, value))
}

#[derive(FromMeta)]
struct GenerateSplitDispatcherArgs {
    #[darling(default)]
    vis: Option<Visibility>,

    #[darling(default)]
    updater: Option<Ident>,

    #[darling(default)]
    getter: Option<Ident>,
}

impl GenerateSplitDispatcherArgs {
    pub fn idents(&self, model_name: &Ident) -> (Ident, Ident) {
        let updater = self
            .updater
            .clone()
            .unwrap_or_else(|| Ident::new(&format!("{model_name}Updater"), Span::call_site()));
        let getter = self
            .getter
            .clone()
            .unwrap_or_else(|| Ident::new(&format!("{model_name}Getter"), Span::call_site()));
        (updater, getter)
    }
}

#[derive(FromMeta)]
struct GenerateNewDispatcherArgs {
    #[darling(default)]
    vis: Option<Visibility>,
}

#[derive(FromMeta)]
struct GenerateDispatcherArgs {
    #[darling(default)]
    dispatcher: Option<Ident>,

    #[darling(default)]
    new: Option<GenerateNewDispatcherArgs>,

    #[darling(default)]
    split: Option<GenerateSplitDispatcherArgs>,
}

impl GenerateDispatcherArgs {
    pub fn dispatcher(&self, model_name: &Ident) -> Ident {
        self.dispatcher
            .clone()
            .unwrap_or_else(|| Ident::new(&format!("{model_name}Dispatcher"), Span::call_site()))
    }
}

#[derive(FromMeta)]
#[darling(derive_syn_parse)]
pub struct DispatcherArgs {
    generate: Option<GenerateDispatcherArgs>,
}

struct DispatcherContext<'a> {
    args: DispatcherArgs,
    crate_: TokenStream,
    attrs: &'a [Attribute],
    model_ty: &'a Type,
    model_name: Ident,
    items: Vec<DispatcherItem<'a>>,
}

impl<'a> DispatcherContext<'a> {
    fn new(value: &'a ItemImpl, args: DispatcherArgs) -> syn::Result<Self> {
        let model_name = match &*value.self_ty {
            Type::Path(TypePath { path, .. }) => path
                .segments
                .last()
                .ok_or_else(|| {
                    syn::Error::new_spanned(&value.self_ty, "Provided type path has no segments")
                })?
                .ident
                .clone(),
            _ => {
                return Err(syn::Error::new_spanned(
                    &value.self_ty,
                    "Expected a type path for the model type",
                ));
            }
        };
        Ok(Self {
            args,
            crate_: crate_(),
            attrs: &value.attrs,
            model_ty: &value.self_ty,
            model_name,
            items: value
                .items
                .iter()
                .map(DispatcherItem::new)
                .collect::<syn::Result<Vec<_>>>()?,
        })
    }

    fn generate(&self) -> syn::Result<TokenStream> {
        let model_ty = &self.model_ty;
        let items = self
            .items
            .iter()
            .map(|item| item.generate(&self.crate_, model_ty))
            .collect::<syn::Result<Vec<_>>>()?;
        let items = quote! { #(#items)* };

        if let Some(args) = &self.args.generate {
            let wrapped_dispatcher = self.generate_wrapped_dispatcher(args)?;

            Ok(quote! {
                #items
                #wrapped_dispatcher
            })
        } else {
            Ok(items)
        }
    }
}

impl<'a> DispatcherContext<'a> {
    fn generate_wrapped_dispatcher(&self, args: &GenerateDispatcherArgs) -> syn::Result<TokenStream> {
        let dispatcher_name = args.dispatcher(&self.model_name);
        let model_ty = &self.model_ty;
        todo!()
    }
}

enum DispatcherItemKind {
    Updater,
    Getter { data_ty: Box<Type> },
}

#[derive(FromAttributes, Default)]
#[darling(attributes(vye))]
struct DispatcherItemArgs {
    #[darling(default)]
    name: Option<Ident>,

    #[darling(default)]
    dispatcher: Option<Visibility>,
}

struct DispatcherItem<'a> {
    args: DispatcherItemArgs,
    kind: DispatcherItemKind,
    attrs: Vec<&'a Attribute>,
    vis: &'a Visibility,
    name: &'a Ident,
    generics: &'a Generics,
    inputs: &'a Punctuated<FnArg, Token![,]>,
    block: &'a Block,
}

impl<'a> DispatcherItem<'a> {
    fn new(value: &'a ImplItem) -> syn::Result<Self> {
        let value = match value {
            ImplItem::Fn(value) => value,
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    format!("Only functions are allowed in `#[vye::dispatcher]` blocks, got {other:?}"),
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
                                &value.sig.output,
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
        let (attrs, args) = parse_then_filter(&value.attrs)?;

        Ok(Self {
            kind,
            args,
            attrs,
            vis: &value.vis,
            name: &value.sig.ident,
            generics: &value.sig.generics,
            inputs: &value.sig.inputs,
            block: &value.block,
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

#[derive(FromAttributes, Default)]
#[darling(attributes(vye))]
struct FieldArgs {
    #[darling(default)]
    vis: Option<Visibility>,
}

struct DispatcherField<'a> {
    args: FieldArgs,
    attrs: Vec<&'a Attribute>,
    name: &'a Ident,
    ty: &'a Type,
}

impl<'a> DispatcherField<'a> {
    fn new(
        fn_arg: &'a FnArg,
        kind: &DispatcherItemKind,
        ctx_name: &mut Option<Ident>,
    ) -> syn::Result<Option<Self>> {
        match fn_arg {
            // self type, skip
            FnArg::Receiver(_) => Ok(None),
            FnArg::Typed(pat_type) => {
                // `&mut UpdateContext<App>`, skip
                if let DispatcherItemKind::Updater = kind
                    && let Some(ident) = is_update_context(pat_type)
                {
                    *ctx_name = Some(ident.clone());
                    return Ok(None);
                }

                // todo: more sophisticated error handling for this case
                let Pat::Ident(PatIdent { ident: name, .. }) = &*pat_type.pat else {
                    return Ok(None);
                };

                let (attrs, field_args) = parse_then_filter(&pat_type.attrs)?;
                Ok(Some(Self {
                    args: field_args,
                    attrs,
                    name,
                    ty: &pat_type.ty,
                }))
            }
        }
    }

    fn to_field(&self) -> Field {
        Field {
            attrs: self.attrs.iter().copied().cloned().collect(),
            vis: self.args.vis.clone().unwrap_or(Visibility::Inherited),
            mutability: FieldMutability::None,
            colon_token: Some(Token![:](self.name.span())),
            ident: Some(self.name.clone()),
            ty: self.ty.clone(),
        }
    }
}

impl<'a> ToTokens for DispatcherField<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.to_field().to_tokens(tokens)
    }
}

impl<'a> DispatcherItem<'a> {
    fn make_fields(&self, ctx_name: &mut Option<Ident>) -> syn::Result<Vec<DispatcherField<'a>>> {
        self.inputs
            .iter()
            .filter_map(|fn_arg| DispatcherField::new(fn_arg, &self.kind, ctx_name).transpose())
            .collect::<syn::Result<Vec<_>>>()
    }

    fn generate(&self, crate_: &TokenStream, model_ty: &Type) -> syn::Result<TokenStream> {
        let mut ctx_name = None;
        let fields = self.make_fields(&mut ctx_name)?;
        let field_names = fields.iter().map(|f| &f.name).collect::<Vec<_>>();
        let name = self.args.name.clone().unwrap_or_else(|| {
            Ident::new(&ccase!(pascal, self.name.to_string()), Span::call_site())
        });
        let attrs = &self.attrs;
        let vis = &self.vis;
        let block = &self.block;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let struct_decl = quote! {
            #(#attrs)*
            #vis struct #name #impl_generics #where_clause {
                #(#fields),*
            }
        };
        match &self.kind {
            DispatcherItemKind::Updater => Ok(quote! {
                #struct_decl
                impl #impl_generics #crate_::ModelMessage for #name #ty_generics #where_clause {}
                impl #impl_generics #crate_::ModelHandler<#name> for #model_ty #ty_generics #where_clause {
                    fn update(
                        &mut self,
                        #name { #(#field_names),* }: #name #ty_generics,
                        #ctx_name: &mut #crate_::UpdateContext<<#model_ty as #crate_::Model>::ForApp>,
                    ) {
                        #block
                    }
                }
            }),
            DispatcherItemKind::Getter { data_ty } => Ok(quote! {
                #struct_decl
                impl #impl_generics #crate_::ModelGetterMessage for #name #ty_generics #where_clause {
                    type Data = #data_ty;
                }
                impl #impl_generics #crate_::ModelGetterHandler<#name> for #model_ty #where_clause {
                    fn getter(&self, #name { #(#field_names),* }: #name #ty_generics) -> #data_ty {
                        #block
                    }
                }
            }),
        }
    }
}

pub fn build(value: ItemImpl, args: DispatcherArgs) -> syn::Result<TokenStream> {
    DispatcherContext::new(&value, args)?.generate()
}
