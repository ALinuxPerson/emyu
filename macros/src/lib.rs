mod model;
mod utils;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn model(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(args as model::RawModelArgs);
    let input = syn::parse_macro_input!(input as utils::InterfaceImpl);
    match model::build(input, args) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
