mod model;
mod method;

pub use model::{ModelArgs, DispatcherConfig};
pub use method::MethodArgs;

use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::Meta;

#[derive(FromMeta, Default)]
pub struct NameConfig {
    #[darling(default)]
    pub dispatcher: Option<Ident>,

    #[darling(default)]
    pub updater: Option<Ident>,

    #[darling(default)]
    pub getter: Option<Ident>,
}

pub struct ProcessedMetaRef<'a>(&'a TokenStream);

impl<'a> ProcessedMetaRef<'a> {
    fn process(meta: &'a Meta) -> Self {
        // strip out the outer `meta(...)`
        Self(
            &meta
                .require_list()
                .expect("should always be a` MetaList`")
                .tokens,
        )
    }
    
    pub(crate) fn to_owned(self) -> ProcessedMeta {
        ProcessedMeta(self.0.clone())
    }
}

impl<'a> ToTokens for ProcessedMetaRef<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

#[derive(Debug)]
pub struct ProcessedMeta(pub(crate) TokenStream);

impl ToTokens for ProcessedMeta {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

#[derive(FromMeta)]
pub struct MetaConfig {
    #[darling(multiple)]
    pub base: Vec<Meta>,

    #[darling(multiple)]
    pub dispatcher: Vec<Meta>,

    #[darling(multiple)]
    pub updater: Vec<Meta>,

    #[darling(multiple)]
    pub getter: Vec<Meta>,

    #[darling(multiple)]
    pub message: Vec<Meta>,

    #[darling(multiple)]
    pub fns: Vec<Meta>,

    #[darling(default)]
    pub inner: Option<InnerMetaConfig>,
}

impl MetaConfig {
    fn field_with(
        &self,
        f: impl FnOnce(&Self) -> &Vec<Meta>,
    ) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.base
            .iter()
            .chain(f(self))
            .map(ProcessedMetaRef::process)
    }

    pub fn dispatcher(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.field_with(|m| &m.dispatcher)
    }

    pub fn updater(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.field_with(|m| &m.updater)
    }

    pub fn getter(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.field_with(|m| &m.getter)
    }

    pub fn message(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.field_with(|m| &m.message)
    }
}

impl MetaConfig {
    pub fn fns(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.field_with(|m| &m.fns)
    }

    pub fn dispatcher_fn(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.fns()
            .chain(self.dispatcher.iter().map(ProcessedMetaRef::process))
    }

    pub fn updater_fn(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.fns()
            .chain(self.updater.iter().map(ProcessedMetaRef::process))
    }

    pub fn getter_fn(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.fns()
            .chain(self.getter.iter().map(ProcessedMetaRef::process))
    }
}

#[derive(FromMeta)]
pub struct InnerMetaConfig {
    #[darling(multiple)]
    pub dispatcher: Vec<Meta>,

    #[darling(multiple)]
    pub updater: Vec<Meta>,

    #[darling(multiple)]
    pub getter: Vec<Meta>,
}

impl InnerMetaConfig {
    pub fn dispatcher(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.dispatcher
            .iter()
            .map(ProcessedMetaRef::process)
    }
    
    pub fn updater(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.updater.iter().map(ProcessedMetaRef::process)
    }
    
    pub fn getter(&self) -> impl Iterator<Item = ProcessedMetaRef<'_>> {
        self.getter.iter().map(ProcessedMetaRef::process)
    }
}
