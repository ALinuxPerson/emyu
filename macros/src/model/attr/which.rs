use super::raw;
use either::Either;
use proc_macro2::Ident;
use std::iter;
use syn::Meta;

fn meta_or_empty<'a, I: Iterator<Item = &'a Meta>>(
    meta: &'a Option<raw::MetaConfig>,
    f: impl FnOnce(&'a raw::MetaConfig) -> I,
) -> impl Iterator<Item = &'a Meta> {
    match meta.as_ref() {
        Some(config) => Either::Left(f(config)),
        None => Either::Right(iter::empty()),
    }
}

pub trait With {
    const SUFFIX: &'static str;

    fn name(name: &raw::NameConfig) -> &Option<Ident>;
    fn outer_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta>;
    fn inner_meta_impl(meta: &raw::InnerMetaConfig) -> &Vec<Meta>;
    fn fn_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta>;

    fn outer_meta(meta: &Option<raw::MetaConfig>) -> impl Iterator<Item = &Meta> {
        meta_or_empty(meta, Self::outer_meta_impl)
    }

    fn inner_meta(meta: &Option<raw::MetaConfig>) -> impl Iterator<Item = &Meta> {
        match meta.as_ref() {
            Some(raw::MetaConfig {
                inner: Some(config),
                ..
            }) => Either::Left(Self::inner_meta_impl(config).iter()),
            _ => Either::Right(iter::empty()),
        }
    }

    fn fn_meta(meta: &Option<raw::MetaConfig>) -> impl Iterator<Item = &Meta> {
        meta_or_empty(meta, Self::fn_meta_impl)
    }
}

pub enum Dispatcher {}

impl With for Dispatcher {
    const SUFFIX: &'static str = "Dispatcher";

    fn name(name: &raw::NameConfig) -> &Option<Ident> {
        &name.dispatcher
    }

    fn outer_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta> {
        meta.dispatcher()
    }

    fn inner_meta_impl(meta: &raw::InnerMetaConfig) -> &Vec<Meta> {
        &meta.dispatcher
    }

    fn fn_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta> {
        meta.dispatcher_fn()
    }
}

pub enum Updater {}

impl With for Updater {
    const SUFFIX: &'static str = "Updater";

    fn name(name: &raw::NameConfig) -> &Option<Ident> {
        &name.updater
    }

    fn outer_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta> {
        meta.updater()
    }

    fn inner_meta_impl(meta: &raw::InnerMetaConfig) -> &Vec<Meta> {
        &meta.updater
    }

    fn fn_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta> {
        meta.updater_fn()
    }
}

pub enum Getter {}

impl With for Getter {
    const SUFFIX: &'static str = "Getter";

    fn name(name: &raw::NameConfig) -> &Option<Ident> {
        &name.getter
    }

    fn outer_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta> {
        meta.getter()
    }

    fn inner_meta_impl(meta: &raw::InnerMetaConfig) -> &Vec<Meta> {
        &meta.getter
    }

    fn fn_meta_impl(meta: &raw::MetaConfig) -> impl Iterator<Item = &Meta> {
        meta.getter_fn()
    }
}
