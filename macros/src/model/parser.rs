use crate::crate_;
use crate::model::attr::{ModelArgs, NewMethodArgs, UpdaterGetterMethodArgs, raw};
use crate::model::{
    FnKind, ModelContext, ParsedFnArg, ParsedGetterFn, ParsedNewFn, ParsedNewSplitFn,
    ParsedSplitFn, ParsedUpdaterFn, ParsedUpdaterGetterFn, RawModelArgs,
};
use crate::utils::{InterfaceImpl, MaybeStubFn};
use darling::FromAttributes;
use proc_macro2::Ident;
use syn::spanned::Spanned;
use syn::{
    AngleBracketedGenericArguments, FnArg, GenericArgument, GenericParam, Pat, PatIdent,
    PatType, Path, PathArguments, PathSegment, ReturnType, Signature, Type, TypePath,
    TypeReference, Visibility,
};

impl<'a> ModelContext<'a> {
    pub(super) fn parse(item: &'a InterfaceImpl, attrs: RawModelArgs) -> syn::Result<Self> {
        let Type::Path(ty_path) = &item.self_ty else {
            return Err(syn::Error::new_spanned(
                &item.self_ty,
                "`#[vye::model]` can only be applied to impl blocks for named types",
            ));
        };
        let model_name = &ty_path
            .path
            .segments
            .last()
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    &item.self_ty,
                    "`#[vye::model]` can only be applied to impl blocks for named types",
                )
            })?
            .ident;
        let items = item
            .items
            .iter()
            .map(ParsedFnFirstPass::parse)
            .collect::<syn::Result<Vec<_>>>()?;
        let ParsedFnsSecondPass {
            new_fn,
            split_fn,
            updaters,
            getters,
        } = ParsedFnsSecondPass::parse(items);
        Ok(Self {
            crate_: crate_(),
            args: ModelArgs::parse(attrs, model_name, ty_path.span())?,
            struct_vis: &item.vis,
            model_ty: ty_path,
            new_fn,
            split_fn,
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
    fn parse(item: &'a MaybeStubFn) -> syn::Result<Self> {
        let args = raw::MethodArgs::from_attributes(&item.attrs)?;
        let kind = FnKind::analyze(item, args)?;
        Ok(Self {
            vis: &item.vis,
            kind,
            fn_args: item
                .sig
                .inputs
                .iter()
                .flat_map(|i| ParsedFnArg::parse(i).transpose())
                .collect::<syn::Result<Vec<_>>>()?,
        })
    }
}

impl<'a> FnKind<'a> {
    fn validate(sig: &Signature) -> syn::Result<()> {
        if sig.constness.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "const functions are not supported in `#[vye::model]`",
            ));
        }

        if sig.asyncness.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "async functions are not supported in `#[vye::model]`",
            ));
        }

        if sig.unsafety.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "unsafe functions are not supported in `#[vye::model]`",
            ));
        }

        if sig.abi.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "extern functions are not supported in `#[vye::model]`",
            ));
        }

        for param in &sig.generics.params {
            if let GenericParam::Lifetime(_) = param {
                return Err(syn::Error::new_spanned(
                    param,
                    "lifetime parameters are not supported in `#[vye::model]`",
                ));
            }
        }

        if sig.variadic.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "variadic functions are not supported in `#[vye::model]`",
            ));
        }

        Ok(())
    }

    fn analyze(item: &'a MaybeStubFn, args: raw::MethodArgs) -> syn::Result<Self> {
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

        fn find_update_context_ident<'a>(
            mut inputs: impl Iterator<Item = &'a FnArg>,
        ) -> Option<&'a Ident> {
            inputs.find_map(|fn_arg| {
                if let FnArg::Typed(PatType { pat, ty, .. }) = fn_arg
                    && let Pat::Ident(PatIdent { ident, .. }) = &**pat
                    && let Type::Reference(TypeReference {
                        mutability, elem, ..
                    }) = &**ty
                    && mutability.is_some()
                    && let Type::Path(TypePath {
                        path: Path { segments, .. },
                        ..
                    }) = &**elem
                    && let Some(PathSegment {
                        ident: ty_ident,
                        arguments:
                            PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                                args: generic_args,
                                ..
                            }),
                    }) = segments.last()
                    && ty_ident == "UpdateContext"
                    && !generic_args.is_empty()
                    && generic_args.len() <= 2
                {
                    match (generic_args.first(), generic_args.last()) {
                        // UpdateContext<MyApp> or UpdateContext<'_, MyApp>
                        (Some(GenericArgument::Type(_)), None)
                        | (Some(GenericArgument::Lifetime(_)), Some(GenericArgument::Type(_))) => {
                            Some(ident)
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })
        }

        Self::validate(&item.sig)?;
        let fn_name = item.sig.ident.to_string();
        let (self_ty, ret_ty, block) = (
            SelfTy::analyze(item.sig.inputs.iter()),
            &item.sig.output,
            item.block.as_ref(),
        );

        match (fn_name.as_str(), self_ty, ret_ty, block) {
            ("new", None, ReturnType::Default, None) => {
                Ok(Self::New(NewMethodArgs::parse(args, item.sig.span())?))
            }
            ("split", None, ReturnType::Default, None) => Ok(Self::Split(&item.attrs)),
            (_, Some(SelfTy::Mutable), ReturnType::Default, Some(block)) => Ok(Self::Updater {
                args: UpdaterGetterMethodArgs::parse_updater(
                    args,
                    &item.sig.ident,
                    item.sig.span(),
                )?,
                ctx: find_update_context_ident(item.sig.inputs.iter()),
                block,
            }),
            (_, Some(SelfTy::Shared), ReturnType::Type(_, ty), block) => Ok(Self::Getter {
                args: UpdaterGetterMethodArgs::parse_getter(
                    args,
                    &item.sig.ident,
                    item.sig.span(),
                )?,
                ty: &**ty,
                block,
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
                        "unsupported function argument in `#[vye::model]`",
                    ))
                }
            }
        }
    }
}

struct ParsedFnsSecondPass<'a> {
    new_fn: ParsedNewFn,
    split_fn: ParsedSplitFn<'a>,
    updaters: Vec<ParsedUpdaterFn<'a>>,
    getters: Vec<ParsedGetterFn<'a>>,
}

impl<'a> ParsedFnsSecondPass<'a> {
    fn parse(items: Vec<ParsedFnFirstPass<'a>>) -> Self {
        let mut new_fn = ParsedNewFn::default();
        let mut split_fn = ParsedSplitFn::default();
        let mut updaters = Vec::with_capacity(items.len());
        let mut getters = Vec::with_capacity(items.len());

        for item in items {
            match item.kind {
                FnKind::New(method_args) => {
                    new_fn = ParsedNewFn(ParsedNewSplitFn {
                        vis: item.vis.clone(),
                        method_args,
                    })
                }
                FnKind::Split(attrs) => {
                    split_fn = ParsedSplitFn {
                        vis: item.vis.clone(),
                        attrs,
                    }
                }
                FnKind::Updater {
                    args: method_args,
                    ctx,
                    block,
                } => updaters.push(ParsedUpdaterFn {
                    common: ParsedUpdaterGetterFn {
                        vis: item.vis,
                        method_args,
                        fn_args: item.fn_args,
                    },
                    ctx,
                    block,
                }),
                FnKind::Getter {
                    args: method_args,
                    ty,
                    block,
                } => getters.push(ParsedGetterFn {
                    common: ParsedUpdaterGetterFn {
                        vis: item.vis,
                        method_args,
                        fn_args: item.fn_args,
                    },
                    block,
                    ret_ty: ty,
                }),
            }
        }

        updaters.shrink_to_fit();
        getters.shrink_to_fit();

        Self {
            new_fn,
            split_fn,
            updaters,
            getters,
        }
    }
}
