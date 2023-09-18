use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Instant;

use once_cell::sync::Lazy;
use parking::{Parker, Unparker};

use crate::heap::OrderedMap;
use crate::id::{Id, IdGenerator};

static EVENT_SOURCE: Lazy<Arc<EventSource>> = Lazy::new(|| EventSource::try_new().unwrap());

pub struct Timer {
    handle: Option<Handle>,
    when: Instant,
}

impl Timer {
    pub fn new(when: Instant) -> Self {
        Self { handle: None, when }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            EVENT_SOURCE.deregister(handle.id(), self.when)
        }
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(id) = self.handle.as_ref().map(Handle::id) {
            EVENT_SOURCE.update(id, self.when, cx.waker().clone());
        } else {
            self.handle = Some(EVENT_SOURCE.register(self.when, cx.waker().clone()));
        }
        if let Some(handle) = self.handle.as_ref() {
            if handle.timeout.load(Ordering::SeqCst) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        } else {
            Poll::Pending
        }
    }
}

struct EventSource {
    id_generator: IdGenerator,
    scheduled: OrderedMap<(Instant, Id<Timer>), ScheduledWaker>,
    unparker: Unparker,
}

impl EventSource {
    fn register(&self, when: Instant, waker: Waker) -> Handle {
        let id = self.id_generator.next();
        let notifier = Arc::new(AtomicBool::default());
        let scheduled_waker = ScheduledWaker {
            waker,
            notifier: notifier.clone(),
        };
        self.scheduled.insert((when, id), scheduled_waker);
        self.unparker.unpark();
        Handle {
            id,
            timeout: notifier,
        }
    }

    fn update(&self, id: Id<Timer>, when: Instant, waker: Waker) {
        self.scheduled
            .update((when, id), |w| ScheduledWaker { waker, ..w });
        self.unparker.unpark();
    }

    fn deregister(&self, id: Id<Timer>, when: Instant) {
        self.scheduled.delete(&(when, id));
        self.unparker.unpark();
    }

    fn try_new() -> Result<Arc<Self>, std::io::Error> {
        let (parker, unparker) = parking::pair();
        let this = Arc::new(Self {
            id_generator: IdGenerator::default(),
            scheduled: OrderedMap::default(),
            unparker,
        });
        let this_clone = this.clone();
        let _ = std::thread::Builder::new()
            .name("timer event source".into())
            .spawn(move || this_clone.run(parker))?;
        Ok(this)
    }

    fn run(&self, parker: Parker) {
        loop {
            let now = Instant::now();
            if let Some(next_wake) = self.scheduled.first_key().map(|pair| pair.0) {
                if next_wake > now {
                    parker.park_deadline(next_wake);
                } else {
                    // TODO: when Rust has `drain` method on BTreeMap, replace the following code
                    while let Some((when, _)) = self.scheduled.first_key() {
                        if when > now {
                            break;
                        }
                        let (_, waker) = self.scheduled.pop_first().unwrap();
                        waker.wake();
                    }
                }
            } else {
                parker.park();
            }
        }
    }
}

struct Handle {
    id: Id<Timer>,
    timeout: Arc<AtomicBool>,
}

impl Handle {
    fn id(&self) -> Id<Timer> {
        self.id
    }

    fn state(&self) -> Poll<()> {
        Poll::Pending
    }
}

struct ScheduledWaker {
    waker: Waker,
    notifier: Arc<AtomicBool>,
}

impl ScheduledWaker {
    fn wake(self) {
        self.notifier.store(true, Ordering::SeqCst);
        self.waker.wake();
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use more_asserts::debug_assert_le;

    use crate::block::block_on;
    use crate::timer::Timer;

    #[test]
    fn test_timer() {
        let begin = Instant::now();
        let duration = Duration::from_secs(3);
        let timer = Timer::new(begin + duration);
        block_on(timer);
        let actual_duration = Instant::now() - begin;
        let diff = duration.as_millis().abs_diff(actual_duration.as_millis());
        debug_assert_le!(diff, 10);
    }
}
