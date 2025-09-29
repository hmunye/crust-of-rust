//! Channels provide a communication primitive between a `sender` and `receiver`
//! either in a single-threaded or multi-threaded context.
//!
//! MPSC (Multiple producer, single consumer) channels allow for many-to-one
//! communication between multiple senders and one receiver (fan-in pattern).
//!
//! Flavors of channels:
//!
//! - Synchronous: Channel where `send()` can block, buffer is bounded.
//!     - Mutex + Condvar + Queue (VecDeque)
//!     - Atomic Queue + thread::park + thread::Thread::unpark
//!
//! - Asynchronous (non-blocking): Channel where `send()` cannot block, buffer
//!   is unbounded.
//!     - Mutex + Condvar + Queue (VecDeque)
//!     - Mutex + Condvar + LinkedList (no resizing)
//!     - Atomic Queue / Atomic Block Linked List + thread signaling
//!
//! - Rendezvous: Synchronous channel with 0 capacity. Typically used for thread
//!   synchronization, not sending data. Cannot send unless there is a `recv()`
//!   blocking, or receive unless there is a `send()` blocking.
//!     - Thread signaling
//!
//! - Oneshot: Technically unbounded channel. In practice, only one call to
//!   `send()`. Can be used for notifying on caught signals, signaling threads
//!   to terminate, etc.
//!     - Atomic Option + thread signaling

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug)]
pub struct RecvError {}

impl std::error::Error for RecvError {}

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RecvError: channel disconnected")
    }
}

struct Inner<T> {
    // So we can have FIFO communication over the channel.
    queue: VecDeque<T>,
    // So we can determine if the channel is closed.
    //
    // `Arc::strong_count` could be used instead to determine if the channel
    // is closed (1 means only the receiver hasn't been dropped), but it
    // doesn't differentiate between `Sender` and `Receiver` meaning we
    // could not potentially notify any blocked `recv` when dropping the last
    // `Sender`.
    senders: usize,
}

struct Shared<T> {
    mu: Mutex<Inner<T>>,
    avail: Condvar,
}

// Sender type of a channel.
pub struct Sender<T> {
    // `Arc` is used so the `Sender` can share the same instance of `ChanInner`
    // with all senders and the receiver.
    inner: Arc<Shared<T>>,
}

// Since we have multiple producers (senders), `Sender` needs a `Clone` impl.
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut guard = self.inner.mu.lock().unwrap();
        guard.senders += 1;
        drop(guard);

        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut guard = self.inner.mu.lock().unwrap();
        guard.senders -= 1;
        let senders = guard.senders;
        drop(guard);

        // Ensure any `Receivers` are awoken if this is the last `Sender`.
        if senders == 0 {
            self.inner.avail.notify_one();
        }
    }
}

impl<T> Sender<T> {
    pub fn send(&self, val: T) {
        let mut inner = self.inner.mu.lock().unwrap();
        inner.queue.push_back(val);

        // Ensure we drop the `MutexGuard` before notifying the `Receiver`,
        // since it will attempt to reacquire the lock. If the notification
        // happens before this function drops the Mutex, a deadlock can occur.
        drop(inner);

        // Notify the waiting `Receiver` there is a value in the queue.
        // `notify_one` is used since this is a `MPSC` channel.
        self.inner.avail.notify_one();
    }
}

// Receiver type of a channel.
pub struct Receiver<T> {
    // `Arc` is used so the `Receiver` can share the same instance of
    // `ChanInner` with all senders.
    inner: Arc<Shared<T>>,
    // Since out implementation uses only one `Receiver`, we can keep a local
    // buffer of all sent items to reduce the number of times we lock to access
    // the shared queue.
    buf: VecDeque<T>,
}

impl<T> Receiver<T> {
    pub fn recv(&mut self) -> Result<T, RecvError> {
        if let Some(val) = self.buf.pop_front() {
            return Ok(val);
        }

        let mut inner = self.inner.mu.lock().unwrap();

        // Looping ensures any spurious wakeup from will rewait  if the
        // condition is not met. The OS does not guarantee that `CondVar::wait`
        // will return only on a notify from another thread.
        loop {
            match inner.queue.pop_front() {
                Some(val) => {
                    // If the shared queue is non-empty, swap it with the local
                    // buffer held by the `Receiver`, so future `recv` do not
                    // need to acquire the mutex.
                    if !inner.queue.is_empty() {
                        std::mem::swap(&mut self.buf, &mut inner.queue);
                    }

                    return Ok(val);
                }
                // Channel is closed.
                None if inner.senders == 0 => return Err(RecvError {}),
                None => {
                    // Blocks the current thread until `avail` is notified by
                    // another thread. It is given a `MutexGuard` so it can
                    // atomically release the lock, and reacquire it once
                    // notified.
                    inner = self.inner.avail.wait(inner).unwrap();
                }
            }
        }
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Shared {
        mu: Mutex::new(Inner {
            queue: VecDeque::new(),
            senders: 1,
        }),
        avail: Condvar::new(),
    });

    (
        Sender {
            inner: inner.clone(),
        },
        Receiver {
            inner: inner.clone(),
            buf: VecDeque::new(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chan_ping_pong() {
        let (tx, mut rx) = channel();
        tx.send(42);
        assert_eq!(rx.recv().unwrap(), 42)
    }

    #[test]
    fn test_chan_closed_tx() {
        // Drop the `Sender` immediately.
        let (_, mut rx) = channel::<()>();

        // Ensure this does not block the thread indefinitely.
        assert!(rx.recv().is_err());
    }

    #[test]
    fn test_chan_closed_rx() {
        // Drop the `Receiver` immediately.
        let (tx, _) = channel();

        // This implementation does not notify Senders when the channel is
        // effectively closed.
        tx.send(42);
    }
}
