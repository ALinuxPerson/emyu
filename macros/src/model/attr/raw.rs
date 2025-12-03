use darling::ast::NestedMeta;
use darling::util::Override;
use darling::{FromAttributes, FromMeta};
use proc_macro2::{Ident, Span};
use syn::{Meta, Token, Visibility};

/// ```rust
/// #[vye::model(
///     /*
///     If passed, generates a wrapped dispatcher, updater, and getter struct. The following will
///     occur on the generated dispatcher struct:
///
///     - If updater fns are defined, a `fn updater() -> Updater` function is generated.
///     - If getter fns are defined, a `fn getter() -> Getter` function is generated.
///     - If no updater fns are defined, no updater struct is generated.
///     - If no getter fns are defined, no getter struct is generated.
///     - If both updater and getter fns are defined, a `fn split() -> (Updater, Getter)` function
///       is generated.
///     - By default, generated `new`, `updater`, `getter`, and `split` functions are private.
///
///     `#[vye::model]` must be applied to an `impl` block: `impl Model { /* ... */ }`
///     */
///
///     // Generates a dispatcher with default settings. Generated structs will be named according
///     // to the name of their model. E.g. if the model is `FooModel`, the structs will be named
///     // `FooDispatcher`, `FooUpdater`, and `FooGetter`.
///     dispatcher,
///
///     // Specifies the base of the generated struct names as an alternative. For example, if
///     // base is "Bar", names will be `BarDispatcher`, `BarUpdater`, and `BarGetter`.
///     dispatcher = "<base>",
///
///     // More customizability options
///     dispatcher(
///         // Name config
///         name(
///             base = "Baz", // see `dispatcher = "<base>"`
///             dispatcher = "FooDispatcher", // explicitly specifies the name of the dispatcher
///             updater = "FooUpdater",       // explicitly specifies the name of the updater
///             getter = "FooGetter",         // explicitly specifies the name of the getter
///             // `dispatcher`, `updater`, and `getter` take precedence over `base`.
///         ),
///         base = "Baz", // equivalent to `name(base = "Baz")`
///
///         // Visibility config
///         vis(
///             pub, // makes all generated structs public. Shorthand for `vis = "pub"`
///
///             // Specifies the base visibility of the generated structs. Takes precedence over `pub`.
///             base = "pub(crate)",
///             dispatcher = "pub(super)",      // specifies the visibility of the dispatcher struct
///             updater = "pub(in crate::baz)", // specifies the visibility of the updater struct
///             getter = "pub",                 // specifies the visibility of the getter struct
///             // `dispatcher`, `updater`, and `getter` take precedence over `base`.
///         ),
///         pub,                // equivalent to `vis(pub)`
///         vis = "pub(crate)", // equivalent to `vis(base = "pub(crate)")`
///
///         // Attributes config
///         meta(
///             // These attributes operate on the outer struct:
///             // `#[derive(Debug)] pub struct FooDispatcher(vye::Dispatcher<FooModel>);`
///
///             // Common attributes for all generated structs
///             base(derive(Debug)),
///             // `base`, `dispatcher`, `updater`, and `getter` can be specified multiple times
///             base(derive(PartialOrd)),
///             dispatcher(derive(Clone)),     // attributes for the generated `dispatcher` struct
///             updater(derive(PartialEq)),    // attributes for the generated `updater` struct
///             getter(derive(Serialize)),     // attributes for the generated `getter` struct
///
///             // These attributes operate on the inner value:
///             // `pub struct FooDispatcher(#[foo] vye::Dispatcher<FooModel>);`
///             inner(
///                 dispatcher(foo), // inner attributes for the generated `dispatcher` struct
///                 // `dispatcher`, `updater`, and `getter` can be specified multiple times
///                 dispatcher(bar),
///                 updater(baz),    // inner attributes for the generated `updater` struct
///                 getter(qux),     // inner attributes for the generated `getter` struct
///             ),
///         ),
///         meta(derive(Debug)), // equivalent to `meta(base(derive(Debug)))`
///         meta(derive(Clone)), // can also be specified multiple times
///
///         // Other config options
///         new = "pub", // specifies the visibility of the `new` function for the generated structs
///         new,         // if passed like a word, assumes `new = "pub"`
///
///         // specifies the visibility of the `split` function for the generated structs
///         split = "pub",
///         split,       // if passed like a word, assumes `split = "pub"`
///     ),
/// )]
/// impl FooModel {
///     // The visibility of this function determines the visibility of the generated `new`
///     // functions for the dispatcher, getter, and updater structs.
///     //
///     // To further customize the visibility, see below:
///     #[vye(
///         // Visibility config. Overrides the function declaration visibility.
///         vis(
///             // Specifies the visibility of the `new` function of the generated structs.
///             base = "pub(crate)",
///             dispatcher = "pub(super)",      // specifies visibility for the dispatcher struct
///             updater = "pub(in crate::baz)", // specifies visibility for the updater struct
///             getter = "pub",                 // specifies visibility for the getter struct
///             // `dispatcher`, `updater`, and `getter` take precedence over `base`.
///         ),
///         vis = "pub(crate)", // equivalent to `vis(base = "pub(crate)")`
///
///         // Attributes config:
///         // `#[some_meta] fn new(dispatcher: vye::Dispatcher<FooModel>) -> { /* ... */ }`
///         meta(
///             // Common attributes of the `new` function for all generated structs
///             base(foo),
///             // `base`, `dispatcher`, `updater`, and `getter` can be specified multiple times
///             base(bar),
///             dispatcher(baz), // attributes for the generated `dispatcher` struct
///             updater(qux),    // attributes for the generated `updater` struct
///             getter(quux),    // attributes for the generated `getter` struct
///         ),
///         meta(foo), // equivalent to `meta(base(derive(foo)))`
///         meta(bar), // can also be specified multiple times
///     )]
///     pub fn new();
///
///     // See `new` documentation. The same rules apply here.
///     pub fn split() -> (_, _);
///
///     // An updater function.
///     // The function must follow this shape:
///     // `$vis fn $fn_name[<T>](
///     //    &mut self,
///     //    field: i32,
///     //    [, generic: T,]
///     //    [, ctx: &mut UpdateContext<App>,]
///     //  )
///     // { /* ... */ }
///     // Meaning:
///     // - The header can only be the visibility followed by `fn`. No `async`, `const`, etc.
///     // - While type generics _are_ allowed, lifetimes are NOT allowed.
///     // - The `ctx` argument can be omitted for brevity.
///     // - The function must not return anything (void).
///     //
///     // The visibility of the function determines the visibility of the generated message struct
///     // and its visibility on the dispatcher and updater structs. If you would like to customize
///     // this, see below:
///     #[vye(
///         // Name config. If not passed, the message name will be the function name converted
///         // to PascalCase with "Message" appended to it. For example, `set_name` becomes
///         // `SetNameMessage`.
///         //
///         // Function names for the dispatcher and updater structs will inherit the name
///         // of the function.
///         name(
///             message = "SetNameMessage",
///             fns = "set_name",
///         ),
///
///         // Visibility config. Overrides the function declaration visibility.
///         vis(
///             // Specifies both the visibility of the message struct and the functions on the
///             // dispatcher and updater structs.
///             base = "pub(crate)",
///             message = "pub", // specifies the visibility of the message struct
///             dispatcher = "pub(super)",      // specifies visibility for the dispatcher struct
///             updater = "pub(in crate::baz)", // specifies visibility for the updater struct
///             // `message`, `dispatcher`, and `updater` take precedence over `base`.
///         ),
///         vis = "pub(crate)", // equivalent to `vis(base = "pub(crate)")`
///
///         // Attributes config, these can be specified multiple times:
///         // `#[some_meta] fn set_name(&mut self, message: SetNameMessage) -> { /* ... */ }`
///         meta(
///             message(derive(Clone)), // outer attributes for the message struct
///             fns(foo),               // common attributes for the dispatcher and updater functions
///             dispatcher(bar),        // attributes for the dispatcher function
///             updater(baz),           // attributes for the updater function
///         ),
///         meta(derive(Serialize)), // equivalent to `meta(message(derive(Serialize)))`
///     )]
///     pub(crate) fn set_name(
///         &mut self,
///
///         // Any attributes declared on these arguments are pasted on the fields of the message
///         // struct
///         #[serde(rename = "pangalan")]
///         name: String,
///         ctx: &mut UpdateContext<MyCoolApp>, // this can be omitted if not used
///     ) {
///         self.name = name;
///         ctx.mark_dirty(Region::Root);
///     }
///
///     // A getter function.
///     // The function must follow this shape:
///     // `$vis fn $fn_name[<T>](
///     //    &self,
///     //    field: i32,
///     //    [, generic: T,]
///     //  ) -> ReturnType
///     // [{ /* ... */ } | ;]
///     // Meaning:
///     // - The header can only be the visibility followed by `fn`. No `async`, `const`, etc.
///     // - While type generics _are_ allowed, lifetimes are NOT allowed.
///     // - The function must return a value.
///     // - The function body CAN be omitted if there are no other arguments, the function name
///     //   corresponds to a field in the model, and the type of the field implements Clone.
///     //
///     // The visibility of the function determines the visibility of the generated message struct
///     // and its visibility on the dispatcher and getter structs. If you would like to customize
///     // this, see below:
///     #[vye(
///         // Name config. If not passed, the message name will be the function name converted
///         // to PascalCase with "Get" and "Message" as its prefix and suffix respectively.
///         // For example, `location` becomes `GetLocationMessage`.
///         //
///         // Function names for the dispatcher and getter structs will inherit the name
///         // of the function.
///         name(
///             message = "GetLocationMessage",
///             fns = "location",
///         ),
///
///         // Visibility config. Overrides the function declaration visibility.
///         vis(
///             // Specifies both the visibility of the message struct and the functions on the
///             // dispatcher and getter structs.
///             base = "pub(crate)",
///             message = "pub", // specifies the visibility of the message struct
///             dispatcher = "pub(super)",      // specifies visibility for the dispatcher struct
///             getter = "pub(in crate::baz)",  // specifies visibility for the getter struct
///             // `message`, `dispatcher`, and `getter` take precedence over `base`.
///         ),
///         vis = "pub(crate)", // equivalent to `vis(base = "pub(crate)")`
///
///         // Attributes config, these can be specified multiple times:
///         // `#[some_meta] fn location(&self, message: GetLocationMessage) -> String { /* ... */ }`
///         meta(
///             message(derive(Clone)), // outer attributes for the message struct
///             fns(foo),               // common attributes for the dispatcher and getter functions
///             dispatcher(bar),        // attributes for the dispatcher function
///             getter(baz),            // attributes for the getter function
///         ),
///         meta(derive(Serialize)), // equivalent to `meta(message(derive(Serialize)))`
///     )]
///     pub(super) fn location(
///         &self,
///         // Any attributes declared on these arguments are pasted on the fields of the message
///         // struct
///         #[serde(rename = "makeLowercase")]
///         make_lowercase: bool,
///     ) -> String {
///         if make_lowercase {
///             self.location.to_lowercase()
///         } else {
///             self.location.clone()
///         }
///     }
///
///     // This is equivalent to `Clone::clone(&self.name)` because the body is omitted.
///     fn name(&self) -> String;
/// }
/// ```
#[derive(FromMeta)]
#[darling(derive_syn_parse)]
pub struct ModelArgs {
    #[darling(default)]
    pub dispatcher: Option<DispatcherDef>,
}

#[derive(FromAttributes)]
#[darling(attributes(vye))]
pub struct MethodArgs {
    #[darling(default)]
    pub name: Option<NameConfig>,

    #[darling(default)]
    pub vis: Option<VisConfig>,

    #[darling(multiple)]
    pub meta: Vec<MetaConfig>,
}

impl MethodArgs {
    pub fn vis(&self) -> MaybeVisConfig<'_> {
        MaybeVisConfig {
            config: self.vis.as_ref(),
            is_pub: false,
        }
    }

    pub fn meta(&self) -> MetaConfigs<'_> {
        MetaConfigs(&self.meta)
    }
}

pub enum DispatcherDef {
    Default,
    Base(Ident),
    Config(Box<DispatcherConfig>),
}

impl DispatcherDef {
    pub fn into_config(self) -> DispatcherConfig {
        match self {
            Self::Default => DispatcherConfig::default(),
            Self::Base(base) => DispatcherConfig {
                base: Some(base),
                ..DispatcherConfig::default()
            },
            Self::Config(config) => *config,
        }
    }
}

impl FromMeta for DispatcherDef {
    // #[vye::model(dispatcher)]
    fn from_word() -> darling::Result<Self> {
        Ok(Self::Default)
    }

    // #[vye::model(dispatcher(...))]
    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        Ok(Self::Config(Box::new(DispatcherConfig::from_list(items)?)))
    }

    // #[vye::model(dispatcher = "Foo")]
    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(Self::Base(Ident::new(value, Span::call_site())))
    }
}

#[derive(FromMeta, Default)]
pub struct DispatcherConfig {
    #[darling(default)]
    pub name: Option<NameConfig>,

    #[darling(default)]
    pub base: Option<Ident>,

    #[darling(default)]
    pub vis: Option<VisConfig>,

    #[darling(default, rename = "pub")]
    pub is_pub: bool,

    #[darling(multiple)]
    pub meta: Vec<MetaConfig>,

    #[darling(default)]
    pub new: Override<Visibility>,

    #[darling(default)]
    pub split: Override<Visibility>,
}

impl DispatcherConfig {
    #[allow(clippy::new_ret_no_self, clippy::wrong_self_convention)]
    pub fn new(&self) -> Visibility {
        self.new.clone().unwrap_or(Visibility::Inherited)
    }

    pub fn split(&self) -> Visibility {
        self.split.clone().unwrap_or(Visibility::Inherited)
    }
}

impl DispatcherConfig {
    pub fn vis(&self) -> MaybeVisConfig<'_> {
        MaybeVisConfig {
            config: self.vis.as_ref(),
            is_pub: self.is_pub,
        }
    }

    pub fn meta(&self) -> MetaConfigs<'_> {
        MetaConfigs(&self.meta)
    }
}

#[derive(FromMeta, Default)]
pub struct NameConfig {
    #[darling(default)]
    pub base: Option<Ident>,

    #[darling(default)]
    pub dispatcher: Option<Ident>,

    #[darling(default)]
    pub updater: Option<Ident>,

    #[darling(default)]
    pub getter: Option<Ident>,

    #[darling(default)]
    pub message: Option<Ident>,

    #[darling(default)]
    pub fns: Option<Ident>,
}

pub enum VisConfig {
    Simple(Visibility),
    Complex(VisFields),
}

impl VisConfig {
    fn field_with(
        &self,
        is_pub: bool,
        f: impl FnOnce(&VisFields) -> &Option<Visibility>,
    ) -> Visibility {
        match self {
            Self::Simple(vis) => vis.clone(),
            Self::Complex(fields) => f(fields)
                .as_ref()
                .or(fields.getter.as_ref())
                .cloned()
                .unwrap_or_else(|| {
                    if fields.is_pub || is_pub {
                        Visibility::Public(Token![pub](Span::call_site()))
                    } else {
                        Visibility::Inherited
                    }
                }),
        }
    }

    pub fn dispatcher_with(&self, is_pub: bool) -> Visibility {
        self.field_with(is_pub, |f| &f.dispatcher)
    }

    pub fn dispatcher(&self) -> Visibility {
        self.dispatcher_with(false)
    }

    pub fn updater_with(&self, is_pub: bool) -> Visibility {
        self.field_with(is_pub, |f| &f.updater)
    }

    pub fn updater(&self) -> Visibility {
        self.updater_with(false)
    }

    pub fn getter_with(&self, is_pub: bool) -> Visibility {
        self.field_with(is_pub, |f| &f.getter)
    }

    pub fn getter(&self) -> Visibility {
        self.getter_with(false)
    }

    pub fn message_with(&self, is_pub: bool) -> Visibility {
        self.field_with(is_pub, |f| &f.message)
    }

    pub fn message(&self) -> Visibility {
        self.message_with(false)
    }
}

impl FromMeta for VisConfig {
    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        Ok(Self::Complex(VisFields::from_list(items)?))
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(Self::Simple(Visibility::from_string(value)?))
    }
}

#[derive(FromMeta)]
pub struct VisFields {
    #[darling(default)]
    pub base: Option<Visibility>,

    #[darling(default)]
    pub dispatcher: Option<Visibility>,

    #[darling(default)]
    pub updater: Option<Visibility>,

    #[darling(default)]
    pub getter: Option<Visibility>,

    #[darling(default)]
    pub message: Option<Visibility>,

    #[darling(default, rename = "pub")]
    pub is_pub: bool,
}

pub struct MaybeVisConfig<'a> {
    config: Option<&'a VisConfig>,
    is_pub: bool,
}

impl<'a> MaybeVisConfig<'a> {
    fn field_vis(&self, f: impl FnOnce(&'a VisConfig) -> Visibility) -> Visibility {
        self.config.map(f).unwrap_or(Visibility::Inherited)
    }

    pub fn dispatcher(&self) -> Visibility {
        self.field_vis(|v| v.dispatcher_with(self.is_pub))
    }

    pub fn updater(&self) -> Visibility {
        self.field_vis(|v| v.updater_with(self.is_pub))
    }

    pub fn getter(&self) -> Visibility {
        self.field_vis(|v| v.getter_with(self.is_pub))
    }

    pub fn message(&self) -> Visibility {
        self.field_vis(|v| v.message_with(self.is_pub))
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
    fn field_with(&self, f: impl FnOnce(&Self) -> &Vec<Meta>) -> impl Iterator<Item = &Meta> {
        self.base.iter().chain(f(self))
    }

    pub fn dispatcher(&self) -> impl Iterator<Item = &Meta> {
        self.field_with(|m| &m.dispatcher)
    }

    pub fn updater(&self) -> impl Iterator<Item = &Meta> {
        self.field_with(|m| &m.updater)
    }

    pub fn getter(&self) -> impl Iterator<Item = &Meta> {
        self.field_with(|m| &m.getter)
    }

    pub fn message(&self) -> impl Iterator<Item = &Meta> {
        self.field_with(|m| &m.message)
    }
}

impl MetaConfig {
    pub fn fns(&self) -> impl Iterator<Item = &Meta> {
        self.field_with(|m| &m.fns)
    }

    pub fn dispatcher_fn(&self) -> impl Iterator<Item = &Meta> {
        self.fns().chain(&self.dispatcher)
    }

    pub fn updater_fn(&self) -> impl Iterator<Item = &Meta> {
        self.fns().chain(&self.updater)
    }

    pub fn getter_fn(&self) -> impl Iterator<Item = &Meta> {
        self.fns().chain(&self.getter)
    }
}

#[derive(Copy, Clone)]
pub struct MetaConfigs<'a>(&'a [MetaConfig]);

impl<'a> MetaConfigs<'a> {
    fn field<I: Iterator<Item = &'a Meta>>(
        self,
        f: impl FnMut(&'a MetaConfig) -> I,
    ) -> impl Iterator<Item = &'a Meta> {
        self.0.iter().flat_map(f)
    }

    pub fn dispatcher(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.dispatcher())
    }

    pub fn updater(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.updater())
    }

    pub fn getter(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.getter())
    }

    pub fn message(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.message())
    }

    pub fn fns(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.fns())
    }

    pub fn dispatcher_fn(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.dispatcher_fn())
    }

    pub fn updater_fn(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.updater_fn())
    }

    pub fn getter_fn(self) -> impl Iterator<Item = &'a Meta> {
        self.field(|m| m.getter_fn())
    }
}

impl<'a> MetaConfigs<'a> {
    fn field_inner_meta(
        self,
        f: impl FnMut(&'a InnerMetaConfig) -> &'a Vec<Meta>,
    ) -> impl Iterator<Item = &'a Meta> {
        self.0.iter().filter_map(|m| m.inner.as_ref()).flat_map(f)
    }

    pub fn dispatcher_inner(self) -> impl Iterator<Item = &'a Meta> {
        self.field_inner_meta(|im| &im.dispatcher)
    }

    pub fn updater_inner(self) -> impl Iterator<Item = &'a Meta> {
        self.field_inner_meta(|im| &im.updater)
    }

    pub fn getter_inner(self) -> impl Iterator<Item = &'a Meta> {
        self.field_inner_meta(|im| &im.getter)
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
