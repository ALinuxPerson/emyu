use crate::maybe::{
    MaybeRwLock, MaybeRwLockReadGuard, MaybeRwLockWriteGuard, MaybeSendSync, Shared,
};
use core::any::type_name;
use core::ops::{Deref, DerefMut};

#[derive(Default)]
pub struct WorldRepr(
    #[cfg(feature = "thread-safe")] type_map::concurrent::TypeMap,
    #[cfg(not(feature = "thread-safe"))] type_map::TypeMap,
);

impl WorldRepr {
    pub(crate) fn add_with<S: MaybeSendSync + 'static>(mut self, state: S) -> Self {
        self.0.insert(Shared::new(MaybeRwLock::new(state)));
        self
    }

    pub(crate) fn add<S: Default + MaybeSendSync + 'static>(self) -> Self {
        self.add_with(S::default())
    }

    pub fn try_state<S: MaybeSendSync + 'static>(&self) -> Option<State<S>> {
        self.0.get::<Shared<MaybeRwLock<S>>>().cloned().map(State)
    }

    pub fn state<S: MaybeSendSync + 'static>(&self) -> State<S> {
        self.try_state()
            .unwrap_or_else(|| panic!("`{}` does not exist in the world", type_name::<S>()))
    }
}

#[derive(Clone)]
pub struct World(Shared<WorldRepr>);

impl From<WorldRepr> for World {
    fn from(value: WorldRepr) -> Self {
        Self(Shared::new(value))
    }
}

impl World {
    pub fn try_state<S: MaybeSendSync + 'static>(&self) -> Option<State<S>> {
        self.0.try_state::<S>()
    }

    pub fn state<S: MaybeSendSync + 'static>(&self) -> State<S> {
        self.0.state::<S>()
    }

    pub fn try_get<S: MaybeSendSync + 'static, R>(&self, f: impl FnOnce(Option<&S>) -> R) -> R {
        if let Some(state) = self.try_state::<S>() {
            f(Some(&state.read()))
        } else {
            f(None)
        }
    }

    pub fn get<S: MaybeSendSync + 'static, R>(&self, f: impl FnOnce(&S) -> R) -> R {
        f(&self.state::<S>().read())
    }

    pub fn try_get_mut<S: MaybeSendSync + 'static, R>(&self, f: impl FnOnce(Option<&mut S>) -> R) -> R {
        if let Some(state) = self.try_state::<S>() {
            f(Some(&mut state.write()))
        } else {
            f(None)
        }
    }

    pub fn get_mut<S: MaybeSendSync + 'static, R>(&self, f: impl FnOnce(&mut S) -> R) -> R {
        f(&mut self.state::<S>().write())
    }

    pub async fn try_get_async<S, Fut>(&self, f: impl FnOnce(Option<&S>) -> Fut) -> Fut::Output
    where
        S: MaybeSendSync + 'static,
        Fut: Future,
    {
        if let Some(state) = self.try_state::<S>() {
            f(Some(&state.read())).await
        } else {
            f(None).await
        }
    }

    pub async fn get_async<S, Fut>(&self, f: impl FnOnce(&S) -> Fut) -> Fut::Output
    where
        S: MaybeSendSync + 'static,
        Fut: Future,
    {
        f(&self.state::<S>().read()).await
    }

    pub async fn try_get_mut_async<S, Fut>(&self, f: impl FnOnce(Option<&mut S>) -> Fut) -> Fut::Output
    where
        S: MaybeSendSync + 'static,
        Fut: Future,
    {
        if let Some(state) = self.try_state::<S>() {
            f(Some(&mut state.write())).await
        } else {
            f(None).await
        }
    }

    pub async fn get_mut_async<S, Fut>(&self, f: impl FnOnce(&mut S) -> Fut) -> Fut::Output
    where
        S: MaybeSendSync + 'static,
        Fut: Future,
    {
        f(&mut self.state::<S>().write()).await
    }
}

#[derive(Clone)]
pub struct State<T>(Shared<MaybeRwLock<T>>);

impl<T> State<T> {
    pub fn read(&self) -> StateRef<'_, T> {
        StateRef(self.0.read())
    }

    pub fn write(&self) -> StateMut<'_, T> {
        StateMut(self.0.write())
    }
}

pub struct StateRef<'s, T>(MaybeRwLockReadGuard<'s, T>);

impl<'s, T> Deref for StateRef<'s, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct StateMut<'s, T>(MaybeRwLockWriteGuard<'s, T>);

impl<'s, T> Deref for StateMut<'s, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'s, T> DerefMut for StateMut<'s, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
