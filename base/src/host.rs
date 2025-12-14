mod spawner;
mod world;

use crate::maybe::{MaybeRwLockReadGuard, MaybeSendSync, Shared};
use crate::{Application, Model, ModelGetterHandler, ModelGetterMessage, command};
use crate::{FlushSignals, Interceptor, ModelBase, ModelBaseReader, Signal};
use crate::{Getter, Updater};
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::ops::ControlFlow;
use futures::StreamExt;
use futures::channel::mpsc;
pub use spawner::*;
use world::WorldRepr;
pub use world::{State, StateMut, StateRef, World};

const DEFAULT_CHANNEL_BUFFER_SIZE: usize = 64;

pub struct CommandContext<A: Application> {
    pub model: ModelBaseReader<A::RootModel>,
    pub world: World,
    pub updater: Updater<A::RootModel>,
}

impl<A: Application> CommandContext<A> {
    pub fn read(&self) -> MaybeRwLockReadGuard<'_, A::RootModel> {
        self.model.read()
    }

    pub fn get<Msg>(&self) -> Signal<Msg::Data>
    where
        Msg: ModelGetterMessage,
        A::RootModel: ModelGetterHandler<Msg>,
    {
        self.model.get()
    }

    pub fn state<S: MaybeSendSync + 'static, R>(&self, f: impl FnOnce(&S) -> R) -> R {
        self.world.get(f)
    }

    pub fn state_mut<S: MaybeSendSync + 'static, R>(&self, f: impl FnOnce(&mut S) -> R) -> R {
        self.world.get_mut(f)
    }

    pub async fn send_message(&mut self, message: <A::RootModel as Model>::Message) {
        self.updater.send(message).await
    }
}

impl<A: Application> Clone for CommandContext<A> {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            world: self.world.clone(),
            updater: self.updater.clone(),
        }
    }
}

type RootMessage<A> = <<A as Application>::RootModel as Model>::Message;

pub struct Host<A: Application> {
    model: ModelBase<A::RootModel>,
    world: World,
    interceptors: Vec<Box<dyn Interceptor<A>>>,
    spawner: Box<dyn Spawner>,
    signals: VecDeque<Shared<dyn FlushSignals>>,
    updater: Updater<A::RootModel>,
    message_rx: mpsc::Receiver<RootMessage<A>>,
}

impl<A: Application> Host<A> {
    pub fn builder() -> HostBuilder<A> {
        HostBuilder::new()
    }

    pub fn new(model: A::RootModel) -> Self {
        Self::builder().model(model).build()
    }

    pub fn defaults() -> Self
    where
        A::RootModel: Default,
    {
        HostBuilder::defaults().build()
    }
}

impl<A: Application> Host<A> {
    pub async fn run(mut self) {
        tracing::debug!("host has started");
        loop {
            if let ControlFlow::Break(()) = self.run_once().await {
                tracing::debug!("host is stopping");
                break;
            }
        }
    }

    async fn run_once(&mut self) -> ControlFlow<()> {
        match self.message_rx.next().await {
            Some(action) => self.handle_message(action).await,
            None => return ControlFlow::Break(()),
        };

        ControlFlow::Continue(())
    }

    async fn handle_message(&mut self, message: RootMessage<A>) {
        for interceptor in &mut self.interceptors {
            interceptor.intercept(self.model.reader(), &message);
        }
        let command = command::into_repr(self.model.write().update(message));
        self.model
            .__accumulate_signals(&mut self.signals, crate::__token());
        if let Some(command) = command {
            let ctx = CommandContext {
                model: self.model.reader(),
                world: self.world.clone(),
                updater: self.updater.clone(),
            };
            let mut updater = self.updater.clone();
            self.spawner.spawn_detached(async move {
                let mut stream = command(ctx);
                while let Some(message) = stream.next().await {
                    updater.send(message).await
                }
            });
        }
        while let Some(signal) = self.signals.pop_front() {
            signal.__flush(crate::__token());
        }
    }
}

impl<A: Application> Host<A> {
    pub fn updater(&self) -> Updater<A::RootModel> {
        self.updater.clone()
    }

    pub fn getter(&self) -> Getter<A::RootModel> {
        Getter::new(self.model.clone())
    }
}

pub struct HostBuilder<A: Application> {
    model: Option<A::RootModel>,
    world: WorldRepr,
    interceptors: Vec<Box<dyn Interceptor<A>>>,
    spawner: Option<Box<dyn Spawner>>,
    buffer_size: usize,
}

impl<A: Application> HostBuilder<A> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn defaults() -> Self
    where
        A::RootModel: Default,
    {
        Self::new().default_model()
    }

    pub fn model(self, value: A::RootModel) -> Self {
        Self {
            model: Some(value),
            ..self
        }
    }

    pub fn state_with<S: MaybeSendSync + 'static>(self, value: S) -> Self {
        Self {
            world: self.world.add_with(value),
            ..self
        }
    }

    pub fn state<S: Default + MaybeSendSync + 'static>(self) -> Self {
        Self {
            world: self.world.add::<S>(),
            ..self
        }
    }

    pub fn interceptor(mut self, value: impl Interceptor<A>) -> Self {
        self.interceptors.push(Box::new(value));
        self
    }

    pub fn buffer_size(self, value: usize) -> Self {
        Self {
            buffer_size: value,
            ..self
        }
    }

    pub fn default_model(self) -> Self
    where
        A::RootModel: Default,
    {
        self.model(Default::default())
    }

    pub fn build(self) -> Host<A> {
        let model = self.model.expect("RootModel was not initialized");
        let model = ModelBase::new(model);

        let (message_tx, message_rx) = mpsc::channel(self.buffer_size);

        Host {
            model: model.clone(),
            world: self.world.into(),
            interceptors: self.interceptors,
            spawner: self.spawner.expect("spawner was not initialized"),
            signals: VecDeque::new(),
            updater: Updater::new(message_tx),
            message_rx,
        }
    }
}

impl<A: Application> Default for HostBuilder<A> {
    fn default() -> Self {
        Self {
            model: None,
            world: WorldRepr::default(),
            interceptors: Vec::new(),
            spawner: None,
            buffer_size: DEFAULT_CHANNEL_BUFFER_SIZE,
        }
    }
}
