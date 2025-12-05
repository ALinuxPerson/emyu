use darling::FromAttributes;
use proc_macro2::Ident;
use crate::model::attr::raw::{MetaConfig, NameConfig};

#[derive(FromAttributes)]
#[darling(attributes(vye))]
pub struct MethodArgs {
    #[darling(default)]
    pub name: Option<NameConfig>,

    #[darling(default)]
    pub message: Option<Ident>,

    #[darling(default)]
    pub meta: Option<MetaConfig>,
}


