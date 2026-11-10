# Unsafe code and FFI

The branch of [rust-idioms](SKILL.md) for `unsafe`, raw pointers and foreign
functions. Everything here is ch20-01 unless another chapter is named.

Reach for `unsafe` when you know the code is correct and the compiler cannot
see it. An `unsafe` block is your statement that you read the contract of what
you call and uphold it.

## Shape

- An `unsafe` block is as small as the operation that needs it. A memory bug
  then has few places to hide.
- Unsafe code sits behind a safe function. The function checks its
  preconditions in safe code first (an `assert!`, a length comparison), so the
  pointers the block uses are valid by construction, and callers never write
  `unsafe`. Containing unsafe code is no reason to mark the whole function
  `unsafe`.
- Inside an `unsafe fn`, each unsafe operation still gets its own `unsafe`
  block. The 2024 edition warns otherwise.
- Every unsafe operation has a `// SAFETY:` comment saying why the rules hold
  at this spot. Every `unsafe fn` documents what the caller must uphold, under
  `# Safety` in its doc comment (ch14-02).

  ```rust
  fn split_at_mut(values: &mut [i32], mid: usize) -> (&mut [i32], &mut [i32]) {
      let len = values.len();
      let ptr = values.as_mut_ptr();
      assert!(mid <= len);

      // SAFETY: `mid <= len` was just checked, so both ranges lie inside
      // `values` and they do not overlap.
      unsafe {
          (
              std::slice::from_raw_parts_mut(ptr, mid),
              std::slice::from_raw_parts_mut(ptr.add(mid), len - mid),
          )
      }
  }
  ```

## 2024 syntax

Older Rust wrote `extern` blocks and `#[no_mangle]` without `unsafe`; those
forms no longer compile. The raw borrow operators replace a cast from a
reference, which still compiles.

```rust
unsafe extern "C" {
    // `safe` is your promise that any input is fine; calls need no `unsafe`.
    safe fn abs(input: i32) -> i32;
    // Without `safe`, every call needs an `unsafe` block.
    fn strlen(s: *const std::ffi::c_char) -> usize;
}

// The attribute is unsafe, the function is not. The exported name has to be
// unique across everything that gets linked.
#[unsafe(no_mangle)]
pub extern "C" fn call_from_c() {}

fn raw_pointers() {
    let mut num = 5;
    let read = &raw const num;
    let write = &raw mut num;
    // SAFETY: both pointers come from a live local and nothing else uses it.
    unsafe {
        *write += 1;
        assert_eq!(*read, 6);
    }
}
```

- Make a raw pointer from a place with `&raw const` or `&raw mut`. Casting an
  integer with `as` gives a pointer nobody can vouch for.
- Mark a foreign function `safe` only after checking that it is safe for every
  input. The compiler takes your word for it.

## Statics, `Send`, `Sync`

- Shared mutable state goes through the concurrency tools of ch16, such as
  `Mutex<T>` behind an `Arc<T>`, which the compiler can check. `static mut` is
  the last resort.
- If a `static mut` is unavoidable, reach it through a raw pointer,
  `*(&raw const COUNTER)`. Taking a reference to it is denied by lint, and that
  includes the reference `println!` takes.
- `unsafe impl Send` or `Sync` comes after you have checked the thread-safety
  guarantee yourself. For a type whose fields are `Send` and `Sync`, the traits
  follow without it (ch16-04).

## Checking

- Run Miri over code that contains `unsafe`: `cargo +nightly miri test`. It
  detects undefined behaviour on the paths the tests execute, so the tests
  have to reach the unsafe code.
- A clean Miri run is evidence, not proof. It says nothing about paths that
  did not run.
