pub(super) mod raw;
mod which;

use convert_case::ccase;
use darling::FromAttributes;
use proc_macro2::{Ident, Span};
use quote::format_ident;
use syn::{Meta, Visibility};
use which::{Dispatcher, Getter, Updater, With};

fn invalid_position_error(span: Span, expr: &str) -> syn::Error {
    syn::Error::new(span, format!("`{expr}` is not valid in this position"))
}

fn resolve_name_for_model<W: With>(
    config: Option<&raw::NameConfig>,
    fallback_base: Option<&Ident>,
    model_name: &Ident,
) -> Ident {
    let base = config.map(|n| n.base.as_ref()).unwrap_or(fallback_base);
    config.and_then(|n| W::name(n).clone()).unwrap_or_else(|| {
        let base = base.map(|p| p.to_string()).unwrap_or_else(|| {
            let model_name = model_name.to_string();
            model_name
                .strip_suffix("Model")
                .unwrap_or(&model_name)
                .to_owned()
        });
        format_ident!("{base}{}", W::SUFFIX)
    })
}

fn resolve_name_for_message(config: Option<&raw::NameConfig>, fn_name: &Ident) -> Ident {
    config
        .as_ref()
        .and_then(|n| n.message.clone())
        .unwrap_or_else(|| {
            let fn_name = ccase!(pascal, fn_name.to_string());
            format_ident!("{fn_name}Message")
        })
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
    pub dispatcher: Properties,
    pub updater: Properties,
    pub getter: Properties,
    pub split: Visibility,
}

impl ModelArgs {
    fn validate(def: &raw::DispatcherConfig, span: Span) -> syn::Result<()> {
        if let Some(config) = &def.name {
            validate! {
                span, |v: &Option<Ident>| v.is_none();
                &config.message => "#[vye::dispatcher(name(message = \"...\"))]",
                &config.fns => "#[vye::dispatcher(name(fns = \"...\"))]",
            }
        }

        if let Some(raw::VisConfig::Complex(fields)) = &def.vis
            && fields.message.is_some()
        {
            return Err(invalid_position_error(
                span,
                "#[vye::dispatcher(vis(message = \"...\"))]",
            ));
        }

        for config in &def.meta {
            validate! {
                span, |v: &Vec<Meta>| v.is_empty();
                &config.message => "#[vye::dispatcher(meta(message(...)))]",
                &config.fns => "#[vye::dispatcher(meta(fns(...)))]",
            }
        }

        Ok(())
    }

    pub fn parse(raw: raw::ModelArgs, model_name: &Ident, span: Span) -> syn::Result<Self> {
        let config = raw
            .dispatcher
            .ok_or_else(|| syn::Error::new(span, "`dispatcher` is required"))?
            .into_config();
        Self::validate(&config, span)?;
        Ok(Self::from_config(config, model_name))
    }

    fn from_config(config: raw::DispatcherConfig, model_name: &Ident) -> Self {
        Self {
            dispatcher: Properties::from_config::<Dispatcher>(&config, model_name),
            updater: Properties::from_config::<Updater>(&config, model_name),
            getter: Properties::from_config::<Getter>(&config, model_name),
            split: config.split(),
        }
    }
}

pub struct Properties {
    pub name: Ident,
    pub vis: Visibility,
    pub new_vis: Visibility,
    pub outer_meta: Vec<Meta>,
    pub inner_meta: Vec<Meta>,
}

impl Properties {
    fn from_config<W: With>(config: &raw::DispatcherConfig, model_name: &Ident) -> Self {
        Self {
            name: resolve_name_for_model::<W>(
                config.name.as_ref(),
                config.base.as_ref(),
                model_name,
            ),
            vis: W::vis(config.vis()),
            new_vis: config.new(),
            outer_meta: W::outer_meta(config.meta()).cloned().collect::<Vec<_>>(),
            inner_meta: W::inner_meta(config.meta()).cloned().collect::<Vec<_>>(),
        }
    }
}

pub enum MethodArgs {
    NewSplit(NewSplitMethodAttr),
    UpdaterGetter(UpdaterGetterMethodAttr),
}

impl MethodArgs {
    pub fn parse_new_split(
        raw: raw::MethodArgs,
        fn_vis: &Visibility,
        span: Span,
    ) -> syn::Result<Self> {
        Ok(Self::NewSplit(NewSplitMethodAttr::parse(
            raw, fn_vis, span,
        )?))
    }

    pub fn parse_updater(
        raw: raw::MethodArgs,
        fn_vis: &Visibility,
        fn_name: &Ident,
        span: Span,
    ) -> syn::Result<Self> {
        Ok(Self::UpdaterGetter(UpdaterGetterMethodAttr::parse_updater(
            raw, fn_vis, fn_name, span,
        )?))
    }

    pub fn parse_getter(
        raw: raw::MethodArgs,
        fn_vis: &Visibility,
        fn_name: &Ident,
        span: Span,
    ) -> syn::Result<Self> {
        Ok(Self::UpdaterGetter(UpdaterGetterMethodAttr::parse_getter(
            raw, fn_vis, fn_name, span,
        )?))
    }
}

struct NewSplitMethodAttr {
    dispatcher: NewSplitMethodAttrKindProperties,
    updater: NewSplitMethodAttrKindProperties,
    getter: NewSplitMethodAttrKindProperties,
}

impl NewSplitMethodAttr {
    fn validate(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        if raw.name.is_some() {
            return Err(invalid_position_error(span, "#[vye(name(...))]"));
        };

        Ok(())
    }

    pub fn parse(raw: raw::MethodArgs, fn_vis: &Visibility, span: Span) -> syn::Result<Self> {
        Self::validate(&raw, span)?;
        Ok(Self {
            dispatcher: NewSplitMethodAttrKindProperties::new::<Dispatcher>(&raw, fn_vis),
            updater: NewSplitMethodAttrKindProperties::new::<Updater>(&raw, fn_vis),
            getter: NewSplitMethodAttrKindProperties::new::<Getter>(&raw, fn_vis),
        })
    }
}

struct NewSplitMethodAttrKindProperties {
    vis: Visibility,
    meta: Vec<Meta>,
}

impl NewSplitMethodAttrKindProperties {
    fn new<W: With>(raw: &raw::MethodArgs, fn_vis: &Visibility) -> Self {
        Self {
            vis: W::vis(raw.vis()),
            meta: W::outer_meta(raw.meta()).cloned().collect(),
        }
    }
}

struct UpdaterGetterMethodAttr {
    message: MessageStructProperties,
    fn_name: Ident,
    dispatcher_fn: FnProperties,
    kind: UpdaterGetterMethodAttrKind,
}

impl UpdaterGetterMethodAttr {
    fn new<W: With>(
        raw: raw::MethodArgs,
        fn_vis: &Visibility,
        fn_name: &Ident,
    ) -> Self {
        Self {
            message: MessageStructProperties::parse(&raw, fn_vis, fn_name),
            fn_name: fn_name.clone(),
            dispatcher_fn: FnProperties::parse::<Dispatcher>(&raw, fn_vis),
            kind: UpdaterGetterMethodAttrKind::Updater {
                fn_: FnProperties::parse::<W>(&raw, fn_vis),
            },
        }
    }

    fn validate(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        if let Some(config) = &raw.name {
            validate! {
                span, |v: &Option<Ident>| v.is_none();
                &config.base => "#[vye(name(base = \"...\"))]",
                &config.dispatcher => "#[vye(name(dispatcher = \"...\"))]",
                &config.updater => "#[vye(name(updater = \"...\"))]",
                &config.getter => "#[vye(name(getter = \"...\"))]",
            }
        }

        if let Some(raw::VisConfig::Complex(fields)) = &raw.vis
            && fields.is_pub
        {
            return Err(invalid_position_error(span, "#[vye(vis(pub))]"));
        }

        for config in &raw.meta {
            if config.inner.is_some() {
                return Err(invalid_position_error(span, "#[vye(meta(inner(...)))]"));
            }
        }

        Ok(())
    }

    fn validate_updater(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        Self::validate(raw, span)?;

        if let Some(raw::VisConfig::Complex(fields)) = &raw.vis
            && fields.getter.is_some()
        {
            return Err(invalid_position_error(
                span,
                "#[vye(vis(getter = \"...\"))]",
            ));
        }

        for config in &raw.meta {
            if !config.getter.is_empty() {
                return Err(invalid_position_error(span, "#[vye(meta(getter(...)))]"));
            }
        }

        Ok(())
    }

    fn validate_getter(raw: &raw::MethodArgs, span: Span) -> syn::Result<()> {
        Self::validate(raw, span)?;

        if let Some(raw::VisConfig::Complex(fields)) = &raw.vis
            && fields.updater.is_some()
        {
            return Err(invalid_position_error(
                span,
                "#[vye(vis(updater = \"...\"))]",
            ));
        }

        for config in &raw.meta {
            if !config.updater.is_empty() {
                return Err(invalid_position_error(span, "#[vye(meta(updater(...)))]"));
            }
        }

        Ok(())
    }

    fn parse_updater(
        raw: raw::MethodArgs,
        fn_vis: &Visibility,
        fn_name: &Ident,
        span: Span,
    ) -> syn::Result<Self> {
        Self::validate_updater(&raw, span)?;
        Ok(Self::new::<Updater>(raw, fn_vis, fn_name))
    }

    fn parse_getter(
        raw: raw::MethodArgs,
        fn_vis: &Visibility,
        fn_name: &Ident,
        span: Span,
    ) -> syn::Result<Self> {
        Self::validate_getter(&raw, span)?;
        Ok(Self::new::<Getter>(raw, fn_vis, fn_name))
    }
}

struct MessageStructProperties {
    name: Ident,
    vis: Visibility,
    outer_attrs: Vec<Meta>,
}

impl MessageStructProperties {
    fn parse(raw: &raw::MethodArgs, fn_vis: &Visibility, fn_name: &Ident) -> Self {
        Self {
            name: resolve_name_for_message(raw.name.as_ref(), fn_name),
            vis: raw.vis().message(),
            outer_attrs: raw.meta().message().cloned().collect(),
        }
    }
}

struct FnProperties {
    vis: Visibility,
    meta: Vec<Meta>,
}

impl FnProperties {
    fn parse<W: With>(raw: &raw::MethodArgs, fn_vis: &Visibility) -> Self {
        Self {
            vis: W::vis(raw.vis()),
            meta: W::fn_meta(raw.meta()).cloned().collect(),
        }
    }
}

enum UpdaterGetterMethodAttrKind {
    Updater { fn_: FnProperties },
    Getter { fn_: FnProperties },
}
