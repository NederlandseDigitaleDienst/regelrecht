# Tests

The branch of [rust-idioms](SKILL.md) for writing and changing tests. Chapters
are cited as in `SKILL.md`. A project's own test conventions come first where
they differ.

## Where tests live

- Unit tests sit in the file they test, in `#[cfg(test)] mod tests` with
  `use super::*;` (ch11-03).
- Integration tests sit in the top-level `tests/` directory and use only the
  public API. Each file there is its own crate. Shared helpers go in
  `tests/common/mod.rs`; a `tests/common.rs` would be run as a test file of its
  own (ch11-03).
- Logic that needs testing lives in the library crate. A binary's `main.rs`
  cannot be reached from `tests/` (ch11-03, ch12-03).

## Assertions

- `assert_eq!` and `assert_ne!` rather than `assert!(a == b)`: on failure they
  print both values. The types compared need `PartialEq` and `Debug`
  (ch11-01).
- Give an assertion a message when the default would not explain the failure,
  and put the actual value in it (ch11-01).

  ```rust
  fn greeting(name: &str) -> String {
      format!("Hello {name}!")
  }

  #[test]
  fn greeting_contains_name() {
      let result = greeting("Carol");
      assert!(
          result.contains("Carol"),
          "greeting did not contain the name, value was `{result}`"
      );
  }
  ```

- Assert the property that matters. Where the exact text may change, check
  `contains` on the part the behaviour depends on (ch11-01).
- `#[should_panic(expected = "part of the message")]`. Without `expected` the
  test passes on any panic, including one from an unrelated bug (ch11-01).
- A test may return `Result<(), E>` and use `?`. Such a test cannot be
  `#[should_panic]`; assert on the `Err` it expects, and on which error it is,
  not only that there is one (ch11-01).
- `unwrap` and `expect` are how a test fails: fine here (ch09-03).

## Behaviour

- Tests run in parallel. Each one sets up what it needs and shares no state
  with another: no common file, working directory or environment variable
  (ch11-02).
- A slow test gets `#[ignore = "why"]`, so the default run stays fast and
  `cargo test -- --ignored` runs it on demand (ch11-02).
- A test double records calls through a `RefCell` field. The production trait
  keeps its `&self` signature; a design does not change for the sake of a test
  (ch15-05).
- Watch every new test fail for the reason you expect: write it before the
  code, or break the code once it passes. A test that has never failed has not
  shown that it can (ch12-04).
- Code that compiles has not been shown to work: the book's first thread pool
  compiled and did nothing. Test the behaviour you want (ch21-02).
