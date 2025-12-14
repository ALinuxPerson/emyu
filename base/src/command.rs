/*
 * Portions of this file are derived from `iced`.
 * URL: https://github.com/iced-rs/iced
 * Commit Hash: c532ad216b1ed9398775a1ad95234165aa5ee649
 *
 * Original Copyright (c) 2019 Héctor Ramón, Iced contributors
 * Licensed under the MIT License.
 */
//! Creates host commands.

use crate::maybe::{MaybeLocalBoxStream, MaybeSend, boxed_stream};
use crate::{Application, CommandContext};
use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::pin::Pin;
use core::{fmt, task};
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, Stream, StreamExt, future, stream};

type CommandRepr<T, ForApp> =
    Box<dyn_Maybe!(Send FnOnce(CommandContext<ForApp>) -> MaybeLocalBoxStream<'static, T>)>;

/// A set of concurrent actions to be performed by the host.
///
/// A [`Command`] _may_ produce a bunch of values of type `T`.
pub struct Command<T, ForApp: Application>(Option<CommandRepr<T, ForApp>>);

impl<T, ForApp: Application> Command<T, ForApp> {
    /// Creates a [`Command`] that does nothing.
    pub fn none() -> Self {
        Self(None)
    }

    fn some_dyn<F>(f: F) -> Self
    where
        F: FnOnce(CommandContext<ForApp>) -> MaybeLocalBoxStream<'static, T> + MaybeSend + 'static,
    {
        Self(Some(Box::new(f)))
    }

    fn some<F, S>(f: F) -> Self
    where
        F: FnOnce(CommandContext<ForApp>) -> S + MaybeSend + 'static,
        S: Stream<Item = T> + MaybeSend + 'static,
    {
        Self::some_dyn(move |ctx| boxed_stream(f(ctx)))
    }

    /// Creates a new [`Command`] that instantly produces the given value.
    pub fn done(value: T) -> Self
    where
        T: MaybeSend + 'static,
    {
        Self::future(|_| future::ready(value))
    }

    /// Creates a [`Command`] that runs the given [`Future`] to completion and maps its output with
    /// the given closure.
    pub fn perform<A, FFut, Fut, FMap>(fut_fn: FFut, f: FMap) -> Self
    where
        FFut: FnOnce(CommandContext<ForApp>) -> Fut + MaybeSend + 'static,
        Fut: Future<Output = A> + MaybeSend + 'static,
        FMap: FnOnce(A) -> T + MaybeSend + 'static,
    {
        Self::future(|ctx| fut_fn(ctx).map(f))
    }

    /// Creates a [`Command`] that runs the given [`Stream`] to completion and maps each item with
    /// the given closure.
    pub fn run<A, FStrm, Strm, FMap>(stream_fn: FStrm, f: FMap) -> Self
    where
        FStrm: FnOnce(CommandContext<ForApp>) -> Strm + MaybeSend + 'static,
        Strm: Stream<Item = A> + MaybeSend + 'static,
        FMap: FnMut(A) -> T + MaybeSend + 'static,
    {
        Self::stream(|ctx| stream_fn(ctx).map(f))
    }

    /// Combines the given tasks and produces a single [`Command`] that will run all of them in
    /// parallel.
    pub fn batch(commands: impl IntoIterator<Item = Self> + MaybeSend + 'static) -> Self
    where
        T: 'static,
    {
        let f = Box::new(move |ctx: CommandContext<ForApp>| {
            let mut select_all = stream::SelectAll::<MaybeLocalBoxStream<T>>::new();

            for command in commands.into_iter() {
                if let Some(repr) = command.0 {
                    select_all.push((repr)(ctx.clone()));
                }
            }

            boxed_stream(select_all)
        });

        Self(Some(f))
    }

    /// Maps the output of a [`Command`] with the given closure.
    pub fn map<O>(self, mut f: impl FnMut(T) -> O + MaybeSend + 'static) -> Command<O, ForApp>
    where
        T: 'static,
        O: MaybeSend + 'static,
    {
        self.then(move |output| Command::done(f(output)))
    }

    /// Performs a new [`Command`] for every output of the current [`Command`] using the given closure.
    ///
    /// This is the monadic interface of [`crate::command2::Command`]—analogous to [`Future`] and
    /// [`Stream`].
    pub fn then<O>(
        self,
        mut f: impl FnMut(T) -> Command<O, ForApp> + MaybeSend + 'static,
    ) -> Command<O, ForApp>
    where
        T: 'static,
        O: MaybeSend + 'static,
    {
        Command(self.0.map(|stream_fn| {
            Box::new(|ctx: CommandContext<ForApp>| {
                boxed_stream(stream_fn(ctx.clone()).flat_map(move |output| {
                    f(output)
                        .0
                        .map(|f| boxed_stream(f(ctx.clone())))
                        .unwrap_or_else(|| boxed_stream(stream::empty()))
                }))
            })
        } as CommandRepr<O, ForApp>))
    }

    /// Chains a new [`Command`] to be performed once the current one finishes completely.
    pub fn chain(self, command: Self) -> Self
    where
        T: 'static,
    {
        match self.0 {
            None => command,
            Some(first_fn) => match command.0 {
                None => Self(Some(first_fn)),
                Some(second_fn) => Self::some(|ctx| first_fn(ctx.clone()).chain(second_fn(ctx))),
            },
        }
    }

    /// Creates a new [`Command`] that collects all the output of the current one into a [`Vec`].
    pub fn collect(self) -> Command<Vec<T>, ForApp>
    where
        T: MaybeSend + 'static,
    {
        match self.0 {
            None => Command::done(Vec::new()),
            Some(stream_fn) => Command::stream(|ctx| {
                stream::unfold(
                    (stream_fn(ctx.clone()), Some(Vec::new())),
                    |(mut stream, outputs)| async move {
                        let mut outputs = outputs?;
                        let Some(output) = stream.next().await else {
                            return Some((Some(outputs), (stream, None)));
                        };
                        outputs.push(output);
                        Some((None, (stream, Some(outputs))))
                    },
                )
                .filter_map(future::ready)
            }),
        }
    }

    /// Creates a new [`Command`] that discards the result of the current one.
    ///
    /// Useful if you only care about the side effects of a [`Command`].
    pub fn discard<O>(self) -> Command<O, ForApp>
    where
        T: 'static,
        O: MaybeSend + 'static,
    {
        self.then(|_| Command::none())
    }

    /// Creates a new [`Command`] that runs the given [`Future`] and produces its output.
    pub fn future<F, Fut>(f: F) -> Self
    where
        F: FnOnce(CommandContext<ForApp>) -> Fut + MaybeSend + 'static,
        Fut: Future<Output = T> + MaybeSend + 'static,
    {
        Self::stream(|ctx| stream::once(f(ctx)))
    }

    /// Creates a new [`Command`] that runs the given [`Stream`] and produces each of its items.
    pub fn stream<F, S>(f: F) -> Self
    where
        F: FnOnce(CommandContext<ForApp>) -> S + MaybeSend + 'static,
        S: Stream<Item = T> + MaybeSend + 'static,
    {
        Self(Some(Box::new(move |ctx| {
            boxed_stream(
                stream::once(yield_now())
                    .filter_map(|_| async { None })
                    .chain(f(ctx)),
            )
        })))
    }
}

impl<T, ForApp: Application> fmt::Debug for Command<T, ForApp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(&format!(
            "Command<{}, {}>",
            core::any::type_name::<T>(),
            core::any::type_name::<ForApp>()
        ))
        .finish_non_exhaustive()
    }
}

impl<T, ForApp: Application> Command<Option<T>, ForApp> {
    /// Executes a new [`Command`] after this one, only when it produces `Some` value.
    ///
    /// The value is provided to the closure to create the subsequent [`Command`].
    pub fn and_then<A>(
        self,
        f: impl Fn(T) -> Command<A, ForApp> + MaybeSend + 'static,
    ) -> Command<A, ForApp>
    where
        T: 'static,
        A: MaybeSend + 'static,
    {
        self.then(move |option| option.map_or_else(Command::none, &f))
    }
}

impl<T, E, ForApp: Application> Command<Result<T, E>, ForApp> {
    /// Executes a new [`Command`] after this one, only when it succeeds with an `Ok` value.
    ///
    /// The success value is provided to the closure to create the subsequent [`Command`].
    pub fn and_then<A>(
        self,
        f: impl Fn(T) -> Command<Result<A, E>, ForApp> + MaybeSend + 'static,
    ) -> Command<Result<A, E>, ForApp>
    where
        T: 'static,
        E: MaybeSend + 'static,
        A: MaybeSend + 'static,
    {
        self.then(move |result| result.map_or_else(|error| Command::done(Err(error)), &f))
    }

    /// Maps the error type of this [`Command`] to a different one using the given function.
    pub fn map_err<E2>(
        self,
        f: impl Fn(E) -> E2 + MaybeSend + 'static,
    ) -> Command<Result<T, E2>, ForApp>
    where
        T: MaybeSend + 'static,
        E: MaybeSend + 'static,
        E2: MaybeSend + 'static,
    {
        self.map(move |result| result.map_err(&f))
    }
}

impl<T, ForApp: Application> Default for Command<T, ForApp> {
    fn default() -> Self {
        Self::none()
    }
}

impl<T, ForApp> From<()> for Command<T, ForApp>
where
    ForApp: Application,
{
    fn from((): ()) -> Self {
        Self::none()
    }
}

/// Creates a new [`Command`] that executes the function returned by the closure
/// and produces the value fed to the [`oneshot::Sender`].
pub fn oneshot<T, ForApp>(
    f: impl FnOnce(oneshot::Sender<T>, CommandContext<ForApp>) -> T + MaybeSend + 'static,
) -> Command<T, ForApp>
where
    T: MaybeSend + 'static,
    ForApp: Application,
{
    let (sender, receiver) = oneshot::channel();

    Command::some(move |ctx| {
        let action = f(sender, ctx);
        stream::once(async move { action }).chain(
            receiver
                .into_stream()
                .filter_map(|result| async { result.ok() }),
        )
    })
}

/// Creates a new [`Command`] that executes the function returned by the closure and produces the
/// values fed to the [`mpsc::Sender`].
pub fn channel<T, ForApp>(
    f: impl FnOnce(mpsc::Sender<T>, CommandContext<ForApp>) -> T + MaybeSend + 'static,
) -> Command<T, ForApp>
where
    T: MaybeSend + 'static,
    ForApp: Application,
{
    let (sender, receiver) = mpsc::channel(1);

    Command::stream(move |ctx| {
        let action = f(sender, ctx);
        stream::once(async move { action }).chain(receiver)
    })
}

pub fn into_repr<T, ForApp: Application>(
    command: Command<T, ForApp>,
) -> Option<CommandRepr<T, ForApp>> {
    command.0
}

async fn yield_now() {
    struct YieldNow {
        yielded: bool,
    }

    impl Future for YieldNow {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<()> {
            if self.yielded {
                return task::Poll::Ready(());
            }

            self.yielded = true;

            cx.waker().wake_by_ref();

            task::Poll::Pending
        }
    }

    YieldNow { yielded: false }.await;
}
