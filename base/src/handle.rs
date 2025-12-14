use core::marker::PhantomData;
use crate::{Application, GlobalFrbSpawner, GlobalTokioSpawner, Host, HostBuilder, Spawner, SpawnerExt};
use crate::{WrappedGetter, WrappedUpdater};

pub struct AppHandle<A: Application, WU, WG> {
    updater: WU,
    getter: WG,
    _app: PhantomData<A>,
}

impl<A, WU, WG> AppHandle<A, WU, WG>
where
    A: Application,
    WU: WrappedUpdater<Model = A::RootModel>,
    WG: WrappedGetter<Model = A::RootModel>,
{
    pub fn new<S: Spawner + Default>(builder_fn: impl FnOnce(HostBuilder<A>) -> Host<A>) -> Self {
        let host = builder_fn(HostBuilder::new());
        let updater = host.updater();
        let getter = host.getter();
        S::default().spawn_detached(host.run());
        Self {
            updater: WU::__new(updater, crate::__token()),
            getter: WG::__new(getter, crate::__token()),
            _app: PhantomData,
        }
    }

    #[cfg(feature = "frb-compat")]
    pub fn new_frb(builder_fn: impl FnOnce(HostBuilder<A>) -> Host<A>) -> Self {
        Self::new::<GlobalFrbSpawner>(builder_fn)
    }

    #[cfg(feature = "tokio")]
    pub fn new_tokio(
        builder_fn: impl FnOnce(HostBuilder<A>) -> Host<A>,
    ) -> Self {
        Self::new::<GlobalTokioSpawner>(builder_fn)
    }
}

impl<A, WU, WG> AppHandle<A, WU, WG>
where
    A: Application,
    WU: WrappedUpdater<Model = A::RootModel>,
    WG: WrappedGetter<Model = A::RootModel>,
{
    pub fn updater(&self) -> WU {
        self.updater.clone()
    }

    pub fn getter(&self) -> WG {
        self.getter.clone()
    }
}
