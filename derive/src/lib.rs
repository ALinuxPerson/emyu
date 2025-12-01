use proc_macro::TokenStream;

mod wrap_dispatcher;
mod updater {

}
mod getter {

}
mod message {

}

#[proc_macro]
pub fn wrap_dispatcher(input: TokenStream) -> TokenStream {
    let def = syn::parse_macro_input!(input as wrap_dispatcher::DispatcherDef);
    match def.expand() {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn updater(args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn getter(args: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[proc_macro_attribute]
pub fn message(args: TokenStream, input: TokenStream) -> TokenStream {
    input
}
