#[cfg(feature = "thread-safe")]
mod global {
    #[cfg(feature = "tokio")]
    pub mod tokio {
        use crate::host::spawner::Spawner;
        use crate::maybe::MaybeLocalBoxFuture;

        #[derive(Default)]
        pub struct GlobalTokioSpawner;

        impl Spawner for GlobalTokioSpawner {
            fn spawn_detached_dyn(&mut self, fut: MaybeLocalBoxFuture<'static, ()>) {
                tokio::spawn(fut);
            }
        }
    }

    #[cfg(feature = "frb-compat")]
    pub mod frb {
        use crate::host::spawner::Spawner;
        use crate::maybe::MaybeLocalBoxFuture;

        #[derive(Default)]
        pub struct GlobalFrbSpawner;

        impl Spawner for GlobalFrbSpawner {
            fn spawn_detached_dyn(&mut self, fut: MaybeLocalBoxFuture<'static, ()>) {
                flutter_rust_bridge::spawn(fut);
            }
        }
    }
}

#[cfg(all(feature = "thread-safe", feature = "tokio"))]
pub use global::tokio::GlobalTokioSpawner;

#[cfg(all(feature = "thread-safe", feature = "frb-compat"))]
pub use global::frb::GlobalFrbSpawner;

use crate::maybe::{boxed_future, MaybeLocalBoxFuture, MaybeSend};

pub trait Spawner: MaybeSend {
    fn spawn_detached_dyn(&mut self, fut: MaybeLocalBoxFuture<'static, ()>);
}

pub trait SpawnerExt: Spawner {
    fn spawn_detached(&mut self, fut: impl Future<Output = ()> + MaybeSend + 'static) {
        self.spawn_detached_dyn(boxed_future(fut))
    }
}

impl<T: Spawner + ?Sized> SpawnerExt for T {}
