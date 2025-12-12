use crate::model::attr::raw::{MetaConfig, NameConfig};
use darling::FromAttributes;
use proc_macro2::Ident;

#[derive(FromAttributes)]
#[darling(attributes(emyu))]
pub struct MethodArgs {
    #[darling(default)]
    pub name: Option<NameConfig>,

    #[darling(default)]
    pub message: Option<Ident>,

    #[darling(default)]
    pub meta: Option<MetaConfig>,
}
