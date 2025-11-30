mod command_message;
mod getter;

#[macro_export]
macro_rules! updater {
    (
        $(#[$($meta:meta)*])*
        $vis:vis struct $UpdaterName:ident for $ModelName:ty {
            $(
            $(#[$($fn_meta:meta)*])*
            $fn_vis:vis fn $fn_name:ident($($fn_arg:ident: $fn_arg_ty:ty),*) -> $MsgType:ident $body:block
            )*
        }
    ) => {
        $(#[$($meta)*])*
        $vis struct $UpdaterName($crate::Updater<$ModelName>);

        impl $UpdaterName {
            $(
                $(#[$($fn_meta)*])*
                $fn_vis async fn $fn_name(&mut self, $($fn_arg: $fn_arg_ty),*) {
                    let f: fn($($fn_arg_ty),*) -> $MsgType = |$($fn_arg: $fn_arg_ty),*| $body;
                    self.0.send(f($($fn_arg),*)).await
                }
            )*
        }
    };
}