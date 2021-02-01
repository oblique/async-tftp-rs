use async_channel::{Receiver, Sender};
use futures_util::future::{select, Either};
use futures_util::stream::{FuturesUnordered, StreamExt};
use pin_utils::pin_mut;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;

type BoxedFuture = Pin<Box<dyn Future<Output = TaskResult> + Send>>;

pub(crate) struct Executor {
    tx: Sender<BoxedFuture>,
    rx: Receiver<BoxedFuture>,
    ex: FuturesUnordered<BoxedFuture>,
}

enum TaskResult {
    Primary(Box<dyn Any>),
    Secondary,
}

pub(crate) struct Spawner {
    tx: Sender<BoxedFuture>,
}

impl Executor {
    pub(crate) fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();

        Executor {
            rx,
            tx,
            ex: FuturesUnordered::new(),
        }
    }

    pub(crate) async fn run<F, T>(&mut self, fut: F) -> T
    where
        T: 'static,
        F: Future<Output = T> + Send + 'static,
    {
        // This is the primary task, wrap its result
        let fut = async move {
            let res = fut.await;
            TaskResult::Primary(Box::new(res))
        };

        // Schedule primary task
        self.ex.push(Box::pin(fut));

        // Pool all tasks until primary task is finished
        loop {
            let ex_next = self.ex.next();
            let rx_recv = self.rx.recv();

            pin_mut!(ex_next);
            pin_mut!(rx_recv);

            match select(ex_next, rx_recv).await {
                // We got the result of primary task, downcast it and return it
                Either::Left((Some(TaskResult::Primary(res)), _)) => {
                    return *res.downcast::<T>().expect("Invalid downcast");
                }
                // We got result from a secondary task
                Either::Left((Some(TaskResult::Secondary), _)) => {}
                // Unreachable because we should recieve result from primary task first
                Either::Left((None, _)) => unreachable!(),
                // We got new future to schedule
                Either::Right((Ok(fut), _)) => self.ex.push(fut),
                // Unreachable because we use unbounded channel and there is at least
                // one sender alive
                Either::Right((Err(_), _)) => unreachable!(),
            }
        }
    }

    pub(crate) fn spawner(&self) -> Spawner {
        Spawner {
            tx: self.tx.clone(),
        }
    }
}

impl Spawner {
    pub(crate) fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // Spawner only spawns secondary tasks (i.e. we are not interested on there
        // return value)
        let fut = async move {
            fut.await;
            TaskResult::Secondary
        };

        self.tx
            .try_send(Box::pin(fut))
            .expect("Trying to spawn task while Executor is dropped");
    }
}
