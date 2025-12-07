pub(super) mod raw;
mod which;

use crate::model::attr::raw::{ProcessedMeta, ProcessedMetaRef};
use crate::utils;
use crate::utils::ThisCrate;
use convert_case::ccase;
use either::Either;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use std::iter;
use syn::Meta;
use which::{Getter, Updater, With};

fn invalid_position_error(span: Span, expr: &str) -> syn::Error {
    syn::Error::new(span, format!("`{expr}` is not valid in this position"))
}

fn resolve_name_for_model<W: With>(config: Option<&raw::NameConfig>, model_name: &Ident) -> Ident {
    config.and_then(|n| W::name(n).clone()).unwrap_or_else(|| {
        let model_name = model_name.to_string();
        let model_name = model_name
            .strip_suffix("Model")
            .unwrap_or(&model_name)
            .to_owned();
        format_ident!("{model_name}{}", W::SUFFIX)
    })
}

fn resolve_name_for_message(args: Option<&raw::MethodArgs>, fn_name: &Ident) -> Ident {
    args.as_ref()
        .and_then(|n| n.message.clone())
        .unwrap_or_else(|| Ident::new(&ccase!(pascal, fn_name.to_string()), Span::call_site()))
}

fn include_if_frb(
    iter: impl IntoIterator<Item = ProcessedMeta>,
    include: impl FnOnce() -> TokenStream,
    flutter_rust_bridge: bool,
) -> impl Iterator<Item = ProcessedMeta> {
    if flutter_rust_bridge {
        Either::Left(iter.into_iter().chain(iter::once(ProcessedMeta(include()))))
    } else {
        Either::Right(iter.into_iter())
    }
}

fn frb_sync_getter(crate_: &ThisCrate) -> TokenStream {
    utils::frb(quote! { sync, getter }, crate_)
}

macro_rules! validate {
    (
        $span:expr,
        $validator:expr;
        $($validated:expr => $expr:expr,)*
    ) => {{
        let validator = $validator;
        $(
        if !validator($validated) {
            return Err(invalid_position_error($span, $expr));
        }
        )*
    }};
}

pub struct ModelArgs {
    pub message: MessageEnumProperties,
    pub updater: ModelProperties,
    pub getter: ModelProperties,
    pub for_app: Ident,
}

impl ModelArgs {
    fn validate(def: &raw::DispatcherConfig, span: Span) -> syn::Result<()> {
        if let Some(raw::MetaConfig { message, fns, .. }) = &def.meta {
            validate! {
                span, |v: &Vec<Meta>| v.is_empty();
                message => "#[vye::dispatcher(meta(message(...)))]",
                fns => "#[vye::dispatcher(meta(fns(...)))]",
            }
        }

        Ok(())
    }

    pub fn parse(
        raw: raw::ModelArgs,
        model_name: &Ident,
        crate_: &ThisCrate,
        span: Span,
    ) -> syn::Result<Self> {
        let flutter_rust_bridge = raw.flutter_rust_bridge();
        let config = raw
            .dispatcher
            .ok_or_else(|| syn::Error::new(span, "`dispatcher` is required"))?
            .into_config();
        let message =
            MessageEnumProperties::from_config(raw.message, model_name, crate_, flutter_rust_bridge);
        Self::validate(&config, span)?;
        Ok(Self::from_config(
            config,
            message,
            raw.for_app,
            model_name,
            crate_,
            flutter_rust_bridge,
        ))
    }

    fn from_config(
        config: raw::DispatcherConfig,
        message: MessageEnumProperties,
        for_app: Ident,
        model_name: &Ident,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> Self {
        Self {
            message,
            updater: ModelProperties::from_config::<Updater>(
                &config,
                model_name,
                crate_,
                flutter_rust_bridge,
            ),
            getter: ModelProperties::from_config::<Getter>(
                &config,
                model_name,
                crate_,
                flutter_rust_bridge,
            ),
            for_app,
        }
    }
}

pub struct ModelProperties {
    pub name: Ident,
    pub outer_meta: Vec<ProcessedMeta>,
    pub inner_meta: Vec<ProcessedMeta>,
}

impl ModelProperties {
    fn from_config<W: With>(
        config: &raw::DispatcherConfig,
        model_name: &Ident,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> Self {
        Self {
            name: resolve_name_for_model::<W>(config.name.as_ref(), model_name),
            outer_meta: {
                include_if_frb(
                    W::outer_meta_owned(&config.meta),
                    || utils::frb_opaque(crate_),
                    flutter_rust_bridge,
                )
                .collect()
            },
            inner_meta: W::inner_meta_owned(&config.meta).collect(),
        }
    }
}

pub struct MessageEnumProperties {
    pub name: Ident,
    pub outer_meta: Vec<ProcessedMeta>,
}

impl MessageEnumProperties {
    pub fn from_config(
        config: raw::MessageConfig,
        model_name: &Ident,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> Self {
        Self {
            name: config.name.unwrap_or_else(|| {
                let model_name = model_name.to_string();
                let model_name = model_name.strip_suffix("Model").unwrap_or(&model_name);
                format_ident!("{model_name}Message")
            }),
            outer_meta: include_if_frb(
                config
                    .meta
                    .iter()
                    .map(ProcessedMetaRef::process)
                    .map(ProcessedMetaRef::into_owned),
                || utils::frb_opaque(crate_),
                flutter_rust_bridge,
            ).collect(),
        }
    }
}

#[derive(Default)]
pub struct NewMethodArgs {
    pub updater_meta: Vec<ProcessedMeta>,
    pub getter_meta: Vec<ProcessedMeta>,
}

impl NewMethodArgs {
    fn validate(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        if raw.name.is_some() {
            return Err(invalid_position_error(span, "#[vye(name(...))]"));
        };

        Ok(())
    }

    pub fn parse(
        raw: raw::MethodArgs,
        span: Span,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> syn::Result<Self> {
        Self::validate(&raw, span)?;
        Ok(Self {
            updater_meta: include_if_frb(
                Updater::outer_meta_owned(&raw.meta),
                || utils::frb_sync(crate_),
                flutter_rust_bridge,
            )
            .collect(),
            getter_meta: include_if_frb(
                Getter::outer_meta_owned(&raw.meta),
                || utils::frb_sync(crate_),
                flutter_rust_bridge,
            )
            .collect(),
        })
    }
}

enum UpdaterOrGetter {
    Updater,
    Getter,
}

pub struct UpdaterGetterMethodArgs {
    pub message: MessageProperties,
    pub fn_name: Ident,
    pub fn_meta: Vec<ProcessedMeta>,
}

impl UpdaterGetterMethodArgs {
    fn parse<W: With>(
        raw: raw::MethodArgs,
        fn_name: &Ident,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
        updater_or_getter: UpdaterOrGetter,
    ) -> Self {
        Self {
            message: MessageProperties::parse(&raw, fn_name),
            fn_name: fn_name.clone(),
            fn_meta: {
                if flutter_rust_bridge && matches!(updater_or_getter, UpdaterOrGetter::Getter) {
                    include_if_frb(
                        W::fn_meta_owned(&raw.meta),
                        || frb_sync_getter(crate_),
                        flutter_rust_bridge,
                    )
                    .collect()
                } else {
                    W::fn_meta_owned(&raw.meta).collect()
                }
            },
        }
    }

    fn validate(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        if let Some(config) = &raw.name {
            validate! {
                span, |v: &Option<Ident>| v.is_none();
                &config.updater => "#[vye(name(updater = \"...\"))]",
                &config.getter => "#[vye(name(getter = \"...\"))]",
            }
        }

        if let Some(raw::MetaConfig { inner, .. }) = &raw.meta
            && inner.is_some()
        {
            return Err(invalid_position_error(span, "#[vye(meta(inner(...)))]"));
        }

        Ok(())
    }

    fn validate_updater(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        Self::validate(raw, span)?;

        if let Some(raw::MetaConfig { getter, .. }) = &raw.meta
            && !getter.is_empty()
        {
            return Err(invalid_position_error(span, "#[vye(meta(getter(...)))]"));
        }

        Ok(())
    }

    fn validate_getter(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        Self::validate(raw, span)?;

        if let Some(raw::MetaConfig { updater, .. }) = &raw.meta
            && !updater.is_empty()
        {
            return Err(invalid_position_error(span, "#[vye(meta(updater(...)))]"));
        }

        Ok(())
    }

    pub fn parse_updater(
        raw: raw::MethodArgs,
        fn_name: &Ident,
        span: Span,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> syn::Result<Self> {
        Self::validate_updater(&raw, span)?;
        Ok(Self::parse::<Updater>(
            raw,
            fn_name,
            crate_,
            flutter_rust_bridge,
            UpdaterOrGetter::Updater,
        ))
    }

    pub fn parse_getter(
        raw: raw::MethodArgs,
        fn_name: &Ident,
        span: Span,
        crate_: &ThisCrate,
        flutter_rust_bridge: bool,
    ) -> syn::Result<Self> {
        Self::validate_getter(&raw, span)?;
        Ok(Self::parse::<Getter>(
            raw,
            fn_name,
            crate_,
            flutter_rust_bridge,
            UpdaterOrGetter::Getter,
        ))
    }
}

pub struct MessageProperties {
    pub name: Ident,
    pub outer_meta: Vec<ProcessedMeta>,
}

impl MessageProperties {
    fn parse(raw: &raw::MethodArgs, fn_name: &Ident) -> Self {
        Self {
            name: resolve_name_for_message(Some(raw), fn_name),
            outer_meta: {
                raw.meta
                    .as_ref()
                    .map(|m| m.message().map(ProcessedMetaRef::into_owned).collect())
                    .unwrap_or_default()
            },
        }
    }
}
