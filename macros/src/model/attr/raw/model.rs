use crate::model::attr::raw::{MetaConfig, NameConfig};
use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro2::{Ident, Span};
use syn::Meta;

/// ```rust
/// #[emyu::model(
///     // Specify the application this model is for. Required.
///     for_app = "MyCoolApp",
///
///     // Specify the name of the generated message enum. By default, this is the model name
///     // stripped of "Model" and suffixed with "Message". For example, `FooModel` becomes
///     // `FooMessage`. The visibility of the `impl` block determines the visibility of the
///     // generated message enum.
///     message = "MyCoolMessage",
///
///     // More customizability options
///     message(
///         name = "MyCustomMessageEnum",   // explicitly specifies the name of the message enum
///
///         // Specifies the outer attributes of the message enum
///         // `#[derive(Debug, Clone)] pub enum MyCustomMessageEnum { /* ... */ }`
///         meta(derive(Debug, Clone, Serialize)),
///         meta(serde(tag = "type")), // this can be specified multiple times
///     ),
///
///     /*
///     If passed, generates a wrapped updater and getter struct. Required.
///
///     `#[emyu::model]` must be applied to an `impl` block: `pub impl Model { /* ... */ }`
///
///     The visibility of the `impl` block can be declared. This affects the visibility of the
///     generated updater and getter structs.
///     */
///
///     // Generates an updater and getter with default settings. Generated structs will be named
///     // according to the name of their model. E.g. if the model is `FooModel`, the structs will
///     // be named `FooUpdater` and `FooGetter`.
///     dispatcher,
///
///     // More customizability options
///     dispatcher(
///         // Name config
///         name(
///             updater = "FooUpdater",       // explicitly specifies the name of the updater
///             getter = "FooGetter",         // explicitly specifies the name of the getter
///         ),
///
///         // Attributes config
///         meta(
///             // These attributes operate on the outer struct:
///             // `#[derive(Debug)] pub struct FooUpdater(emyu::Updater<FooModel>);`
///
///             // Common attributes for all generated structs
///             base(derive(Debug)),
///             // `base`, `updater`, and `getter` can be specified multiple times
///             base(derive(PartialOrd)),
///             updater(derive(PartialEq)),    // attributes for the generated `updater` struct
///             getter(derive(Serialize)),     // attributes for the generated `getter` struct
///
///             // These attributes operate on the inner value:
///             // `pub struct FooUpdater(#[foo] emyu::Updater<FooModel>);`
///             inner(
///                 // `updater` and `getter` can be specified multiple times
///                 updater(baz),    // inner attributes for the generated `updater` struct
///                 getter(qux),     // inner attributes for the generated `getter` struct
///             ),
///         ),
///     ),
///
///     // (only when `frb-compat` feature is enabled) Adds special attributes and behavior for
///     // Flutter-Rust-Bridge compatibility.
///     frb,
/// )]
/// pub(crate) impl FooModel {
///     // The visibility of this function determines the visibility of the generated `new`
///     // functions for the getter and updater structs.
///     #[emyu(
///         // Attributes config:
///         // `#[some_meta] fn new(updater: emyu::Updater<FooModel>) -> { /* ... */ }`
///         meta(
///             // Common attributes of the `new` function for all generated structs
///             base(foo),
///             // `base`, `updater`, and `getter` can be specified multiple times
///             base(bar),
///             updater(qux),    // attributes for the generated `updater` struct
///             getter(quux),    // attributes for the generated `getter` struct
///         ),
///     )]
///     pub fn new();
///
///     // An updater function.
///     // The function must follow this shape:
///     // `$vis fn $fn_name(
///     //    &mut self,
///     //    field: i32,
///     //    [, ctx: &mut UpdateContext<App>,]
///     //  )
///     // { /* ... */ }
///     // Meaning:
///     // - The header can only be the visibility followed by `fn`. No `async`, `const`, etc.
///     // - Generics are NOT allowed.
///     // - The `ctx` argument can be omitted for brevity.
///     // - The function must not return anything (void).
///     //
///     // The visibility of the function determines its visibility on the updater struct.
///     //
///     // Function names for the updater struct will inherit the name of this function.
///     #[emyu(
///         // Name config. If not passed, the message name will be the function name converted
///         // to PascalCase. For example, `set_name` becomes `SetName`.
///         message = "SetName",
///
///         // Attributes config, these can be specified multiple times:
///         // `#[some_meta] fn set_name(&mut self, message: SetNameMessage) -> { /* ... */ }`
///         meta(
///             message(derive(Clone)), // outer attributes for the message struct
///             updater(baz),           // attributes for the updater function
///         ),
///     )]
///     pub(crate) fn set_name(
///         &mut self,
///
///         // Any attributes declared on these arguments are pasted on the fields of the message
///         // variant
///         #[serde(rename = "pangalan")]
///         name: String,
///         ctx: &mut UpdateContext<MyCoolApp>, // this can be omitted if not used
///     ) {
///         self.name = name;
///     }
///
///     // A getter function.
///     // The function must follow this shape:
///     // `$vis fn $field_name(&self) -> Signal<ReturnType>;
///     // Meaning:
///     // - The header CAN only be the visibility followed by `fn`. No `async`, `const`, etc.
///     // - Generics are NOT allowed.
///     // - The function must return a value wrapped in a `Signal<...>`.
///     // - The function body MUST be omitted.
///     // - The function name MUST correspond to a field on the model.
///     //
///     // The visibility of the function determines the visibility of the generated message struct
///     // and its visibility on the getter struct.
///     //
///     // Function name for the getter struct will inherit the name of this function.
///     #[emyu(
///         // Name config. If not passed, the message name will be the function name converted
///         // to PascalCase with "Get" and "Message" as its prefix and suffix respectively.
///         // For example, `location` becomes `GetLocationMessage`.
///         message = "GetLocationMessage",
///
///         // Attributes config, these can be specified multiple times:
///         // `#[some_meta] fn location(&self, message: GetLocationMessage) -> String { /* ... */ }`
///         meta(
///             message(derive(Clone)), // outer attributes for the message struct
///             getter(baz),            // attributes for the getter function
///         ),
///     )]
///     pub(super) fn location(&self) -> Signal<String>;
/// }
/// ```
#[derive(FromMeta)]
#[darling(derive_syn_parse)]
pub struct ModelArgs {
    pub for_app: Ident,

    #[darling(default)]
    pub message: MessageConfig,

    #[darling(default)]
    pub dispatcher: Option<DispatcherDef>,

    #[cfg(feature = "frb-compat")]
    #[darling(default, rename = "frb")]
    flutter_rust_bridge: bool,
}

impl ModelArgs {
    pub const fn flutter_rust_bridge(&self) -> bool {
        #[cfg(feature = "frb-compat")]
        {
            self.flutter_rust_bridge
        }
        #[cfg(not(feature = "frb-compat"))]
        {
            false
        }
    }
}

pub enum MessageDef {
    Named(Ident),
    Config(Box<MessageConfig>),
}

impl MessageDef {
    pub fn into_config(self) -> MessageConfig {
        match self {
            Self::Named(name) => MessageConfig {
                name: Some(name),
                ..Default::default()
            },
            Self::Config(config) => *config,
        }
    }
}

impl FromMeta for MessageDef {
    // #[emyu::model(message(...))]
    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        Ok(Self::Config(Box::new(MessageConfig::from_list(items)?)))
    }

    // #[emyu::model(message = "MyMessage")]
    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(Self::Named(Ident::new(value, Span::call_site())))
    }
}

#[derive(FromMeta, Default)]
pub struct MessageConfig {
    #[darling(default)]
    pub name: Option<Ident>,

    #[darling(multiple)]
    pub meta: Vec<Meta>,
}

pub enum DispatcherDef {
    Default,
    Config(Box<DispatcherConfig>),
}

impl DispatcherDef {
    pub fn into_config(self) -> DispatcherConfig {
        match self {
            Self::Default => DispatcherConfig::default(),
            Self::Config(config) => *config,
        }
    }
}

impl FromMeta for DispatcherDef {
    // #[emyu::model(dispatcher)]
    fn from_word() -> darling::Result<Self> {
        Ok(Self::Default)
    }

    // #[emyu::model(dispatcher(...))]
    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        Ok(Self::Config(Box::new(DispatcherConfig::from_list(items)?)))
    }
}

#[derive(FromMeta, Default)]
pub struct DispatcherConfig {
    #[darling(default)]
    pub name: Option<NameConfig>,

    #[darling(default)]
    pub meta: Option<MetaConfig>,
}
