#[macro_export]
macro_rules! command {
    ($($tt:tt)*) => {
        $crate::__parse_struct!(
            @impl $crate::__command_impl;
            $($tt)*
        );
    };
}

#[macro_export]
macro_rules! message {
    ($($tt:tt)*) => {
        $crate::__parse_struct!(
            @impl $crate::__message_impl;
            $($tt)*
        );
    };
}

#[macro_export]
macro_rules! getter {
    ($($tt:tt)*) => {
        $crate::__parse_struct!(
            @impl $crate::__getter_impl;
            $($tt)*
        );
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __parse_struct {
    // 1. Named Fields: struct Name { ... }
    (
        @impl $callback:path;
        $(#[$meta:meta])*
        $vis:vis struct $Name:ident {
            $($fields:tt)*
        } $($rest:tt)*
    ) => {
        $(#[$meta])*
        $vis struct $Name {
            $($fields)*
        }
        $crate::__parse_header!(@impl $callback; $Name; $($rest)*);
    };

    // 2. Tuple Struct: struct Name( ... );
    (
        @impl $callback:path;
        $(#[$meta:meta])*
        $vis:vis struct $Name:ident (
            $($fields:tt)*
        ) $($rest:tt)*
    ) => {
        $(#[$meta])*
        $vis struct $Name ( $($fields)* );
        $crate::__parse_header!(@impl $callback; $Name; $($rest)*);
    };

    // 3. Unit Struct: struct Name;
    (
        @impl $callback:path;
        $(#[$meta:meta])*
        $vis:vis struct $Name:ident $($rest:tt)*
    ) => {
        $(#[$meta])*
        $vis struct $Name;
        $crate::__parse_header!(@impl $callback; $Name; $($rest)*);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __parse_header {
    // hacky workaround for `__getter_impl`, making this macro not really generic anymore is it
    (
        @impl $callback:path;
        $Name:ident;
        for $Model:ty where Data = $Ret:ty;
        $($rest:tt)*
    ) => {
        $callback!($Name $Model; where Data = $Ret; $($rest)*);
    };
    (
        @impl $callback:path;
        $Name:ident;
        for $Model:ty;
        $($rest:tt)*
    ) => {
        $callback!($Name $Model; ; $($rest)*);
    };
}


#[macro_export]
#[doc(hidden)]
macro_rules! __command_impl {
    (
        $CommandName:ident $ModelName:ty;
        ;
        | $this:ident, $ctx:ident | $body:expr
    ) => {
        #[$crate::__macros::async_trait]
        impl $crate::Command for $CommandName {
            type ForApp = <$ModelName as $crate::Model>::ForApp;

            async fn apply(&mut self, ctx: &mut $crate::ApplyContext<'_, Self::ForApp>) {
                async fn f(
                    $this: &mut $CommandName,
                    $ctx: &mut $crate::ApplyContext<'_, <$CommandName as $crate::Command>::ForApp>,
                ) {
                    $body
                }
                f(self, ctx).await
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __message_impl {
    (
        $MsgName:ident $ModelName:ty;
        $body:expr
    ) => {
        impl $crate::ModelMessage for $MsgName {}

        impl $crate::ModelHandler<$MsgName> for $ModelName {
            fn update(&mut self, message: $MsgName, ctx: &mut $crate::UpdateContext<<Self as $crate::Model>::ForApp>) {
                let f: fn(&mut $ModelName, $MsgName, &mut $crate::UpdateContext<<Self as $crate::Model>::ForApp>) = $body;
                f(self, message, ctx);
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __getter_impl {
    (
        $MsgName:ident $ModelName:ty;
        where Data = $Ret:ty;
        | $this:ident, $msg:ident | $body:expr
    ) => {
        impl $crate::ModelGetterMessage for $MsgName {
            type Data = $Ret;
        }

        impl $crate::ModelGetterHandler<$MsgName> for $ModelName {
            fn getter(&self, message: $MsgName) -> $Ret {
                let f: fn(&$ModelName, $MsgName) -> $Ret = | $this, $msg | $body;
                f(self, message)
            }
        }
    };
}
