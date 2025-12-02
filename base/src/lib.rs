#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

#[macro_use]
pub mod macros;

#[doc(hidden)]
pub mod __macros {
    #[cfg(feature = "frb-compat")]
    pub extern crate flutter_rust_bridge;

    #[cfg(feature = "frb-compat")]
    pub extern crate anyhow;

    pub extern crate futures;

    pub use async_trait::async_trait;

    #[cfg(feature = "frb-compat")]
    pub use flutter_rust_bridge::frb;
}

pub mod base;
pub mod dispatcher;
pub mod runtime;
pub mod handle;

pub use base::*;
pub use dispatcher::*;
pub use runtime::*;
pub use handle::*;

#[cfg(feature = "std")]
type VRwLock<T> = std::sync::RwLock<T>;

#[cfg(feature = "std")]
type VRWLockReadGuard<'a, T> = std::sync::RwLockReadGuard<'a, T>;

#[cfg(feature = "std")]
type VRWLockWriteGuard<'a, T> = std::sync::RwLockWriteGuard<'a, T>;

#[cfg(not(feature = "std"))]
type VRwLock<T> = spin::RwLock<T>;

#[cfg(not(feature = "std"))]
type VRWLockReadGuard<'a, T> = spin::RwLockReadGuard<'a, T>;

#[cfg(not(feature = "std"))]
type VRWLockWriteGuard<'a, T> = spin::RwLockWriteGuard<'a, T>;
