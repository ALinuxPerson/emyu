#[cfg(feature = "thread-safe")]
mod impls {
    pub trait MaybeSend: Send {}
    pub trait MaybeSync: Sync {}
    pub trait MaybeStatic: 'static {}

    impl<T: Send> MaybeSend for T {}
    impl<T: Sync> MaybeSync for T {}
    impl<T: 'static> MaybeStatic for T {}
    pub type Shared<T> = alloc::sync::Arc<T>;
}

#[cfg(not(feature = "thread-safe"))]
mod impls {
    pub trait MaybeSend {}
    pub trait MaybeSync {}
    pub trait MaybeStatic {}

    impl<T> MaybeSend for T {}
    impl<T> MaybeSync for T {}
    impl<T> MaybeStatic for T {}
    pub type Shared<T> = alloc::rc::Rc<T>;
}

pub use impls::{MaybeSend, MaybeStatic, MaybeSync, Shared};

#[cfg(feature = "thread-safe")]
mod sync {
    use core::ops::{Deref, DerefMut};

    #[cfg(not(feature = "std"))]
    use spin::{
        Mutex as MutexImpl, MutexGuard as MutexGuardImpl, RwLock as RwLockImpl,
        RwLockReadGuard as RwLockReadGuardImpl, RwLockWriteGuard as RwLockWriteGuardImpl,
    };
    #[cfg(feature = "std")]
    use std::sync::{
        Mutex as MutexImpl, MutexGuard as MutexGuardImpl, RwLock as RwLockImpl,
        RwLockReadGuard as RwLockReadGuardImpl, RwLockWriteGuard as RwLockWriteGuardImpl,
    };

    macro_rules! unwrap_lock {
        ($e:expr) => {{
            #[cfg(feature = "std")]
            {
                $e.unwrap()
            }

            #[cfg(not(feature = "std"))]
            {
                $e
            }
        }};
    }

    // --- MaybeRwLock ---
    pub struct MaybeRwLock<T>(RwLockImpl<T>);
    pub struct MaybeRwLockReadGuard<'a, T>(RwLockReadGuardImpl<'a, T>);
    pub struct MaybeRwLockWriteGuard<'a, T>(RwLockWriteGuardImpl<'a, T>);

    impl<T> MaybeRwLock<T> {
        pub fn new(value: T) -> Self {
            Self(RwLockImpl::new(value))
        }
        pub fn read(&self) -> MaybeRwLockReadGuard<'_, T> {
            MaybeRwLockReadGuard(unwrap_lock!(self.0.read()))
        }
        pub fn write(&self) -> MaybeRwLockWriteGuard<'_, T> {
            MaybeRwLockWriteGuard(unwrap_lock!(self.0.write()))
        }
    }

    pub struct MaybeMutex<T>(MutexImpl<T>);
    pub struct MaybeMutexGuard<'a, T>(MutexGuardImpl<'a, T>);

    impl<T> MaybeMutex<T> {
        pub fn new(value: T) -> Self {
            Self(MutexImpl::new(value))
        }
        pub fn lock(&self) -> MaybeMutexGuard<'_, T> {
            MaybeMutexGuard(unwrap_lock!(self.0.lock()))
        }
    }

    impl<'a, T> Deref for MaybeRwLockReadGuard<'a, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<'a, T> Deref for MaybeRwLockWriteGuard<'a, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<'a, T> DerefMut for MaybeRwLockWriteGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl<'a, T> Deref for MaybeMutexGuard<'a, T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl<'a, T> DerefMut for MaybeMutexGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
}

#[cfg(not(feature = "thread-safe"))]
mod sync {
    use core::cell::{Ref, RefCell, RefMut};

    pub struct MaybeRwLock<T>(RefCell<T>);
    pub type MaybeRwLockReadGuard<'a, T> = Ref<'a, T>;
    pub type MaybeRwLockWriteGuard<'a, T> = RefMut<'a, T>;

    impl<T> MaybeRwLock<T> {
        pub fn new(value: T) -> Self {
            Self(RefCell::new(value))
        }
        pub fn read(&self) -> MaybeRwLockReadGuard<'_, T> {
            self.0.borrow()
        }
        pub fn write(&self) -> MaybeRwLockWriteGuard<'_, T> {
            self.0.borrow_mut()
        }
    }

    pub struct MaybeMutex<T>(RefCell<T>);
    pub type MaybeMutexGuard<'a, T> = RefMut<'a, T>;

    impl<T> MaybeMutex<T> {
        pub fn new(value: T) -> Self {
            Self(RefCell::new(value))
        }
        pub fn lock(&self) -> MaybeMutexGuard<'_, T> {
            self.0.borrow_mut()
        }
    }
}

pub use sync::{
    MaybeMutex, MaybeMutexGuard, MaybeRwLock, MaybeRwLockReadGuard, MaybeRwLockWriteGuard,
};

pub trait MaybeSendSync: MaybeSend + MaybeSync + MaybeStatic {}
impl<T: MaybeSend + MaybeSync + MaybeStatic> MaybeSendSync for T {}

pub trait MaybeSendStatic: MaybeSend + MaybeStatic {}
impl<T: MaybeSend + MaybeStatic> MaybeSendStatic for T {}
