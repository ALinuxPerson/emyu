//! `#[emyu::model]` macro implementation.
mod attr;
mod parser;
mod generator;

use crate::model::attr::{NewMethodArgs, UpdaterGetterMethodArgs};
use crate::utils::{InterfaceImpl, ThisCrate};
use attr::ModelArgs;
pub use attr::raw::ModelArgs as RawModelArgs;
use proc_macro2::{Ident, TokenStream};
use syn::{Attribute, Block, Type, TypePath, Visibility};

struct ModelContext<'a> {
    crate_: ThisCrate,
    args: ModelArgs,
    struct_vis: &'a Visibility,
    model_ty: &'a TypePath,
    new_fn: ParsedNewFn,
    updaters: Vec<ParsedUpdaterFn<'a>>,
    getters: Vec<ParsedGetterFn<'a>>,
}

enum FnKind<'a> {
    // fn new();
    New(NewMethodArgs),

    // fn updater(&mut self) {}
    Updater {
        args: UpdaterGetterMethodArgs,
        ctx: Option<&'a Ident>,
        block: &'a Block,
    },

    // fn getter(&self) -> Ret [{} | ;]
    Getter {
        args: UpdaterGetterMethodArgs,
        ty: &'a Type,
    },
}

struct ParsedFnArg<'a> {
    attrs: &'a [Attribute],
    name: &'a Ident,
    ty: &'a Type,
}

struct ParsedNewFn {
    vis: Visibility,
    method_args: NewMethodArgs,
}

impl Default for ParsedNewFn {
    fn default() -> Self {
        Self {
            vis: Visibility::Inherited,
            method_args: NewMethodArgs::default(),
        }
    }
}

struct ParsedUpdaterGetterFn<'a> {
    vis: &'a Visibility,
    method_args: UpdaterGetterMethodArgs,
}

struct ParsedUpdaterFn<'a> {
    common: ParsedUpdaterGetterFn<'a>,
    fn_args: Vec<ParsedFnArg<'a>>,
    ctx: Option<&'a Ident>,
    block: &'a Block,
}

struct ParsedGetterFn<'a> {
    common: ParsedUpdaterGetterFn<'a>,
    ret_ty: &'a Type,
}


pub fn build(item: InterfaceImpl, attrs: RawModelArgs) -> syn::Result<TokenStream> {
    Ok(ModelContext::parse(&item, attrs)?.generate())
}
