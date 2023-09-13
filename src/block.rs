use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use futures::pin_mut;
use parking::{Parker, Unparker};

struct Signal {
    unparker: Unparker,
}

impl Signal {
    fn new() -> (Self, Parker) {
        let (parker, unparker) = parking::pair();
        (Self { unparker }, parker)
    }
}

impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.unparker.unpark();
    }
}

pub fn block_on<F: Future>(fut: F) -> F::Output {
    pin_mut!(fut);
    let (signal, parker) = Signal::new();
    let signal = Arc::new(signal);
    let waker = Waker::from(signal.clone());
    let context = &mut Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(context) {
            Poll::Ready(output) => break output,
            Poll::Pending => parker.park(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use async_std::task::sleep;

    use crate::block::block_on;

    #[test]
    fn test_ready() {
        assert_eq!(block_on(std::future::ready(1)), 1);
    }

    #[test]
    fn test_timeout() {
        let x = block_on(async {
            sleep(Duration::from_millis(250)).await;
            1
        });
        assert_eq!(x, 1);
    }
}
