use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

/// The `async` qualifier on this function `foo` essentially desugars to:
///
/// ```rust
/// fn foo() -> impl Future<Output = usize> {
///     async {
///         println!("foo");
///         42
///     }
/// }
/// ```
///
/// `async` is essentially a transformation directive to the compiler.
async fn foo() -> usize {
    println!("foo");
    42
}

#[allow(clippy::manual_async_fn)]
fn foo1() -> impl Future<Output = usize> {
    // This `async` block defines a Future that, when awaited, begins execution
    // by printing "foo" to `stdout`. It then reaches `foo().await`, at which
    // point execution is suspended until that Future resolves.
    //
    // The `.await` expression can be hypothetically desugared to something
    // like:
    //
    // ```
    // let mut fut = foo();
    // let x = match fut {
    //     mut pinned => loop {
    //         match unsafe { Pin::new_unchecked(&mut pinned) }.poll() {
    //             Poll::Ready(n) => break n,
    //             Poll::Pending => yield,
    //         }
    //     },
    // };
    // ```
    //
    // `.await` essentially polls the inner Future in a loop until it returns
    // `Poll::Ready`, suspending the `async` block each time `Poll::Pending` is
    // returned. While suspended, the task is parked by the executor and resumed
    // later, allowing cooperative multitasking.
    //
    // A compiler-generated `generator` function, which is unstable and used
    // internally to implement `async/await`, transforms an `async` block or
    // function into a state machine where each `.await` acts as a suspension
    // point that triggers a transition to a new state. The compiler generates
    // a data structure to represent this state machine, and it only holds the
    // minimal set of information required to resume execution from the current
    // suspension point. This includes things like which `.await` the execution
    // is currently suspended on and any local variables that are still live
    // across that suspension.
    //
    // `Pin` here ensures that the memory location of the Future
    // (the `async` block's state machine) is guaranteed not to move in memory.
    // This is essential because the compiler generated state-machine may
    // potentially include self-references and if it were ever moved in memory
    // after beginning executed, any self-referential pointers it holds could
    // become invalid and dangle.
    //
    // `Pin<T>` enforces this immovability. It tells the compiler that `T` must
    // not be moved once pinned, unless it implements the `Unpin` auto trait.
    // If `T: Unpin`, it is safe to move it even when pinned
    // (e.g., no self-references). If not, the compiler must ensure the memory
    // location is preserved.
    //
    // The reason `Pin::new_unchecked` would be safe in this hypothetical
    // desugaring is that the entire `async` block is already pinned when its
    // `Future::poll` method is called. This is a requirement enforced by the
    // method signature on `poll`:
    //
    // ```rust
    // fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
    // ```
    async {
        println!("foo");
        let x = foo().await;
        println!("foo1");
        x
    }
}

async fn bar() {
    // `x` here is not a `usize`, but a `Future` that will eventually yield a
    // `usize` when awaited. Specifically, it has the type:
    //
    //      impl Future<Output = usize>
    //
    // Unlike JavaScript `Promises`, Futures are lazy: they do not execute
    // eagerly. Instead, they behave more like iterators, they only make
    // progress when explicitly polled. The `println` in `foo()` will also not
    // execute until the Future is actually awaited.
    //
    // Futures essentially describe a series of instructions that will be
    // executed at some point in the program execution.
    let x = foo();

    // The `await` keyword indicates that instructions following the `await`
    // should not be executed until the Future resolves to it's output type.
    let x = foo1().await;
}
