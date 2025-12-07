use crate::model::attr::raw::ProcessedMeta;
use crate::model::attr::{ModelArgs, ModelProperties, NewMethodArgs};
use crate::model::{
    ModelContext, ParsedFnArg, ParsedGetterFn, ParsedNewFn, ParsedUpdaterFn, ParsedUpdaterGetterFn,
};
use crate::utils::ThisCrate;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use std::iter;
use syn::{TypePath, Visibility};

impl<'a> ModelContext<'a> {
    pub(super) fn generate(&self) -> TokenStream {
        let impl_model = self.generate_impl_model();
        let message = self.generate_message();
        let updater = self.generate_updater();
        let getter = self.generate_getter();
        quote! {
            #impl_model
            #message
            #updater
            #getter
        }
    }

    fn generate_impl_model(&self) -> TokenStream {
        let crate_ = &self.crate_;
        let model_ty = self.model_ty;
        let for_app = &self.args.for_app;
        let message_name = &self.args.message.name;
        let model_fns = self
            .updaters
            .iter()
            .map(|u| u.generate_model_fn(crate_, for_app));
        let match_cases = self
            .updaters
            .iter()
            .map(|u| u.generate_match_case(message_name));
        let accumulate_signals = self
            .getters
            .iter()
            .map(|g| g.generate_accumulate_signals(crate_));

        quote! {
            impl #model_ty {
                #(#model_fns)*
            }
            impl #crate_::Model for #model_ty {
                type ForApp = #for_app;
                type Message = #message_name;

                fn update(&mut self, message: #message_name, ctx: &mut #crate_::UpdateContext<#for_app>) {
                    match message {
                        #(#match_cases)*
                    }
                }

                fn __accumulate_signals(
                    &self,
                    signals: &mut #crate_::__macros::alloc::collections::VecDeque<#crate_::__macros::Shared<dyn #crate_::__macros::FlushSignals>>,
                    _: #crate_::__private::Token,
                ) {
                    #(#accumulate_signals)*
                }
            }
        }
    }
}

impl<'a> ModelContext<'a> {
    fn generate_message(&self) -> TokenStream {
        let vis = &self.struct_vis;
        let name = &self.args.message.name;
        let outer_meta = &self.args.message.outer_meta;
        let variants = self.updaters.iter().map(|u| u.generate_message_variant());

        quote! {
            #(#[#outer_meta])*
            #vis enum #name {
                #(#variants)*
            }
        }
    }
}

impl<'a> ModelContext<'a> {
    fn generate_any_struct(
        &self,
        props_accessor: impl FnOnce(&ModelArgs) -> &ModelProperties,
        f: impl FnOnce(
            &ThisCrate,
            &Visibility,
            &TypePath,
            &Ident,
            &[ProcessedMeta],
            &[ProcessedMeta],
        ) -> TokenStream,
    ) -> TokenStream {
        let props = props_accessor(&self.args);
        let crate_ = &self.crate_;
        let vis = self.struct_vis;
        let model_ty = self.model_ty;
        let name = &props.name;
        let outer_meta = &props.outer_meta;
        let inner_meta = &props.inner_meta;
        f(crate_, vis, model_ty, name, outer_meta, inner_meta)
    }
}

impl<'a> ModelContext<'a> {
    fn generate_updater_getter_impls<AnyFn>(
        &self,
        trait_name: &'static str,
        inner_ty: TokenStream,
        props_accessor: impl FnOnce(&ModelArgs) -> &ModelProperties,
        new_fn: impl FnOnce(&ParsedNewFn, &ThisCrate, &TypePath) -> TokenStream,
        updater_getter_accessor: impl FnOnce(&Self) -> &Vec<AnyFn>,
        generate_fn: impl Fn(&AnyFn) -> TokenStream,
    ) -> TokenStream {
        let trait_name = Ident::new(trait_name, Span::call_site());
        let props = props_accessor(&self.args);
        let updater_getter = updater_getter_accessor(self);
        let crate_ = &self.crate_;
        let updater_getter_name = &props.name;
        let model_ty = &self.model_ty;
        let new_fn = new_fn(&self.new_fn, crate_, self.model_ty);
        let fns = updater_getter.iter().map(generate_fn);

        quote! {
            impl #crate_::#trait_name for #updater_getter_name {
                type Model = #model_ty;
                fn __new(
                    value: #inner_ty<#model_ty>,
                    _token: #crate_::__private::Token,
                ) -> Self {
                    Self(value)
                }
            }
            impl #crate_::__private::Sealed for #updater_getter_name {}
            impl #updater_getter_name {
                #new_fn
                #(#fns)*
            }
        }
    }
}

impl<'a> ModelContext<'a> {
    fn generate_updater(&self) -> TokenStream {
        let struct_decl = self.generate_updater_struct();
        let impls = self.generate_updater_impls();
        quote! {
            #struct_decl
            #impls
        }
    }

    fn generate_updater_struct(&self) -> TokenStream {
        self.generate_any_struct(
            |a| &a.updater,
            |crate_, vis, model_ty, updater_name, outer_meta, inner_meta| {
                quote! {
                    #(#[#outer_meta])*
                    #vis struct #updater_name(#(#[#inner_meta])* #crate_::Updater<#model_ty>);
                }
            },
        )
    }

    fn generate_updater_impls(&self) -> TokenStream {
        let crate_ = &self.crate_;
        self.generate_updater_getter_impls(
            "WrappedUpdater",
            quote! { #crate_::Updater },
            |a| &a.updater,
            |new_fn, crate_, dispatcher_name| new_fn.generate_for_updater(crate_, dispatcher_name),
            |m| &m.updaters,
            |u| u.generate_updater_fn(&self.args.message.name),
        )
    }
}

impl<'a> ModelContext<'a> {
    fn generate_getter(&self) -> TokenStream {
        let struct_decl = self.generate_getter_struct();
        let impls = self.generate_getter_impls();
        let message_structs_and_trait_impls = self
            .getters
            .iter()
            .map(|g| g.generate_message_struct_and_trait_impls(&self.crate_, self.model_ty));

        quote! {
            #struct_decl
            #impls
            #(#message_structs_and_trait_impls)*
        }
    }

    fn generate_getter_struct(&self) -> TokenStream {
        self.generate_any_struct(
            |a| &a.getter,
            |crate_, vis, model_ty, getter_name, outer_meta, inner_meta| {
                quote! {
                    #(#[#outer_meta])*
                    #vis struct #getter_name(#(#[#inner_meta])* #crate_::Getter<#model_ty>);
                }
            },
        )
    }

    fn generate_getter_impls(&self) -> TokenStream {
        let crate_ = &self.crate_;
        self.generate_updater_getter_impls(
            "WrappedGetter",
            quote! { #crate_::Getter },
            |a| &a.getter,
            |new_fn, crate_, dispatcher_name| new_fn.generate_for_getter(crate_, dispatcher_name),
            |m| &m.getters,
            |g| g.generate_getter_fn(crate_),
        )
    }
}

impl<'a> ParsedFnArg<'a> {
    fn generate_fn_arg(&self) -> TokenStream {
        let Self { name, ty, .. } = *self;
        quote! { #name: #ty }
    }

    fn generate_field(&self) -> TokenStream {
        let Self { attrs, name, ty } = *self;
        quote! { #(#[#attrs])* #name: #ty }
    }
}

impl ParsedNewFn {
    fn generate(
        &self,
        crate_: &ThisCrate,
        wrapped_ty: &'static str,
        inner_ty: TokenStream,
        model_ty: &TypePath,
        meta_fn: impl FnOnce(&NewMethodArgs) -> &Vec<ProcessedMeta>,
    ) -> TokenStream {
        let wrapped_ty = Ident::new(wrapped_ty, Span::call_site());
        let vis = &self.vis;
        let meta = meta_fn(&self.method_args);

        quote! {
            #(#[#meta])*
            #vis fn new(value: #inner_ty<#model_ty>) -> Self {
                #crate_::#wrapped_ty::__new(value, #crate_::__token())
            }
        }
    }

    fn generate_for_updater(&self, crate_: &ThisCrate, model_ty: &TypePath) -> TokenStream {
        self.generate(
            crate_,
            "WrappedUpdater",
            quote! { #crate_::Updater },
            model_ty,
            |args| &args.updater_meta,
        )
    }

    fn generate_for_getter(&self, crate_: &ThisCrate, model_ty: &TypePath) -> TokenStream {
        self.generate(
            crate_,
            "WrappedGetter",
            quote! { #crate_::Getter },
            model_ty,
            |args| &args.getter_meta,
        )
    }
}

impl<'a> ParsedUpdaterGetterFn<'a> {
    fn generate_updater_getter_fn(
        &self,
        f: impl FnOnce(&Visibility, &[ProcessedMeta], &Ident, &Ident) -> TokenStream,
    ) -> TokenStream {
        let vis = self.vis;
        let meta = &self.method_args.fn_meta;
        let fn_name = &self.method_args.fn_name;
        let message_name = &self.method_args.message.name;

        f(vis, meta, fn_name, message_name)
    }
}

impl<'a> ParsedUpdaterFn<'a> {
    fn generate_message_variant(&self) -> TokenStream {
        let variant_name = &self.common.method_args.message.name;
        let outer_meta = &self.common.method_args.message.outer_meta;
        let fields = self.fn_args.iter().map(|fa| fa.generate_field());

        quote! {
            #(#[#outer_meta])*
            #variant_name { #(#fields),* },
        }
    }

    fn generate_match_case(&self, message_name: &Ident) -> TokenStream {
        let variant_name = &self.common.method_args.message.name;
        let fn_name = format_ident!("__{}", self.common.method_args.fn_name);
        let field_names = self.fn_args.iter().map(|fa| fa.name).collect::<Vec<_>>();
        let field_names_and_ctx = field_names
            .iter()
            .copied()
            .cloned()
            .chain(iter::once(Ident::new("ctx", Span::call_site())));

        quote! {
            #message_name::#variant_name { #(#field_names),* } => self.#fn_name(#(#field_names_and_ctx),*),
        }
    }

    fn generate_model_fn(&self, crate_: &ThisCrate, for_app: &Ident) -> TokenStream {
        let fn_name = format_ident!("__{}", self.common.method_args.fn_name);
        let fn_args = self.fn_args.iter().map(|fa| fa.generate_fn_arg()).chain({
            let ctx = self
                .ctx
                .cloned()
                .unwrap_or_else(|| Ident::new("_", Span::call_site()));
            iter::once(quote! { #ctx: &mut #crate_::UpdateContext<#for_app> })
        });
        let block = self.block;
        quote! {
            fn #fn_name(&mut self, #(#fn_args),*) { #block }
        }
    }

    fn generate_updater_fn(&self, message_name: &Ident) -> TokenStream {
        self.common
            .generate_updater_getter_fn(|vis, meta, fn_name, variant_name| {
                let field_names = self.fn_args.iter().map(|fa| fa.name).collect::<Vec<_>>();
                let fn_args = self
                    .fn_args
                    .iter()
                    .map(|fa| fa.generate_fn_arg())
                    .collect::<Vec<_>>();

                quote! {
                    #(#[#meta])*
                    #vis async fn #fn_name(&mut self, #(#fn_args),*) {
                        self.0.send(#message_name::#variant_name { #(#field_names),* }).await
                    }
                }
            })
    }
}

impl<'a> ParsedGetterFn<'a> {
    fn generate_accumulate_signals(&self, crate_: &ThisCrate) -> TokenStream {
        let field_name = &self.common.method_args.fn_name;

        quote! {
            signals.push_back(self.#field_name.__to_dyn_flush_signals(#crate_::__token()));
        }
    }

    fn generate_message_struct(&self) -> TokenStream {
        let vis = self.common.vis;
        let outer_meta = &self.common.method_args.message.outer_meta;
        let name = &self.common.method_args.message.name;

        quote! {
            #(#[#outer_meta])*
            #vis struct #name;
        }
    }

    fn generate_message_struct_and_trait_impls(
        &self,
        crate_: &ThisCrate,
        model_ty: &TypePath,
    ) -> TokenStream {
        let struct_decl = self.generate_message_struct();
        let message_name = &self.common.method_args.message.name;
        let field_name = &self.common.method_args.fn_name;
        let ret_ty = self.ret_ty;

        quote! {
            #struct_decl
            impl #crate_::ModelGetterMessage for #message_name {
                type Data = #ret_ty;
            }
            impl #crate_::ModelGetterHandler<#message_name> for #model_ty {
                fn getter(
                    &self,
                ) -> #crate_::Signal<#ret_ty> {
                    ::core::clone::Clone::clone(&self.#field_name)
                }
            }
        }
    }

    fn generate_getter_fn(&self, crate_: &ThisCrate) -> TokenStream {
        self.common
            .generate_updater_getter_fn(|vis, meta, fn_name, message_name| {
                let ret_ty = self.ret_ty;
                quote! {
                    #(#[#meta])*
                    #vis fn #fn_name(&mut self) -> #crate_::Signal<#ret_ty> {
                        self.0.get::<#message_name>()
                    }
                }
            })
    }
}
