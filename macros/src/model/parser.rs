use crate::model::attr::raw::ProcessedMeta;
use crate::model::attr::{ModelArgs, NewMethodArgs, UpdaterGetterMethodArgs, raw};
use crate::model::{
    FnKind, ModelContext, ParsedFnArg, ParsedGetterFn, ParsedNewFn, ParsedUpdaterFn,
    ParsedUpdaterGetterFn, RawModelArgs,
};
use crate::utils;
use crate::utils::{InterfaceImpl, MaybeStubFn, ThisCrate};
use darling::FromAttributes;
use proc_macro2::TokenStream;
use syn::spanned::Spanned;
use syn::{
    AngleBracketedGenericArguments, FnArg, GenericArgument, GenericParam, Pat, PatIdent, PatType,
    Path, PathArguments, PathSegment, ReturnType, Signature, Type, TypePath, Visibility,
};

impl<'a> ModelContext<'a> {
    pub(super) fn parse(item: &'a InterfaceImpl, attrs: RawModelArgs) -> syn::Result<Self> {
        let Type::Path(ty_path) = &item.self_ty else {
            return Err(syn::Error::new_spanned(
                &item.self_ty,
                "`#[emyu::model]` can only be applied to impl blocks for named types",
            ));
        };
        let model_name = &ty_path
            .path
            .segments
            .last()
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    &item.self_ty,
                    "`#[emyu::model]` can only be applied to impl blocks for named types",
                )
            })?
            .ident;
        let crate_ = ThisCrate::default();
        let items = item
            .items
            .iter()
            .map(|item| ParsedFnFirstPass::parse(item, &crate_, attrs.flutter_rust_bridge()))
            .collect::<syn::Result<Vec<_>>>()?;
        let ParsedFnsSecondPass {
            new_fn,
            updaters,
            getters,
        } = ParsedFnsSecondPass::parse(items, &crate_, attrs.flutter_rust_bridge());
        Ok(Self {
            args: ModelArgs::parse(attrs, model_name, &crate_, ty_path.span())?,
            crate_,
            struct_vis: &item.vis,
            model_ty: ty_path,
            new_fn,
            updaters,
            getters,
        })
    }
}

struct ParsedFnFirstPass<'a> {
    vis: &'a Visibility,
    fn_args: Vec<ParsedFnArg<'a>>,
    kind: FnKind<'a>,
}

impl<'a> ParsedFnFirstPass<'a> {
    fn parse(
        item: &'a MaybeStubFn,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> syn::Result<Self> {
        let args = raw::MethodArgs::from_attributes(&item.attrs)?;
        let mut kind = FnKind::analyze(item, args, crate_, flutter_rust_bridge)?;
        Ok(Self {
            vis: &item.vis,
            fn_args: item
                .sig
                .inputs
                .iter()
                .flat_map(|i| ParsedFnArg::parse(i).transpose())
                .collect::<syn::Result<Vec<_>>>()?,
            kind,
        })
    }
}

impl<'a> FnKind<'a> {
    fn validate(sig: &Signature) -> syn::Result<()> {
        if sig.constness.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "const functions are not supported in `#[emyu::model]`",
            ));
        }

        if sig.asyncness.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "async functions are not supported in `#[emyu::model]`",
            ));
        }

        if sig.unsafety.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "unsafe functions are not supported in `#[emyu::model]`",
            ));
        }

        if sig.abi.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "extern functions are not supported in `#[emyu::model]`",
            ));
        }

        for param in &sig.generics.params {
            if let GenericParam::Lifetime(_) = param {
                return Err(syn::Error::new_spanned(
                    param,
                    "lifetime parameters are not supported in `#[emyu::model]`",
                ));
            }
        }

        if sig.variadic.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "variadic functions are not supported in `#[emyu::model]`",
            ));
        }

        Ok(())
    }

    fn analyze(
        item: &'a MaybeStubFn,
        args: raw::MethodArgs,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> syn::Result<Self> {
        enum SelfTy {
            Shared,
            Mutable,
        }

        impl SelfTy {
            fn analyze<'a>(args: impl Iterator<Item = &'a FnArg>) -> Option<Self> {
                for arg in args {
                    if let FnArg::Receiver(receiver) = arg
                        && receiver.reference.is_some()
                    {
                        return if receiver.mutability.is_some() {
                            Some(Self::Mutable)
                        } else {
                            Some(Self::Shared)
                        };
                    }
                }

                None
            }
        }

        fn extract_inner_signal_ty(ret_ty: &ReturnType) -> Option<&Type> {
            if let ReturnType::Type(_, ty) = ret_ty
                && let Type::Path(TypePath {
                    path: Path { segments, .. },
                    ..
                }) = &**ty
                && let Some(PathSegment {
                    ident,
                    arguments:
                        PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }),
                }) = segments.last()
                && ident == "Signal"
                && args.len() == 1
                && let GenericArgument::Type(ty) = &args[0]
            {
                Some(ty)
            } else {
                None
            }
        }

        Self::validate(&item.sig)?;
        let fn_name = item.sig.ident.to_string();
        let has_no_fn_args = item.sig.inputs.is_empty()
            || (item.sig.inputs.len() == 1
                && matches!(item.sig.inputs.first(), Some(FnArg::Receiver(_))));
        let (self_ty, ret_ty, block) = (
            SelfTy::analyze(item.sig.inputs.iter()),
            extract_inner_signal_ty(&item.sig.output),
            item.block.as_ref(),
        );

        match (fn_name.as_str(), self_ty, ret_ty, has_no_fn_args, block) {
            ("new", None, None, true, None) => Ok(Self::New(NewMethodArgs::parse(
                args,
                item.sig.span(),
                crate_,
                flutter_rust_bridge,
            )?)),
            (_, Some(SelfTy::Mutable), command_ty, _, Some(block)) => Ok(Self::Updater {
                args: UpdaterGetterMethodArgs::parse_updater(
                    args,
                    &item.sig.ident,
                    item.sig.span(),
                    crate_,
                    flutter_rust_bridge,
                )?,
                command_ty,
                block,
            }),
            (_, Some(SelfTy::Shared), Some(ty), true, None) => Ok(Self::Getter {
                args: UpdaterGetterMethodArgs::parse_getter(
                    args,
                    &item.sig.ident,
                    item.sig.span(),
                    crate_,
                    flutter_rust_bridge,
                )?,
                ty,
            }),
            _ => Err(syn::Error::new_spanned(
                &item.sig,
                "could not determine function shape",
            )),
        }
    }
}

impl<'a> ParsedFnArg<'a> {
    fn parse(item: &'a FnArg) -> syn::Result<Option<Self>> {
        match item {
            FnArg::Receiver(_) => Ok(None),
            FnArg::Typed(PatType { attrs, pat, ty, .. }) => {
                if let Pat::Ident(PatIdent { ident: name, .. }) = &**pat {
                    Ok(Some(Self { attrs, name, ty }))
                } else {
                    Err(syn::Error::new_spanned(
                        item,
                        "unsupported function argument in `#[emyu::model]`",
                    ))
                }
            }
        }
    }
}

impl ParsedNewFn {
    fn inject_base_meta(&mut self, tokens: TokenStream) {
        self.method_args
            .updater_meta
            .push(ProcessedMeta(tokens.clone()));
        self.method_args.getter_meta.push(ProcessedMeta(tokens));
    }
}

struct ParsedFnsSecondPass<'a> {
    new_fn: ParsedNewFn,
    updaters: Vec<ParsedUpdaterFn<'a>>,
    getters: Vec<ParsedGetterFn<'a>>,
}

impl<'a> ParsedFnsSecondPass<'a> {
    fn parse(
        items: Vec<ParsedFnFirstPass<'a>>,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> Self {
        let mut new_fn = ParsedNewFn::default();
        let mut updaters = Vec::with_capacity(items.len());
        let mut getters = Vec::with_capacity(items.len());

        for item in items {
            match item.kind {
                FnKind::New(method_args) => {
                    new_fn = ParsedNewFn {
                        vis: item.vis.clone(),
                        method_args,
                    }
                }
                FnKind::Updater {
                    args: method_args,
                    command_ty,
                    block,
                } => updaters.push(ParsedUpdaterFn {
                    common: ParsedUpdaterGetterFn {
                        vis: item.vis,
                        method_args,
                    },
                    fn_args: item.fn_args,
                    command_ty,
                    block,
                }),
                FnKind::Getter {
                    args: method_args,
                    ty,
                } => getters.push(ParsedGetterFn {
                    common: ParsedUpdaterGetterFn {
                        vis: item.vis,
                        method_args,
                    },
                    ret_ty: ty,
                }),
            }
        }

        if flutter_rust_bridge {
            new_fn.inject_base_meta(utils::frb_sync(crate_));
        }

        updaters.shrink_to_fit();
        getters.shrink_to_fit();

        Self {
            new_fn,
            updaters,
            getters,
        }
    }
}
