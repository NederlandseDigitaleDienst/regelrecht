---
name: rust-idioms
description: Rust idioms from The Rust Programming Language (2024 edition) for the places where quickly written Rust drifts from it - ownership in signatures, panic versus Result, types that carry their invariants, function and module size, doc sections. Use when writing, changing or reviewing Rust code.
---

# Rust idioms

These rules come from *The Rust Programming Language* as shipped with the
toolchain (2024 edition). They are the ones the book argues for and that the
compiler and default clippy do not enforce. A project's own lints and review
rules come first where they are stricter. Each rule names its chapter
(`ch09-03`); `rustup doc --book --path` prints the book's index file and the
chapter files sit beside it, for when you want the full reasoning.

## How to use this

1. Read this file, then the reference for every branch your change touches:
   - `Rc`, `RefCell`, `Arc`, `Mutex`, threads, channels, `async`:
     [sharing-and-concurrency.md](sharing-and-concurrency.md)
   - `unsafe`, raw pointers, `extern`, FFI attributes: [unsafe.md](unsafe.md)
   - writing or changing tests: [testing.md](testing.md)
   - a new trait, a generic or `dyn` parameter, a closure parameter, a public
     API, derives, a macro: [api-design.md](api-design.md)
2. Write the code. When reviewing instead, hold the diff against every rule and
   report each deviation with its chapter.
3. Run the gate, `.claude/skills/rust-idioms/book-lints.mjs`, from the
   directory you run cargo in. In this repository that is `packages/`:
   `node ../.claude/skills/rust-idioms/book-lints.mjs -p <crate>`. It runs
   clippy with the lints that correspond to these rules and reports only
   findings on lines you changed. `--base <ref>` reports everything since the
   branch left `<ref>`, and `--all` reports every finding. It sees only what
   clippy compiles: add `--all-targets` when you changed tests, and a `-p` per
   crate you touched.

The work is done when the gate is green, or each remaining finding has a reason
you state in your report, and every new function, type and module has been held
against the sections below, which cover what no lint can see.

## Signatures: borrow by default

- A parameter borrows. It takes ownership only when the function stores or
  consumes the value. Text comes in as `&str`, never `&String`, and never
  `String` just to read it (ch04-03, ch10-03).
- A function that creates a value returns it owned. A function that selects
  part of its argument returns a slice or reference into it, not an index that
  can go stale (ch04-02, ch04-03).
- Every `.clone()` is a decision. It is fine for a few small values copied
  once (ch12-03). It goes when the callee can own the value instead: take
  `impl Iterator<Item = String>` and move items out with `next()`, rather than
  `&[String]` plus a `clone` per item (ch13-03).
- Pass an iterator on as an iterator. Collect into a `Vec` only when something
  needs the `Vec`, and return `impl Iterator<Item = T>` when the caller may
  stop early (ch13-03).
- A lifetime goes only on the parameters the return value borrows from, even
  when the compiler suggests the same `'a` on all of them. When it suggests
  `'static`, a reference is outliving its value: fix that instead (ch10-03,
  ch12-04).
- A struct owns its fields (`String`, not `&str`) unless it is deliberately a
  short-lived view (ch05-01).

## Errors: `Result` by default, panic for bugs

- Return `Result` whenever failure is something a caller can meet: bad input, a
  missing file, a parse, a limit. Panicking decides for the caller that there
  is nothing to recover (ch09-03).
- Panic only on a broken contract: an unexpected state that is a bug in the
  calling code, where the code after it relies on that state not occurring or
  no type could rule it out (ch09-03).
- Where a failure is impossible by construction, use `expect` rather than
  `unwrap`, with the reason phrased as what should hold:
  `.expect("hardcoded address should be valid")`. The message is the marker
  for whoever changes the assumption later (ch09-02, ch09-03). A project that
  lints against `expect` as well wants the error propagated, or an explicit
  exemption with that same reason.
- Propagate with `?` and let `From` convert: one
  `impl From<ParseIntError> for MyError` covers every call site (ch09-02).
  That fits a source error with one meaning in your error type. Where it has
  several (a read and a write both fail with `io::Error`), convert at the call
  site, so the error says which. `?` does not cross between `Option` and
  `Result`; convert with `ok_or` or `ok` (ch09-02).
- `new` promises to succeed. A constructor that returns `Result` gets another
  name: the book uses `build`, and `FromStr` or `TryFrom` say the same through
  a standard trait (ch12-03).
- Every `Result` is handled or propagated. When only the error matters, write
  `if let Err(e) = run()` (ch02-00, ch12-03).

## Types: the type carries the invariant

- Validate once, in a type. A value with a rule (a range, a format, non-empty)
  gets its own type with a private field, a checking constructor and a getter.
  Functions then take that type and skip the check (ch09-03).
- A newtype keeps meanings apart: `struct Millimeters(u32)` cannot be passed
  where `Meters` is expected. A `type` alias checks nothing; it only shortens a
  long type (ch20-03).
- Enum variants hold their own data. A struct with a `kind` field, plus fields
  that only mean something for some kinds, is an enum written sideways. A
  value that can be absent is an `Option<T>`, so the compiler makes every user
  handle the empty case (ch06-01).
- States as types. When an operation is valid only in some states, each state
  is its own type and the transition consumes `self` and returns the next one:
  `fn request_review(self) -> PendingReviewPost`. Calling it in the wrong state
  then fails to compile, where a state field would fail at runtime (ch18-03).
- Values that travel together are a struct with named fields. A function that
  returns a tuple its caller destructures straight away is missing a struct
  (ch05-02, ch12-03).
- A type that guards an invariant keeps its fields private: behaviour goes
  through methods, and read access is a getter named after the field. A plain
  carrier of data has nothing to guard and needs no getters (ch05-03, ch07-03,
  ch18-01).
- Integer arithmetic that can overflow on real input says what should happen:
  `checked_*`, `saturating_*` or `wrapping_*`. Left implicit, it panics in
  debug builds and wraps silently in release. Arithmetic whose operands are
  bounded so that it cannot overflow stays plain (ch03-02).
- Text is walked with `.chars()` or `.bytes()`. A byte-range slice `&s[a..b]`
  panics when it lands inside a character, so slice only at an index that a
  search over the same string returned (ch04-03, ch08-02).
- `HashMap` iterates in arbitrary order. Sort, or use an ordered collection,
  before anything a user or a test can observe depends on it (ch08-03).

## Control flow

- `let ... else` states the precondition and leaves the main path un-nested.
  Use it where the alternative is a body wrapped in `if let`, or a two-arm
  `match` whose other arm returns (ch06-03, ch19-02).

  ```rust
  fn first_word_len(line: Option<&str>) -> Result<usize, &'static str> {
      let Some(line) = line else {
          return Err("no line");
      };
      let Some(word) = line.split_whitespace().next() else {
          return Err("empty line");
      };
      Ok(word.len())
  }
  ```

- `match` over an enum when every variant must be handled: `if let` chains
  give up the exhaustiveness check, and so does an arm with a guard (ch06-03,
  ch19-01, ch19-03).
- A bare name in a pattern is a new binding that matches anything. To compare
  against an existing variable use a guard, `Some(n) if n == expected`. `_name`
  still binds and can move the value; `_` does not (ch19-03).
- When branches repeat the same calls with different values, compute the values
  in the `if` or `match` and run the shared code once (ch21-01).
- Edition 2024 has let chains: `if let Some(n) = limit && n > 30 { ... }`. The
  shipped text of ch19-01 predates them and says this cannot be written.

## Structure: thin `main`, one idea per function

- `main` parses the arguments, builds the configuration, calls `run` and turns
  what comes back into a message on stderr (`eprintln!`) and an exit status.
  That mapping is the only error handling in `main`. Everything else lives in
  the library crate, where a test can reach it (ch07-03, ch12-03, ch12-06).
- One idea per function. A function that reads, decides and writes is three
  functions, and the gate flags any new one over 100 lines (ch12-02).
- A module that grows is split into `foo.rs` and `foo/bar.rs`. The book calls
  `foo/mod.rs` the older style and warns against mixing the two, so a crate
  that already uses `mod.rs` keeps it (ch07-05).
- Items are private by default; `pub` marks only what callers need (ch07-03).
- `use` brings a function's parent module into scope, so the call reads
  `fs::read_to_string(..)`, and brings a type in by its full path. Paths start
  at `crate::`; `super::` is for a module that moves with its parent. Name what
  you import, since a glob hides where a name comes from; `use super::*;` in a
  test module is the exception (ch07-03, ch07-04).
- A hardcoded value gets a name that explains it:
  `const THREE_HOURS_IN_SECONDS: u32 = 60 * 60 * 3;` (ch03-01).
- Format arguments go inline: `"{name}: {value:?}"` (ch02-00).

## Docs

- A doc comment on public API carries the sections that apply, from the
  book's checklist (ch14-02):
  `# Errors` on anything that returns `Result` (which errors, under which
  conditions), `# Panics` (the conditions that panic), `# Safety` on an
  `unsafe fn` (what the caller must uphold), and `# Examples` on the entry
  points of a library, with code that shows the use. Examples run under
  `cargo test`, so they cannot drift.
- `//!` at the top of a crate or module says what it is for and, where it has
  parts, how they are organized (ch14-02).
