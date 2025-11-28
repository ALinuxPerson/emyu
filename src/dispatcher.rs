use crate::base::Model;
use async_trait::async_trait;
use dyn_clone::DynClone;
use futures::SinkExt;
use futures::channel::mpsc;
use std::marker::PhantomData;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("the channel to the mvu runtime is closed")]
#[non_exhaustive]
pub struct MvuRuntimeChannelClosedError;

#[async_trait]
pub trait Dispatcher<Msg>: Send + DynClone {
    async fn try_dispatch(&mut self, message: Msg) -> Result<(), MvuRuntimeChannelClosedError>;
}

dyn_clone::clone_trait_object!(<Msg> Dispatcher<Msg>);

#[async_trait]
pub trait DispatcherExt<Msg: Send>: Dispatcher<Msg> {
    async fn dispatch(&mut self, message: Msg)
    where
        Msg: 'async_trait,
    {
        self.try_dispatch(message)
            .await
            .expect("mvu runtime channel closed")
    }
}

impl<D, Msg> DispatcherExt<Msg> for D
where
    D: Dispatcher<Msg>,
    Msg: Send,
{
}

pub struct MessageDispatcher<M: Model>(pub(crate) mpsc::Sender<M::Message>);

impl<M: Model> MessageDispatcher<M> {
    pub fn map<MChild, F>(self, f: F) -> MappedDispatcher<MChild, M, F>
    where
        MChild: Model,
        F: FnMut(MChild::Message) -> M::Message,
    {
        MappedDispatcher {
            dispatcher: Box::new(self),
            f,
            _child: PhantomData,
        }
    }
}

#[async_trait]
impl<M: Model> Dispatcher<M::Message> for MessageDispatcher<M> {
    async fn try_dispatch(
        &mut self,
        message: M::Message,
    ) -> Result<(), MvuRuntimeChannelClosedError> {
        self.0
            .send(message)
            .await
            .map_err(|_| MvuRuntimeChannelClosedError)
    }
}

impl<M: Model> Clone for MessageDispatcher<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub struct MappedDispatcher<MChild, MParent: Model, F> {
    dispatcher: Box<dyn Dispatcher<MParent::Message>>,
    f: F,
    _child: PhantomData<MChild>,
}

impl<MChild, MParent, F> MappedDispatcher<MChild, MParent, F>
where
    MChild: Model,
    MParent: Model,
    F: FnMut(MChild::Message) -> MParent::Message + Send + Clone + 'static,
{
    pub fn map<MSubChild, SubF>(self, f: SubF) -> MappedDispatcher<MSubChild, MChild, SubF>
    where
        MSubChild: Model,
        SubF: FnMut(MSubChild::Message) -> MChild::Message,
    {
        MappedDispatcher {
            dispatcher: Box::new(self),
            f,
            _child: PhantomData,
        }
    }
}

impl<MChild, MParent: Model, F: Clone> Clone for MappedDispatcher<MChild, MParent, F> {
    fn clone(&self) -> Self {
        Self {
            dispatcher: self.dispatcher.clone(),
            f: self.f.clone(),
            _child: PhantomData,
        }
    }
}

#[async_trait]
impl<MChild, MParent, F> Dispatcher<MChild::Message> for MappedDispatcher<MChild, MParent, F>
where
    MChild: Model,
    MParent: Model,
    F: FnMut(MChild::Message) -> MParent::Message + Send + Clone,
{
    async fn try_dispatch(
        &mut self,
        message: MChild::Message,
    ) -> Result<(), MvuRuntimeChannelClosedError> {
        self.dispatcher.try_dispatch((self.f)(message)).await
    }
}
