#[macro_export]
macro_rules! define_command {
    (
        $(#[$meta:meta])*
        $name:ident($($type:ty),* $(,)?) for $app:ty;
        fn($self:ident, $ctx:ident) $body:block
    ) => {
        $(#[$meta])*
        #[derive(Debug)]
        pub struct $name($($type),*);

        #[::async_trait::async_trait]
        impl $crate::Command for $name {
            type ForApp = $app;

            async fn apply(&mut self, $ctx: &mut ExecContext<'_, Self::ForApp>) {
                let $self  = self;
                $body
            }
        }
    };
}

#[macro_export]
macro_rules! message_handler {
    {
        $(#[$meta:meta])*
        $vis:vis struct $MsgName:ident {
            $($fields:tt)*
        } for $ModelName:ty;
        $body:expr
    } => {
        $(#[$meta])*
        $vis struct $MsgName {
            $($fields)*
        }
        $crate:__message_handler_common!($MsgName $ModelName; $body);
    };

    {
        $(#[$meta:meta])*
        $vis:vis struct $MsgName:ident (
            $($fields:tt)*
        ) for $ModelName:ty;
        $body:expr
    } => {
        $(#[$meta])*
        $vis struct $MsgName ( $($fields)* );
        $crate::__message_handler_common!($MsgName $ModelName; $body);
    };

    {
        $(#[$meta:meta])*
        $vis:vis struct $MsgName:ident for $ModelName:ty;
        $body:expr
    } => {
        $(#[$meta])*
        $vis struct $MsgName;
        $crate::__message_handler_common!($MsgName $ModelName; $body);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __message_handler_common {
    ($MsgName:ident $ModelName:ty; $body:expr) => {
        impl $crate::ModelMessage for $MsgName {}

        impl $crate::ModelHandler<$MsgName> for $ModelName {
            fn update(&mut self, message: $MsgName, ctx: &mut $crate::UpdateContext<<Self as $crate::Model>::ForApp>) {
                let f: fn(&mut $ModelName, $MsgName, &mut $crate::UpdateContext<<Self as $crate::Model>::ForApp>) = $body;
                f(self, message, ctx);
            }
        }
    };
}
