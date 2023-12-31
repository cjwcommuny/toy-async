use std::future::Future;
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use futures::channel::oneshot;
use pin_project::pin_project;

struct Executor {
    ready_queue: mpsc::Receiver<Arc<Task>>,
}

impl Executor {
    fn run(self) {
        for task in self.ready_queue.into_iter() {
            let waker = Waker::from(task.clone());
            let context = &mut Context::from_waker(&waker);
            let _ = task.future.lock().unwrap().as_mut().poll(context);
        }
    }
}

pub struct Spawner {
    sender: mpsc::SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn new(max_queued_tasks: usize) -> Self {
        let (sender, ready_queue) = mpsc::sync_channel(max_queued_tasks);
        let executor = Executor { ready_queue };
        std::thread::spawn(|| executor.run()); // TODO: add signal to kill the thread

        Spawner { sender }
    }

    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + 'static + Send,
    ) -> Handle<T> {
        let (sender, receiver) = oneshot::channel();
        let task = Task {
            future: Mutex::new(Box::pin(SelfStoreFuture {
                output: Some(sender),
                future,
            })),
            sender: self.sender.clone(),
        };
        self.sender.send(Arc::new(task)).unwrap();
        Handle { receiver }
    }
}

struct Task {
    // TODO: 能否避免堆分配
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    sender: mpsc::SyncSender<Arc<Task>>,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        self.sender.send(self.clone()).expect("send failed");
    }
}

#[pin_project]
pub struct Handle<T> {
    #[pin]
    receiver: oneshot::Receiver<T>,
}

impl<T> Future for Handle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().receiver.poll(cx).map(Result::unwrap)
    }
}

#[pin_project]
struct SelfStoreFuture<T, F> {
    output: Option<oneshot::Sender<T>>,

    #[pin]
    future: F,
}

impl<T, F> Future for SelfStoreFuture<T, F>
where
    F: Future<Output = T>,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.project();
        this.future.poll(cx).map(|output| {
            if let Some(sender) = this.output.take() {
                sender.send(output).ok();
            }
        })
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use async_std::task::sleep;
    use futures::executor::block_on;

    use crate::spawn::Spawner;

    #[test]
    fn test_ready() {
        let spawner = Spawner::new(10);
        let handle = spawner.spawn(async { 1 });
        let output = block_on(handle);
        assert_eq!(output, 1)
    }

    #[test]
    fn test_sleep() {
        let spawner = Spawner::new(10);
        let handle = spawner.spawn(async {
            sleep(Duration::from_secs(1)).await;
            1
        });
        let output = block_on(handle);
        assert_eq!(output, 1)
    }
}
