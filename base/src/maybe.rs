#[cfg(not(feature = "thread-safe"))]
mod primitives {
    pub trait MaybeSend {}
    impl<T> MaybeSend for T {}

    pub trait MaybeSync {}
    impl<T> MaybeSync for T {}

    pub trait MaybeStatic {}
    impl<T> MaybeStatic for T {}
    
    pub type MaybeArc<T> = alloc::rc::Rc<T>;
}

#[cfg(feature = "thread-safe")]
mod primitives {
    pub trait MaybeSend: Send {}
    impl<T: Send> MaybeSend for T {}

    pub trait MaybeSync: Sync {}
    impl<T: Sync> MaybeSync for T {}

    pub trait MaybeStatic: 'static {}
    impl<T: 'static> MaybeStatic for T {}
    
    pub type MaybeArc<T> = alloc::sync::Arc<T>;
}

pub(crate) use primitives::*;

pub trait MaybeSendSync: MaybeSend + MaybeSync {}
impl<T: MaybeSend + MaybeSync> MaybeSendSync for T {}

pub trait MaybeSendStatic: MaybeSend + MaybeStatic {}
impl<T: MaybeSend + MaybeStatic> MaybeSendStatic for T {}

pub trait MaybeSendSyncStatic: MaybeSend + MaybeSync + MaybeStatic {}
impl<T: MaybeSend + MaybeSync + MaybeStatic> MaybeSendSyncStatic for T {}