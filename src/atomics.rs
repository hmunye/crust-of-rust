//! Atomics provide primitive shared-memory communication between threads.
//!
//! The primary reason to use an atomic primitive (`AtomicBool`, `AtomicUsize`)
//! over built-in primitives (`bool`, `usize`) is to synchronize access to a
//! value between threads and place different restrictions on the code the
//! compiler is allowed to generate and how the CPU performs accesses. Sharing a
//! normal `usize` between two threads, where one reads it's value and another
//! updates it, give no guarantees that the reading thread will ever see any
//! writes made by the writing thread.
//!
//! Rust atomics currently follow the rules of [C++20 atomics].
//!
//! Operations on atomics are performed in a single step (e.g, `load`, `store`,
//! `fetch_add`) to prevent other threads from accessing the value in between
//! such operations.
//!
//! [Ordering] indicates to the compiler which set of guarantees should be
//! applied for a specific atomic memory access with respect to other threads.
//! They dictate the allowed observable behavior when multiple threads interact
//! with the same memory location.
//!
//! [Ordering]: std::sync::atomic::Ordering
//! [C++20 atomics]: https://en.cppreference.com/w/cpp/atomic/memory_order.html

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Mutex<T> {
    v: UnsafeCell<T>,
    lock: AtomicBool,
}

// SAFETY: Access to the inner `UnsafeCell` is locked behind an `AtomicBool`.
//
// `T` needs to be `Send` because the lock can be acquired from multiple threads
// and those threads might move the value. `T` does not have to be `Sync` since
// a reference to the inner value `T` is never given out.
unsafe impl<T: Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    const LOCKED: bool = true;
    const UNLOCKED: bool = false;

    pub fn new(val: T) -> Self {
        Self {
            v: UnsafeCell::new(val),
            lock: AtomicBool::new(Self::UNLOCKED),
        }
    }

    /*
    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // Spin-lock that loops until the lock is released (set to UNLOCKED).
        while self.lock.load(Ordering::Relaxed) != Self::UNLOCKED {
            std::hint::spin_loop()
        }

        // This implementation is not thread-safe due to a race condition
        // between the load and store operations.
        //
        // Because the load and store are separate atomic operations with no
        // memory ordering guarantees, it's possible for multiple threads to:
        //  - observe the lock as UNLOCKED
        //  - exit the spin-loop simultaneously
        //  - and then both store LOCKED
        //
        // As a result, more than one thread may enter the critical section at
        // the same time, violating mutual exclusion.
        //
        // This issue can occur whether the threads are on different cores or
        // time-sliced on a single core (due to OS preemption). A thread may be
        // interrupted between load and store, letting another thread acquire
        // the lock improperly.

        // Simulates preemption scenario.
        // std::thread::yield_now();

        self.lock.store(Self::LOCKED, Ordering::Relaxed);

        // SAFETY: At this point, we believe we hold the lock, so we can safely
        // create a mutable reference to the inner value.
        let ret = f(unsafe { &mut *self.v.get() });

        self.lock.store(Self::UNLOCKED, Ordering::Relaxed);

        ret
    }
    */

    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // Attempt to acquire the lock using an atomic compare-and-swap (CAS)
        // operation. `compare_exchange_weak` takes four arguments:
        //
        //   - The expected current value (`UNLOCKED`)
        //   - The new value to write if the current value matches (`LOCKED`)
        //   - The memory ordering to use on success (`Ordering::Acquire`)
        //   - The memory ordering to use on failure (`Ordering::Relaxed`)
        //
        // It returns `Ok(previous_value)` if the value matched and was updated,
        // and `Err(current_value)` otherwise.
        //
        // The important part is that this is done in one atomic
        // read-modify-write (RMW) operation. No other thread can observe an
        // intermediate state in between the load and store like it could with
        // separate `load` and `store` operations.

        // We use `compare_exchange_weak` instead of `compare_exchange` because
        // `weak` is allowed to fail spuriously, not just if the current value
        // doesn't match, but if, for example, exclusive access to the cache
        // line is lost. This is common on some architectures like ARM, where
        // CAS is implemented using a Load-Linked / Store-Conditional (LL/SC)
        // pair (e.g., `ldrex/strex`). In those cases, the store (`strex`) can
        // fail even when the value hasn't changed, simply because another core
        // accessed the same cache line. Using `weak` in a loop avoids excessive
        // cache-line contention and performs better in practice. On x86, both
        // versions compile to the same instruction (`lock cmpxchg`) and behave
        // identically, so it is mainly for portability.

        // In this call, we use `Ordering::Acquire` on success. This ensures
        // that all memory writes performed by another thread before it released
        // the lock (with `Ordering::Release` or stronger) become observable to
        // us. In other words, `Acquire` guarantees we see the "happens-before"
        // timeline established by the previous release. But importantly, it
        // does not guarantee we load the most recent value stored, just that
        // the value we do observe comes with a consistent timeline of writes
        // leading up to it. `Ordering::AcqRel` is not needed since the final
        // `Ordering::Release` store of the lock is enough to synchronize with
        // any other write, the store made by `compare_exchange_weak` does not
        // also have to be synchronized and can be `Relaxed`.
        //
        // Each `Release` store can be thought of as capturing a timeline of
        // prior writes. When an `Acquire` load reads one of those values, it
        // synchronizes with that "timeline", even if it’s not the most recent
        // store.
        //
        // The failure ordering is `Relaxed`, which is sufficient because a
        // failed CAS means we didn’t acquire the lock so no synchronization or
        // observability guarantees are required in that case.
        while self
            .lock
            .compare_exchange_weak(
                Self::UNLOCKED,
                Self::LOCKED,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_err()
        {
            // If CAS fails, we avoid hammering the cache line with more RMW
            // operations.
            //
            // CAS (compare_exchange) requires exclusive access to the cache
            // line which means multiple threads attempting it simultaneously
            // will contend for ownership, causing the MESI protocol to
            // constantly bounce the line between cores (invalidating,
            // upgrading, etc).
            //
            // Instead of repeatedly trying CAS, we spin using a relaxed `load`
            // to check whether the lock is still held. Reads don’t require
            // exclusive access meaning they can be shared between cores, so
            // this is a much cheaper way to wait until the lock becomes
            // available again.
            //
            // `Relaxed` ordering is fine here because we don’t need
            // any synchronization guarantees, we’re just observing the lock
            // state.
            while self.lock.load(Ordering::Relaxed) == Self::LOCKED {
                std::hint::spin_loop()
            }
        }

        // SAFETY: At this point, we believe we hold the lock, so we can safely
        // create a mutable reference to the inner value.
        let ret = f(unsafe { &mut *self.v.get() });

        // After the closure finishes, we release the lock using
        // `Ordering::Release`.
        //
        // This ensures that all writes performed inside the critical section
        // become observable to any thread that subsequently acquires the lock
        // with `Ordering::Acquire` or stronger. With a weaker ordering, another
        // thread might acquire the lock and not see the updates made here, even
        // though they happened before the lock was released.
        self.lock.store(Self::UNLOCKED, Ordering::Release);

        ret
    }
}

/// In this function, it’s possible that both `r1` and `r2` end up as 42. This
/// happens because `Ordering::Relaxed` provides no synchronization or ordering
/// guarantees between threads, only atomicity of individual operations.
///
/// Each atomic variable maintains a modification order, which is a total order
/// of all writes to that variable. For example, `x`’s modification order might
/// be:
///
///   0 (initial) -> 42 (written by `t1`)
///
/// Similarly for `y`.
///
/// However, relaxed loads can observe any value from that variable’s
/// modification order, not necessarily the most recent. Because of this, the
/// load of `x` in thread `t2` can observe either the initial value 0 or the
/// updated value 42.
///
/// In the program, `t1` loads `y` (initially 0), then stores that value into
/// `x`. Meanwhile, `t2` loads from `x` (which may or may not yet be updated)
/// and stores 42 into `y`. Due to the lack of synchronization, the stores and
/// loads can be observed in any order, making it possible for both threads to
/// read the value 42, even if the thread storing 42 has not executed yet.
///
/// Essentially, the CPU (and the memory model) treats each atomic variable’s
/// modification order as the definitive sequence of all values that have been,
/// or could be, written to that memory location. This is more holistic than the
/// intuitive, linear way humans think, where we expect `t1` to complete its
/// write of 42 before `t2` can ever observe 42.
///
/// With `Ordering::Relaxed`, this assumption breaks down. The value 42 is
/// considered part of `x`’s modification order once the store is initiated,
/// even if other threads haven’t yet observed it in program order. So, under a
/// `Relaxed` load on `x` by `t2` can legally observe `42` without any guarantee
/// that the write actually happened first in wall-clock time or in any global
/// order that the threads observe.
pub fn atomic_relaxed() {
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    let x: &'static _ = Box::leak(Box::new(AtomicUsize::new(0)));
    let y: &'static _ = Box::leak(Box::new(AtomicUsize::new(0)));

    let t1 = thread::spawn(move || {
        let r1 = y.load(Ordering::Relaxed);
        x.store(r1, Ordering::Relaxed);
        r1
    });

    let t2 = thread::spawn(move || {
        let r2 = x.load(Ordering::Relaxed);
        y.store(42, Ordering::Relaxed);
        r2
    });

    let r1 = t1.join().unwrap();
    let r2 = t2.join().unwrap();

    println!("r1: {r1}, r2: {r2}");
}

/// Possible values of `z` after both threads finish:
///
///  - 0?
///    With just acquire/release ordering, 0 is possible because each thread's
///    loads of `x` and `y` can legally observe values consistent with the
///    happens-before relationship, but not necessarily synchronized globally.
///
///    For example, `t1` might see `x` as true (because of `_tx` storing it),
///    but see `y` as false (if `_ty` hasn't completed or `t1` reads stale data),
///    so it won't increment `z`. Similarly for `t2`. This lack of global
///    ordering allows both to skip increments, leaving `z` at 0.
///
///  - 1?
///    This can happen if one thread observes both flags as true, and the other
///    thread does not. For instance, if `_tx` and `t1` run first and set `x` to
///    true and `t1` observes `y` as false, but later `_ty` and `t2` run and see
///    `y` as true but `x` as false, only one increment occurs.
///
///  - 2?
///    Both threads observe `x` and `y` as true. This happens if `_tx` and `_ty`
///    complete before `t1` and `t2` run, so both threads enter the critical
///    sections and each increments `z` once.
///
///
/// When using `Ordering::SeqCst`, the behavior is stricter:
///
///  - 0 is not possible because sequential consistency enforces a single total
///    order of all operations seen by all threads. If `t1` sees `x == true`
///    and `y == true`, then any other thread observing `y == true` must also
///    see `x == true` to maintain a consistent global order. This guarantees
///    that if either thread observes both flags as true, the other must as well,
///    preventing inconsistent partial observations.
pub fn atomic_sequentially_consistent() {
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    let x: &'static _ = Box::leak(Box::new(AtomicBool::default()));
    let y: &'static _ = Box::leak(Box::new(AtomicBool::default()));
    let z: &'static _ = Box::leak(Box::new(AtomicUsize::default()));

    let _tx = thread::spawn(move || {
        x.store(true, Ordering::Release);
    });

    let _ty = thread::spawn(move || {
        y.store(true, Ordering::Release);
    });

    let t1 = thread::spawn(move || {
        while !x.load(Ordering::Acquire) {
            std::hint::spin_loop()
        }

        if y.load(Ordering::Acquire) {
            z.fetch_add(1, Ordering::Relaxed);
        }
    });

    let t2 = thread::spawn(move || {
        while !y.load(Ordering::Acquire) {
            std::hint::spin_loop()
        }

        if x.load(Ordering::Acquire) {
            z.fetch_add(1, Ordering::Relaxed);
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();

    let _z = z.load(Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_mutex_valid() {
        let mu: &'static _ = Box::leak(Box::new(Mutex::new(0)));
        let mut handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(move || {
                    for _ in 0..1000 {
                        mu.with_lock(|v| *v += 1);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(mu.with_lock(|v| *v), 10 * 1000);
    }
}
