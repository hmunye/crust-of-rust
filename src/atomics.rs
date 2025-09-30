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
//! Rust atomics currently follow the rules of `C++20` atomics.
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

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Mutex<T> {
    v: UnsafeCell<T>,
    lock: AtomicBool,
}

// SAFETY: Access to the inner `UnsafeCell` is locked behind an `AtomicBool`.
//
// `T` needs to be `Send` because the lock can be acquired from multiple threads
// and those threads might move the value.
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
        // Spin-lock, looping until the lock is released (set to UNLOCKED).
        while self.lock.load(Ordering::Relaxed) != Self::UNLOCKED {
            std::hint::spin_loop()
        }

        // Because we do the load and store in separate atomic operations, two
        // threads may concurrently exit the spin-loop, both perform the store
        // operation, and both enter the critical section, executing the closure
        // on the same previous value of `v`, overwriting their updates made.
        //
        // These threads may be executing on separate cores or one core, but the
        // same issue may occur. On a single core, the OS can preempt a thread
        // at any point in it's execution, including between the load and store
        // operations, preventing that thread from taking the lock and executing
        // its closure with the current value.
        //
        // There is a race condition between the load and store.

        // std::thread::yield_now();

        self.lock.store(Self::LOCKED, Ordering::Relaxed);

        // SAFETY: We hold the lock, therefore we can create a mutable ref.
        let ret = f(unsafe { &mut *self.v.get() });

        self.lock.store(Self::UNLOCKED, Ordering::Relaxed);

        ret
    }
    */

    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // `compare_exchange` takes as arguments: what the current value should
        // be for an exchange to occur, the value to exchange with the current
        // value, and the memory orderings on success and fail. The method
        // returns `Err(T)` if the value was not updated and `Ok(T)` if it was.
        //
        // The important difference here is that the operation is done in one
        // atomic step. It isn't possible for any other thread to access the
        // value in between this thread's read and write, like it was when using
        // load and store as two separate atomic operations.
        //
        // `compare_exchange_weak` performs the same `CAS` (compare-and-swap)
        // operation as `compare_exchange`, but is allowed to fail spuriously.
        // On an architecture like some `ARM` variants,
        // (Load-Linked/Store-Conditional) pairs are used to implement a `CAS`
        // but the `strex` (SC) instruction can fail if exclusive access to the
        // cache-line is lost, even if the current value matches `current`.
        // Using the `weak` implementation for loops avoids unnecessary
        // contention on these platforms, while on `x86`, both versions compile
        // to the same CPU instruction which cannot spuriously fail.
        //
        // `Ordering::Relaxed` only guarantees the operation will happen
        // atomically and nothing else. An `Ordering::Acquire` load of a value
        // stored using `Ordering::Release` or stronger must observe all
        // operations that happen before that `Ordering::Release` store,
        // essentially synchronizing with the most recent store of the value in
        // modification order. The acquire/release pair form a happens-before
        // relationship between the previous thread that stores to the value
        // `lock` and the next thread that loads it.
        //
        // A RMW (read-modify-write) operation performs both a load and store,
        // so using `Ordering::Acquire` results in the load being
        // `Ordering::Acquire` but the store being `Ordering::Relaxed`. Using
        // `Ordering::AcqRel` ensures the load is `Ordering::Acquire` and the
        // store is `Ordering::Release`. In this case it is unneeded since the
        // `Ordering::Release` store and `Ordering::Acquire` load establish the
        // happens-before relationship, so the critical section is left ordered.
        // `Ordering::AcqRel` is more commonly used when performing a single
        // modification operation.
        //
        // In this case, the fail ordering of `Ordering::Relaxed` is fine since
        // we do not need the thread that failed to acquire the lock to
        // synchronize with the last thread that released the lock.
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
            // MESI protocol (cache coherence)
            //
            // This optimization allows us to spin-loop on an operation that
            // only requires a read on the memory location meaning it can be
            // shared between threads/cores. This avoids the coordination that
            // is required for something like `compare_exchange`, since that
            // would need exclusive access to the cache line for a
            // read-modify-write operation.
            //
            // `Ordering::Relaxed` is fine here since we do not need to
            // synchronize with any other threads because the lock is not
            // acquired, just observed.
            while self.lock.load(Ordering::Relaxed) == Self::LOCKED {
                std::hint::spin_loop()
            }
        }

        // SAFETY: We hold the lock, therefore we can create a mutable ref.
        let ret = f(unsafe { &mut *self.v.get() });

        // By using `Ordering::Release`, it guarantees that any load of `lock`
        // using `Ordering::Acquire` or stronger must observe all previous
        // operations that happen before the store (happens-before relationship)
        // and that nothing can be ordered after the `Ordering::Release` store.
        //
        // With `Ordering::Relaxed`, modifications made before the store may
        // never be observed by other threads who acquire the lock.
        self.lock.store(Self::UNLOCKED, Ordering::Release);

        ret
    }
}

// With this function, it is possible that `r1 == r2 == 42`. This is because
// when multiple threads execute concurrently, there are essentially no
// guarantees on what a specific thread reads that another thread wrote under
// `Ordering::Relaxed`.
//
// Typically for atomic operations, there is a modification order stored per
// value. In this example, the modification order for `x` and `y` would be
// (0 42). With `Ordering::Relaxed`, you can observe any value written by any
// thread to that memory location. The load of `x` in `t2` is allowed to observe
// any value stored to `x`. Because the load of `y` in `t1` is stored in `x`, 42
// is now apart of x's modification set. Therefore any other thread loading `x`
// under `Ordering::Relaxed` can observe the initialized value 0 or 42.
#[allow(dead_code)]
fn atomic_relaxed() {
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

//
#[allow(dead_code)]
fn atomic_sequentially_consistent() {
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

    // Possible values of `z`:
    //  - 0?
    //    Restrictions:
    //      - t1 runs after _tx
    //      - t2 runs after _ty
    //
    //    Yes because with acquire/release ordering, the acquire is synchronized
    //    with the value loaded from `x` or `y`, which could be false or true
    //    given their modification sets. `Ordering::Acquire` is allowed to
    //    observe and value from that modification set when loaded subject to
    //    the happens-before relationship.
    //
    //    With `Ordering::SeqCst`, 0 is no longer a possible value. Because a
    //    thread, say `t1`, sees `x` as true and `y` as true, all other threads
    //    must be consistent with that observation. When `t2` sees `y` as true,
    //    `x` must be true to stay consistent. There must exist some ordering
    //    that is consistent among all of the threads that observe under
    //    `Ordering::SeqCst`.
    //
    //  - 1?
    //    Yes with thread schedule (_tx, t1, _ty, t2)
    //
    //  - 2?
    //    Yes with thread schedule (_tx, _ty, t1, t2)
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
