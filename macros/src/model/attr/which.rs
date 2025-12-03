use super::raw;
use proc_macro2::Ident;
use syn::{Meta, Visibility};

pub trait With {
    const SUFFIX: &'static str;
    
    fn name(name: &raw::NameConfig) -> &Option<Ident>;
    fn vis(vis: raw::MaybeVisConfig) -> Visibility;
    fn outer_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta>;
    fn inner_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta>;
    fn fn_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta>;
}

pub enum Dispatcher {}

impl With for Dispatcher {
    const SUFFIX: &'static str = "Dispatcher";
    
    fn name(name: &raw::NameConfig) -> &Option<Ident> {
        &name.dispatcher
    }

    fn vis(vis: raw::MaybeVisConfig) -> Visibility {
        vis.dispatcher()
    }

    fn outer_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.dispatcher()
    }

    fn inner_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.dispatcher_inner()
    }

    fn fn_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.dispatcher_fn()
    }
}

pub enum Updater {}

impl With for Updater {
    const SUFFIX: &'static str = "Updater";
    
    fn name(name: &raw::NameConfig) -> &Option<Ident> {
        &name.updater
    }

    fn vis(vis: raw::MaybeVisConfig) -> Visibility {
        vis.updater()
    }

    fn outer_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.updater()
    }

    fn inner_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.updater_inner()
    }

    fn fn_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.updater_fn()
    }
}

pub enum Getter {}

impl With for Getter {
    const SUFFIX: &'static str = "Getter";
    
    fn name(name: &raw::NameConfig) -> &Option<Ident> {
        &name.getter
    }

    fn vis(vis: raw::MaybeVisConfig) -> Visibility {
        vis.getter()
    }

    fn outer_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.getter()
    }

    fn inner_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.getter_inner()
    }

    fn fn_meta(meta: raw::MetaConfigs<'_>) -> impl Iterator<Item = &Meta> {
        meta.getter_fn()
    }
}
