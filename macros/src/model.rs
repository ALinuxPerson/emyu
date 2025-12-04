//! `#[vye::model]` macro implementation.
mod attr;
mod parser;
mod generator;

use crate::model::attr::{NewMethodArgs, UpdaterGetterMethodArgs};
use crate::utils::InterfaceImpl;
use attr::ModelArgs;
pub use attr::raw::ModelArgs as RawModelArgs;
use proc_macro2::{Ident, TokenStream};
use syn::{Attribute, Block, Type, TypePath, Visibility};

struct ModelContext<'a> {
    crate_: TokenStream,
    args: ModelArgs,
    struct_vis: &'a Visibility,
    model_ty: &'a TypePath,
    new_fn: ParsedNewFn,
    split_fn: ParsedSplitFn<'a>,
    updaters: Vec<ParsedUpdaterFn<'a>>,
    getters: Vec<ParsedGetterFn<'a>>,
}

enum FnKind<'a> {
    // fn new();
    New(NewMethodArgs),

    // fn split();
    Split(&'a [Attribute]),

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
        block: Option<&'a Block>,
    },
}

struct ParsedFnArg<'a> {
    attrs: &'a [Attribute],
    name: &'a Ident,
    ty: &'a Type,
}

struct ParsedNewSplitFn {
    vis: Visibility,
    method_args: NewMethodArgs,
}

impl Default for ParsedNewSplitFn {
    fn default() -> Self {
        Self {
            vis: Visibility::Inherited,
            method_args: NewMethodArgs::default(),
        }
    }
}

#[derive(Default)]
struct ParsedNewFn(ParsedNewSplitFn);

struct ParsedSplitFn<'a> {
    vis: Visibility,
    attrs: &'a [Attribute],
}

impl Default for ParsedSplitFn<'static> {
    fn default() -> Self {
        Self {
            vis: Visibility::Inherited,
            attrs: &[],
        }
    }
}

struct ParsedUpdaterGetterFn<'a> {
    vis: &'a Visibility,
    method_args: UpdaterGetterMethodArgs,
    fn_args: Vec<ParsedFnArg<'a>>,
}

struct ParsedUpdaterFn<'a> {
    common: ParsedUpdaterGetterFn<'a>,
    ctx: Option<&'a Ident>,
    block: &'a Block,
}

struct ParsedGetterFn<'a> {
    common: ParsedUpdaterGetterFn<'a>,
    block: Option<&'a Block>,
    ret_ty: &'a Type,
}


pub fn build(item: InterfaceImpl, attrs: RawModelArgs) -> syn::Result<TokenStream> {
    Ok(ModelContext::parse(&item, attrs)?.generate())
}
