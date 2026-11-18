# Trait and API design

The branch of [rust-idioms](SKILL.md) for a new trait, a generic or `dyn`
parameter, a closure parameter, a public API, derives, or a macro. Chapters are
cited as in `SKILL.md`.

## Generic, enum or trait object

- A closed set of types known when the code is compiled is an enum, handled
  with `match` (ch08-01, ch18-02).
- A set that code outside this crate has to be able to extend is a trait
  object, `Box<dyn Trait>` (ch18-02).
- Where each use holds one concrete type, write a generic with a trait bound.
  It is resolved at compile time; a trait object pays a lookup per call and
  blocks inlining (ch18-02).
- For behaviour per state, an enum needs a `match` at every place that checks
  the state, so what one state means is spread over those places. Trait objects
  keep each state's behaviour in one `impl`, at the price of states that know
  about each other and some duplicated logic. The book does not pick a winner;
  try the enum and compare (ch18-03).
- Rethink a pattern from an object-oriented language around ownership and
  types before porting it. See "States as types" in `SKILL.md` (ch18-03).

## Traits

- `impl Trait` in a parameter for the simple case; a named `<T: Trait>` when
  the signature has to say more, such as two parameters of the same type
  (ch10-02).
- Several or long bounds move to a `where` clause, so the name, parameters and
  return type stay together (ch10-02).
- A function that needs many type parameters is doing too much; split it
  (ch10-01).
- A trait has few required methods and builds default methods on them, so an
  implementor writes little (ch10-02). Shared behaviour is a default method,
  not an imitation of inheritance (ch18-01).
- An associated type when a type implements the trait once (`Iterator::Item`);
  a generic trait parameter only when several implementations per type are
  wanted (ch20-02).
- A type parameter added to an existing trait gets a default, so existing
  implementations keep compiling (ch20-02).
- Either the trait or the type must be local. To implement a foreign trait on a
  foreign type, wrap the type in a newtype; add `Deref` only if every method of
  the inner type should show through (ch10-02, ch20-02).

## Closures and functions

- Accept a callable as a generic bounded by the least demanding trait that
  fits: `FnOnce` if you call it once, `FnMut` if you call it repeatedly, `Fn`
  if it must not mutate. A bare `fn` pointer shuts closures out; it is for FFI
  (ch13-01, ch20-04).
- Return a closure or iterator as `impl Fn(..)` or `impl Iterator`. Switch to
  `Box<dyn Fn(..)>` only when different closures must share one type, for
  example in a `Vec` (ch10-02, ch20-04).
- Model a new API on the standard-library one it stands in for, down to its
  bounds, so that switching costs callers little: a pool's `execute` takes
  what `thread::spawn` takes, `F: FnOnce() + Send + 'static` (ch21-02).
- Write the call site first, then the implementation behind it (ch21-02).

## Public surface

- Re-export with `pub use`, so the public path matches how a caller thinks
  about the domain and the internal module tree stays free to change. Re-export
  a dependency's types that appear in your signatures (ch07-04, ch14-02).
- When one error type recurs across a module, give the module a
  `Result<T>` alias, as `std::io` does:
  `type Result<T> = std::result::Result<T, Error>;` (ch20-03).
- A `type` alias shortens a long repeated type and names its purpose:
  `type Thunk = Box<dyn Fn() + Send + 'static>;` (ch20-03).
- A constructor is an associated function, `Config::build(..)`, not a free
  `parse_config(..)` (ch12-03). Helper types and their constructors stay
  private (ch21-02).
- Receivers: `&self` by default, `&mut self` to mutate, `self` only when the
  method turns the value into something else and the original should be gone
  (ch05-03).
- A count or size is a `usize`. A value that a type cannot rule out, such as
  zero, is checked on entry and documented under `# Panics` or returned as an
  error (ch21-02).

## Derives (appendix-03)

- `Display` is written by hand, for the end user. `Debug` is derived, for the
  programmer.
- `PartialEq` and `Debug` on any type that appears in `assert_eq!`.
- `Eq` and `Hash` for a `HashMap` key. `Eq` says every value equals itself,
  which a type containing a float cannot promise: `NaN` is not equal to
  itself.
- Derived `PartialOrd` and `Ord` compare fields in declaration order, and rank
  enum variants in declaration order. With either derive, that order is part of
  the type's behaviour: reordering fields changes results.
- `Copy` only when every part is `Copy` and a copy is cheap; always together
  with `Clone`.
- `Default` lets a caller set a few fields and take the rest:
  `Config { verbose: true, ..Default::default() }`.

## Macros

- A function first. A macro is for what a function cannot do: a variable
  number of arguments, or generating an `impl` at compile time. Macro
  definitions are harder to read and maintain (ch20-05).
- A derive macro for trait `Foo` in crate `foo` lives in crate `foo_derive`
  and is named after the trait. Parse with `syn`, generate with `quote`, and
  keep parsing and generation in separate functions (ch20-05).
- A procedural macro reports failure by panicking, so each `panic!` or
  `expect` carries a message that says what was wrong with the input
  (ch20-05).
