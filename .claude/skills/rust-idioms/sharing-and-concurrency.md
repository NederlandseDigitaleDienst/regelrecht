# Shared ownership, threads and async

The branch of [rust-idioms](SKILL.md) for code that shares a value between
owners, threads or tasks. Chapters are cited as in `SKILL.md`.

## Shared ownership

- Compile-time borrows come first: a plain reference or a `Box<T>`. `RefCell<T>`
  is for the case where you are sure the code obeys the borrow rules and the
  compiler cannot see it, because a violation becomes a panic at runtime,
  possibly only in production (ch15-05).
- `Rc<T>` only when no part of the program clearly outlives the others. If one
  does, it is the owner and the rest borrow (ch15-04).
- Pick by what you need (ch15-05, ch16-03):

  | Need | Type |
  |------|------|
  | one owner, borrows checked at compile time | `Box<T>` or a plain value |
  | several owners, read only | `Rc<T>` |
  | one owner, mutation checked at runtime | `RefCell<T>` |
  | several owners that mutate | `Rc<RefCell<T>>` |
  | several owners across threads | `Arc<T>`, with `Mutex<T>` to mutate |

  `Arc` and `Mutex` cost more than `Rc` and `RefCell`; use them when a second
  thread is real (ch16-03). The book does not cover `std::thread::scope`:
  threads that finish inside one function can borrow from it there, which
  keeps the first rule of this section and needs no `Arc`.
- Write `Rc::clone(&a)`, so a reference-count bump reads differently from a
  deep copy. The book's listings do the same with `Arc::clone(&a)` (ch15-04,
  ch16-03).
- A back-reference is a `Weak<T>`. A child holds `Weak` to its parent and calls
  `upgrade()` before use. Two strong references to each other never drop, and
  nothing in Rust reports it (ch15-06).
- `Box<T>` is for a recursive type, a trait object, or moving a large value
  without copying it. Other values stay on the stack, and a list is a `Vec<T>`
  (ch15-01).
- Cleanup belongs in `Drop`, so no caller can forget it. Release early with
  `drop(value)` (ch15-03).
- `Drop::drop` handles its errors. It can run while a panic unwinds, and an
  `unwrap()` that panics there is a double panic: the program crashes and the
  cleanup still in progress is lost (ch21-03).
- A field that is always present stays a plain field. To move it out in `drop`,
  first look for another way, such as `self.workers.drain(..)`. `Option` plus
  `take()` is for a field that really has to leave early, like a channel sender
  that is dropped to close the channel (ch21-03).

## Threads

- A lock guard created in the condition of `while let`, `if let` or `match`
  lives through the whole body. Bind the result with `let` first, so the lock
  is released before the work starts (ch21-02). Edition 2024 releases an
  `if let` guard before the `else` block, and still not before the body.

  ```rust
  use std::sync::{Mutex, mpsc::Receiver};

  fn work(queue: &Mutex<Receiver<Box<dyn FnOnce() + Send>>>) {
      loop {
          // The guard is a temporary of this statement only.
          let message = queue
              .lock()
              .expect("queue lock should not be poisoned")
              .recv();
          let Ok(job) = message else { break };
          job();
      }
  }
  ```

- Limit a guard's life with an inner block, or `drop(guard)`, as soon as the
  shared value is no longer needed (ch15-03, ch16-03).
- A poisoned lock means another thread panicked while holding it. The book
  lets the waiting thread panic too; a project that recovers the guard instead
  has decided otherwise, and its way holds (ch16-03, ch21-02).
- An operation that takes two locks can deadlock when two threads each hold
  one of them, and the compiler does not see it (ch16-03).
- A shared number that is only counted or added to is a `std::sync::atomic`
  type, not a `Mutex<i32>` (ch16-03).
- A thread started with `thread::spawn` gets its `JoinHandle` kept and joined.
  When `main` returns, running threads are cut off (ch16-01).
- `send` fails once the receiver is gone. Handle that `Result` (ch16-02).
- A type that owns long-lived threads implements `Drop`: close the channel
  first, then join each thread. The worker loop ends when `recv()` returns
  `Err` (ch21-03).
- Bound the number of threads; a thread per request lets a burst of requests
  exhaust the machine (ch21-02).
- `thread::spawn` panics when the system cannot start a thread. Where that has
  to be an error the caller can handle, `thread::Builder::spawn` returns a
  `Result` (ch21-02).
- Receive with `for message in rx`, which ends when the channel closes.
  `try_recv` is for a thread that has other work to do between messages
  (ch16-02).
- `Send` and `Sync` follow from a type's fields. Implementing them by hand is
  unsafe code (ch16-04).

## Async

- Threads for CPU-bound work that runs in parallel, async for I/O-bound work
  that waits. They combine: blocking work on a thread, its results over an
  async channel (ch17-06).
- Nothing long-running sits between two await points. Synchronous code in an
  async block keeps every other future from making progress. Break long work
  up with an explicit yield rather than a short sleep, and measure instead of
  yielding on every line, because yielding is not free either (ch17-03).
- A future does nothing until it is awaited, joined or selected (ch17-01).
- For concurrency inside one task, put the parts in separate async blocks and
  join them. Awaiting them one after another runs them in sequence. This needs
  no spawned task (ch17-02).
- Move the sender into its block with `async move`. The receiving loop,
  `while let Some(value) = rx.recv().await`, ends only when every sender has
  been dropped (ch17-02).
- Await the handle of a spawned task; a task still running when `main` ends is
  cut off (ch17-02).
- Pinning matters only when futures are moved around, for example into a
  collection of `dyn Future`; use `std::pin::pin!` there. `.await` pins on its
  own (ch17-05).
- The book's async examples use its teaching crate `trpl`, a thin layer over
  `futures` and `tokio`. `trpl::join`, `trpl::select`, `trpl::spawn_task`,
  `trpl::yield_now` and `trpl::block_on` are that crate's names: use the
  equivalents of the runtime the project already depends on. That
  `trpl::select` favours its first argument is a property of `trpl`, not of
  select in general (ch17-01, ch17-03).
