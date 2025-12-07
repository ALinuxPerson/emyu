use crate::maybe::{MaybeSend, MaybeSendStatic, MaybeSendSync};
use crate::runtime::{CommandContext, UpdateContext};
use crate::sync::VMutex;
use crate::{
    __private, VRWLockReadGuard, VRWLockWriteGuard, VRwLock, lock_mutex, read_vrwlock,
    write_vrwlock,
};
use alloc::sync::Arc;
use async_trait::async_trait;
use core::fmt::Debug;
use core::marker::PhantomData;
use futures::channel::mpsc;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

// must be `'static` for interceptors
pub trait Application: 'static {
    type RootModel: Model<ForApp = Self>;
}

pub struct AdHocApp<RootModel>(PhantomData<RootModel>);

impl<RootModel> Application for AdHocApp<RootModel>
where
    RootModel: Model<ForApp = AdHocApp<RootModel>>,
{
    type RootModel = RootModel;
}

pub trait ModelGetterMessage: MaybeSendStatic {
    type Data: MaybeSendStatic;
}

pub trait Model: MaybeSendSync + 'static {
    type ForApp: Application;
    type Message: MaybeSend;

    fn update(&mut self, message: Self::Message, ctx: &mut UpdateContext<Self::ForApp>);

    #[doc(hidden)]
    fn __accumulate_signals(
        &self,
        signals: &mut VecDeque<Arc<dyn FlushSignals>>,
        _token: __private::Token,
    );
}

pub trait ModelGetterHandler<M: ModelGetterMessage>: Model {
    fn getter(&self, message: M) -> M::Data;
}

#[async_trait]
pub trait Command: Debug + MaybeSendSync {
    type ForApp: Application;

    async fn apply(&mut self, ctx: &mut CommandContext<'_, Self::ForApp>);
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("the channel to the mvu runtime is closed")]
    MvuRuntimeChannelClosed,

    #[error("the channel to the model getter is closed")]
    ModelGetterChannelClosed,
}

impl From<MvuRuntimeChannelClosedError> for Error {
    fn from(_: MvuRuntimeChannelClosedError) -> Self {
        Self::MvuRuntimeChannelClosed
    }
}

impl From<ModelGetterChannelClosedError> for Error {
    fn from(_: ModelGetterChannelClosedError) -> Self {
        Self::ModelGetterChannelClosed
    }
}

#[derive(Error, Debug)]
#[error("the channel to the mvu runtime is closed")]
#[non_exhaustive]
pub struct MvuRuntimeChannelClosedError;

#[derive(Error, Debug)]
#[non_exhaustive]
#[error("the channel to the model getter is closed")]
pub struct ModelGetterChannelClosedError;

pub struct ModelBase<M>(Arc<VRwLock<M>>);

impl<M> ModelBase<M> {
    pub fn new(model: M) -> Self {
        Self(Arc::new(VRwLock::new(model)))
    }

    pub fn read(&self) -> VRWLockReadGuard<'_, M> {
        read_vrwlock(&self.0)
    }

    pub fn reader(&self) -> ModelBaseReader<M> {
        ModelBaseReader(self.clone())
    }

    pub fn write(&self) -> VRWLockWriteGuard<'_, M> {
        write_vrwlock(&self.0)
    }
}

pub type Lens<MParent, MChild> = fn(&<MChild as Model>::Message) -> <MParent as Model>::Message;

#[macro_export]
macro_rules! lens {
    ($parent:ty => $child:ident) => {
        |parent: $parent| &parent.$child
    };
}

impl<M: Model> ModelBase<M> {
    pub fn update(&self, message: M::Message, ctx: &mut UpdateContext<M::ForApp>) {
        self.write().update(message, ctx)
    }

    pub fn get<Msg>(&self, message: Msg) -> Msg::Data
    where
        Msg: ModelGetterMessage,
        M: ModelGetterHandler<Msg>,
    {
        self.read().getter(message)
    }

    pub fn zoom<Child>(&self, lens: fn(&M) -> &ModelBase<Child>) -> ModelBase<Child>
    where
        Child: Model<ForApp = M::ForApp>,
    {
        lens(&*self.read()).clone()
    }

    #[doc(hidden)]
    pub fn __accumulate_signals(
        &self,
        signals: &mut VecDeque<Arc<dyn FlushSignals>>,
        token: __private::Token,
    ) {
        self.read().__accumulate_signals(signals, token);
    }
}

impl<M> Clone for ModelBase<M> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub struct ModelBaseReader<M>(ModelBase<M>);

impl<M> ModelBaseReader<M> {
    pub fn read(&self) -> VRWLockReadGuard<'_, M> {
        self.0.read()
    }
}

impl<M: Model> ModelBaseReader<M> {
    pub fn get<Msg>(&self, message: Msg) -> Msg::Data
    where
        Msg: ModelGetterMessage,
        M: ModelGetterHandler<Msg>,
    {
        self.0.get(message)
    }

    pub fn zoom<Child>(&self, lens: fn(&M) -> &ModelBase<Child>) -> ModelBaseReader<Child>
    where
        Child: Model<ForApp = M::ForApp>,
    {
        ModelBaseReader(self.0.zoom(lens))
    }
}

impl<M> Clone for ModelBaseReader<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub trait Interceptor<A: Application>: MaybeSendSync + 'static {
    fn intercept(
        &mut self,
        model: ModelBaseReader<A::RootModel>,
        message: &<A::RootModel as Model>::Message,
    );
}

impl<A, F> Interceptor<A> for F
where
    A: Application,
    F: FnMut(ModelBaseReader<A::RootModel>, &<A::RootModel as Model>::Message) + MaybeSendSync + 'static,
{
    fn intercept(
        &mut self,
        model: ModelBaseReader<A::RootModel>,
        message: &<A::RootModel as Model>::Message,
    ) {
        self(model, message)
    }
}

pub struct Signal<T>(Arc<SignalRepr<T>>);

impl<T> Signal<T> {
    pub fn subscribe(&self) -> mpsc::Receiver<()> {
        let (tx, rx) = mpsc::channel(1);
        let mut subscribers = lock_mutex(&self.0.subscribers);
        subscribers.push(tx);
        rx
    }

    pub fn read(&self) -> VRWLockReadGuard<'_, T> {
        read_vrwlock(&self.0.state)
    }

    fn write(&mut self) -> VRWLockWriteGuard<'_, T> {
        write_vrwlock(&self.0.state)
    }

    pub fn update<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        let ret = { f(&mut *self.write()) };
        self.0.dirty.store(true, Ordering::Release);
        ret
    }

    pub fn set(&mut self, value: T) {
        self.update(|v| *v = value);
    }

    #[doc(hidden)]
    pub fn __to_dyn_flush_signals(&self, _: __private::Token) -> Arc<dyn FlushSignals>
    where
        T: MaybeSendSync + 'static,
    {
        Arc::clone(&self.0) as _
    }
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

struct SignalRepr<T> {
    state: Arc<VRwLock<T>>,
    subscribers: VMutex<Vec<mpsc::Sender<()>>>,
    dirty: AtomicBool,
}

#[doc(hidden)]
pub trait FlushSignals: MaybeSend {
    fn __flush(&mut self, _token: __private::Token);
}

impl<T: MaybeSendSync> FlushSignals for SignalRepr<T> {
    fn __flush(&mut self, _: __private::Token) {
        if self.dirty.swap(false, Ordering::AcqRel) {
            let mut subscribers = lock_mutex(&self.subscribers);
            for subscriber in &mut *subscribers {
                subscriber.try_send(()).ok();
            }
            subscribers.retain(|s| !s.is_closed());
        }
    }
}
