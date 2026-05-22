# How to Contribute to LibAFL

For bugs, feel free to open issues or contact us directly. Thank you for your support. <3

## Licensing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project shall be licensed as MPL-2.0, without any additional terms or conditions.

## On AI Assistance

We do NOT accept contributions with any form of AI assistance from first-time contributors.

## Pull Request Guideline

Even though we will gladly assist you in finishing up your PR, try to:

- keep all the crates compiling with *stable* rust (hide the eventual non-stable code under [`cfg`s](https://github.com/AFLplusplus/LibAFL/blob/main/crates/libafl/build.rs#L26))
- run `cargo +nightly fmt` on your code before pushing
- check the output of `cargo clippy --all` or `./scripts/clippy.sh` (On windows use `.\scripts\clippy.ps1`)
- run `cargo build --no-default-features` to check for `no_std` compatibility (and possibly add `#[cfg(feature = "std")]`) to hide parts of your code.
- Please add and describe your changes to MIGRATION.md if you change the APIs.

You can also run ./scripts/precommit.sh to execute checks that will be performed on a PR.

Some of the parts in this list may be hard, don't be afraid to open a PR if you cannot fix them by yourself, so we can help.

### Pre-commit Hooks

Some of these checks can be performed automatically during commit using [pre-commit](https://pre-commit.com/).
Once the package is installed, simply run `pre-commit install` to enable the hooks, the checks will run automatically before the commit becomes effective.

### Adding dependencies

Avoid adding additional crates dependencies if it can be avoided in general.
Check if the dependency to add is not already present in the root `Cargo.toml` file or in other crates.
If it is the case, use the dependency using `workspace = true` when adding the dependency.
As a rule of thumb, if a given dependency is used more than once, it should be added in the root `Cargo.toml` file.

## LibAFL Code Rules

Before making your pull requests, try to see if your code follows these rules.

- Wherever possible, use `Cow<'static, str>` instead of String.
- `PhantomData` should have the smallest set of types needed. Try not adding `PhantomData` to your struct unless it is really necessary. Also even when you really need `PhantomData`, try to keep the types `T` used in `PhantomData` as smallest as possible
- Wherever possible, trait implementations with lifetime specifiers should use '_ lifetime elision.
- Complex constructors should be replaced with `typed_builder`, or write code in the builder pattern for yourself.

## Rules for Generics and Associated Types

1. Remove generic restrictions at the definitions (e.g., we do not need to specify that types impl `Serialize`, `Deserialize`, or `Debug` anymore at the struct definitions). Therefore, try avoiding code like this unless the constraint is really necessary.

```rust
pub struct X<A> 
    where
        A: P // <- Do not add contraints here
{
    fn ...
}
```

2. Reduce generics to the least restrictive necessary. __Never overspecify the constraints__. There's no automated tool to check the useless constraints, so you have to verify this manually.

```rust
pub struct X<A> 
    where
        A: P + Q // <- Try to use the as smallest set of constraints as possible. If the code still compiles after deleting Q, then remove it. 
{
    fn ...
}
```

3. In general, prefer generic to associated types in traits definition as much as possible. They are much easier to use around, and avoid tricky caveats / type repetition in the code. It is also much easier to have unconstrained struct definitions.
Try not to write this:

```rust
pub trait X
{
    type A;
    
    fn a(&self) -> Self::A;
}
```

Try to write this instead:

```rust
pub trait X<A>
{
    fn a(&self) -> A;
}
```

4. You should only declare an associated type on a trait when implementors of that trait actually hold a value of that type. In other words, the associated type needs to correspond to something concrete the struct owns or can return.

```rust
pub trait X {
    type XXX; // OK to define. Because we are sure that implementors of X will hold a value of this type XXX.

    fn a(&self) -> Self::XXX;
}

pub struct MyObject<A> {
    a: A, // MyObject holds an instance of A.
}

impl<A> X for MyObject<A> {
    type XXX = A; // ...so we can bind the associated type to A.

    fn a(&self) -> Self::XXX {
        // and you can define a getter. 
    }
}
```

5. Generic naming should be consistent. Do NOT use multiple name for the same generic, it just makes things more confusing. Do:

```rust
pub struct X<A> {
    phantom: PhanomData<A>,
}

impl<A> X<A> {}
```

But not:

```rust
pub struct X<A> {
    phantom: PhanomData<A>,
}

impl<B> X<B> {} // <- Do NOT do that, use A instead of B
```

6. __Ideally__ the types used in the arguments of methods in traits should have the same as the types defined on the traits.

```rust
pub trait X<A, B, C> // <- this trait have 3 generics, A, B, and C
{
    fn do_stuff(&self, a: A, b: B, c: C); // <- this is good because it uses all A, B, and C.
    
    fn do_other_stuff(&self, a: A, b: B); // <- this is not ideal because it does not have C.
}
```

7. Try to avoid cyclical dependency if possible. Sometimes it is necessary but try to avoid it. For example, The following code is a bad example.

```rust
pub struct X {}
pub struct Y {}

pub trait A {
    fn fuzz<X>(&self, x: &X) 
    {
        x.do_stuff(self);
    }
}

pub trait X {
    fn do_stuff<A>(&self, a: &A); // <- This function should not take a
}
```

trait `X` should not implement any method that takes `a`, any object that could implement `A` trait.

## Formatting

1. Always alphabetically order the type generics. Therefore,

```rust
pub struct X<E, EM, OT, S, Z> {}; // <- Generics are alphabetically ordered
```

But not,

```rust
pub struct X<S, OT, Z, EM, E> {}; // <- Generics are not ordered
```

2. Similarly, generic bounds in `where` clauses should be alphabetically sorted.
Prefer:

```rust
pub trait FooA {}
pub trait FooB {}

pub struct X<A, B>;

impl<A, B> X<A, B>
where
    A: FooA,
    B: FooB,
{}
```

Over:

```rust
pub trait FooA {}
pub trait FooB {}

pub struct X<A, B>;

impl<A, B> X<A, B>
where
    B: FooB, // <-|
             //   | Generic bounds are not alphabetically ordered.
    A: FooA, // <-|
{}
```
