//! The feedbacks reduce observer state after each run to a single `is_interesting`-value.
//! If a testcase is interesting, it may be added to a Corpus.

use alloc::borrow::Cow;
use core::{fmt::Debug, marker::PhantomData};

use libaflmm_bolts::{
    Named,
    tuples::{Handle, Handled, MatchName, MatchNameRef},
};
use libaflmm_core::Result;
use serde::{Deserialize, Serialize};

use crate::{
    Error,
    common::{DependencyResolver, Registrator},
    corpus::TestcaseId,
    executors::ExitKind,
    observers::TimeObserver,
    states::State,
};

pub mod list;
pub use list::*;

pub mod map;
pub use map::*;

/// The module for list feedback
#[cfg(feature = "nautilus")]
pub mod nautilus;
#[cfg(feature = "nautilus")]
pub use nautilus::*;

pub mod bool;
pub use bool::BoolValueFeedback;

#[cfg(feature = "simd")]
pub mod simd;

pub mod stdio;

pub type StdFeedback<C, O> = StdMapFeedback<C, O>;
pub type StdObjectiveFeedback = FastOrFeedback<CrashFeedback, TimeoutFeedback>;

/// Feedbacks evaluate the observers.
/// Basically, they reduce the information provided by an observer to a value,
/// indicating the "interestingness" of the last run.
pub trait Feedback<I, OT, S>: Named + DependencyResolver {
    /// `is_interesting ` return if an input is worth the addition to the corpus
    fn is_interesting(
        &mut self,
        _state: &mut S,
        _input: &I,
        _observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool> {
        Ok(false)
    }

    /// Append to the testcase the generated metadata in case of a new corpus item
    ///
    /// Precondition: `testcase` must contain an input.
    #[inline]
    fn append_metadata(
        &mut self,
        _state: &mut S,
        _observers: &OT,
        _testcase_id: &TestcaseId,
    ) -> Result<()> {
        Ok(())
    }
}

/// Has an associated observer name (mostly used to retrieve the observer with `MatchName` from an `ObserverTuple`)
pub trait HasObserverHandle {
    /// The observer for which we hold a reference
    type Observer: ?Sized;

    /// The name associated with the observer
    fn observer_handle(&self) -> &Handle<Self::Observer>;
}

/// A combined feedback consisting of multiple [`Feedback`]s
#[derive(Debug)]
pub struct CombinedFeedback<A, B, FL> {
    /// First [`Feedback`]
    pub first: A,
    /// Second [`Feedback`]
    pub second: B,
    name: Cow<'static, str>,
    phantom: PhantomData<FL>,
}

impl<A, B, FL> Named for CombinedFeedback<A, B, FL> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<A, B, FL> CombinedFeedback<A, B, FL>
where
    A: Named,
    B: Named,
    FL: FeedbackLogic,
{
    /// Create a new combined feedback
    pub fn new(first: A, second: B) -> Self {
        let name = Cow::from(format!(
            "{} ({},{})",
            FL::name(),
            first.name(),
            second.name()
        ));
        Self {
            first,
            second,
            name,
            phantom: PhantomData,
        }
    }
}

impl<A, B, FL> DependencyResolver for CombinedFeedback<A, B, FL>
where
    A: DependencyResolver,
    B: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.first.register(registrator)?;
        self.second.register(registrator)
    }
}

impl<A, B, FL, I, OT, S> Feedback<I, OT, S> for CombinedFeedback<A, B, FL>
where
    A: Feedback<I, OT, S>,
    B: Feedback<I, OT, S>,
    FL: FeedbackLogic,
{
    fn is_interesting(
        &mut self,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool> {
        FL::is_pair_interesting(
            |state, input, observers, exit_kind| {
                self.first
                    .is_interesting(state, input, observers, exit_kind)
            },
            |state, input, observers, exit_kind| {
                self.second
                    .is_interesting(state, input, observers, exit_kind)
            },
            state,
            input,
            observers,
            exit_kind,
        )
    }

    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.first.append_metadata(state, observers, testcase_id)?;
        self.second.append_metadata(state, observers, testcase_id)
    }
}

impl<A, B, FL, T> FeedbackFactory<CombinedFeedback<A, B, FL>, T> for CombinedFeedback<A, B, FL>
where
    A: FeedbackFactory<A, T> + Named,
    B: FeedbackFactory<B, T> + Named,
    FL: FeedbackLogic,
{
    fn create_feedback(&self, ctx: &T) -> CombinedFeedback<A, B, FL> {
        CombinedFeedback::new(
            self.first.create_feedback(ctx),
            self.second.create_feedback(ctx),
        )
    }
}

/// Logical combination of two feedbacks
pub trait FeedbackLogic {
    /// The name of this combination
    fn name() -> &'static str;

    /// If the feedback pair is interesting.
    ///
    /// `first` and `second` are closures which invoke the corresponding
    /// [`Feedback::is_interesting`] methods of the associated feedbacks. Implementors may choose to
    /// use the closure or not, depending on eagerness logic
    fn is_pair_interesting<I, OT, S, F1, F2>(
        first: F1,
        second: F2,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool>
    where
        F1: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
        F2: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>;
}

/// Factory for feedbacks which should be sensitive to an existing context, e.g. observer(s) from a
/// specific execution
pub trait FeedbackFactory<F, T> {
    /// Create the feedback from the provided context
    fn create_feedback(&self, ctx: &T) -> F;
}

impl<FE, FU, T> FeedbackFactory<FE, T> for FU
where
    FU: Fn(&T) -> FE,
{
    fn create_feedback(&self, ctx: &T) -> FE {
        self(ctx)
    }
}
/// Eager `OR` combination of two feedbacks
#[derive(Debug, Clone)]
pub struct LogicEagerOr;
/// Fast `OR` combination of two feedbacks
#[derive(Debug, Clone)]
pub struct LogicFastOr;

/// Eager `AND` combination of two feedbacks
#[derive(Debug, Clone)]
pub struct LogicEagerAnd;

/// Fast `AND` combination of two feedbacks
#[derive(Debug, Clone)]
pub struct LogicFastAnd;

impl FeedbackLogic for LogicEagerOr {
    fn name() -> &'static str {
        "Eager OR"
    }

    fn is_pair_interesting<I, OT, S, F1, F2>(
        first: F1,
        second: F2,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool>
    where
        F1: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
        F2: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
    {
        Ok(
            first(state, input, observers, exit_kind)?
                | second(state, input, observers, exit_kind)?,
        )
    }
}

impl FeedbackLogic for LogicFastOr {
    fn name() -> &'static str {
        "Fast OR"
    }

    fn is_pair_interesting<I, OT, S, F1, F2>(
        first: F1,
        second: F2,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool>
    where
        F1: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
        F2: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
    {
        let a = first(state, input, observers, exit_kind)?;
        if a {
            return Ok(true);
        }

        second(state, input, observers, exit_kind)
    }
}

impl FeedbackLogic for LogicEagerAnd {
    fn name() -> &'static str {
        "Eager AND"
    }

    fn is_pair_interesting<I, OT, S, F1, F2>(
        first: F1,
        second: F2,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool>
    where
        F1: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
        F2: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
    {
        Ok(
            first(state, input, observers, exit_kind)?
                & second(state, input, observers, exit_kind)?,
        )
    }
}

impl FeedbackLogic for LogicFastAnd {
    fn name() -> &'static str {
        "Fast AND"
    }

    fn is_pair_interesting<I, OT, S, F1, F2>(
        first: F1,
        second: F2,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool>
    where
        F1: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
        F2: FnOnce(&mut S, &I, &OT, &ExitKind) -> Result<bool>,
    {
        Ok(first(state, input, observers, exit_kind)?
            && second(state, input, observers, exit_kind)?)
    }
}

/// Combine two feedbacks with an eager AND operation,
/// will call all feedbacks functions even if not necessary to conclude the result
pub type EagerAndFeedback<A, B> = CombinedFeedback<A, B, LogicEagerAnd>;

/// Combine two feedbacks with an fast AND operation,
/// might skip calling feedbacks functions if not necessary to conclude the result
pub type FastAndFeedback<A, B> = CombinedFeedback<A, B, LogicFastAnd>;

/// Combine two feedbacks with an eager OR operation,
/// will call all feedbacks functions even if not necessary to conclude the result
pub type EagerOrFeedback<A, B> = CombinedFeedback<A, B, LogicEagerOr>;

/// Combine two feedbacks with an fast OR operation - fast.
///
/// This might skip calling feedbacks functions if not necessary to conclude the result.
/// This means any feedback that is not first might be skipped, use caution when using with
/// `TimeFeedback`
pub type FastOrFeedback<A, B> = CombinedFeedback<A, B, LogicFastOr>;

/// Compose feedbacks with an `NOT` operation
#[derive(Debug, Clone)]
pub struct NotFeedback<A> {
    /// The feedback to invert
    pub inner: A,
    /// The name
    name: Cow<'static, str>,
}

impl<A> DependencyResolver for NotFeedback<A>
where
    A: DependencyResolver,
{
    fn register(&mut self, registrator: &mut Registrator) -> Result<()> {
        registrator.register_ty::<Self>();
        self.register_md(registrator)?;

        self.inner.register(registrator)?;
        Ok(())
    }
}

impl<A, I, OT, S> Feedback<I, OT, S> for NotFeedback<A>
where
    A: Feedback<I, OT, S>,
{
    fn is_interesting(
        &mut self,
        state: &mut S,
        input: &I,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool> {
        Ok(!self
            .inner
            .is_interesting(state, input, observers, exit_kind)?)
    }

    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        self.inner.append_metadata(state, observers, testcase_id)
    }
}

impl<A> Named for NotFeedback<A> {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<A, T> FeedbackFactory<NotFeedback<A>, T> for NotFeedback<A>
where
    A: Named + FeedbackFactory<A, T>,
{
    fn create_feedback(&self, ctx: &T) -> NotFeedback<A> {
        NotFeedback::new(self.inner.create_feedback(ctx))
    }
}

impl<A> NotFeedback<A>
where
    A: Named,
{
    /// Creates a new [`NotFeedback`].
    pub fn new(inner: A) -> Self {
        let name = Cow::from(format!("Not({})", inner.name()));
        Self { inner, name }
    }
}

/// Variadic macro to create a chain of [`AndFeedback`](EagerAndFeedback)
#[macro_export]
macro_rules! feedback_and {
    ( $last:expr ) => { $last };

    ( $last:expr, ) => { $last };

    ( $head:expr, $($tail:expr),+ $(,)?) => {
        // recursive call
        $crate::feedbacks::EagerAndFeedback::new($head , feedback_and!($($tail),+))
    };
}
///
/// Variadic macro to create a chain of (fast) [`AndFeedback`](FastAndFeedback)
#[macro_export]
macro_rules! feedback_and_fast {
    ( $last:expr ) => { $last };

    ( $last:expr, ) => { $last };

    ( $head:expr, $($tail:expr),+ $(,)?) => {
        // recursive call
        $crate::feedbacks::FastAndFeedback::new($head , feedback_and_fast!($($tail),+))
    };
}

/// Variadic macro to create a chain of [`OrFeedback`](EagerOrFeedback)
#[macro_export]
macro_rules! feedback_or {
    ( $last:expr ) => { $last };

    ( $last:expr, ) => { $last };

    ( $head:expr, $($tail:expr),+ $(,)?) => {
        // recursive call
        $crate::feedbacks::EagerOrFeedback::new($head , feedback_or!($($tail),+))
    };
}

/// Combines multiple feedbacks with an `OR` operation, not executing feedbacks after the first positive result
#[macro_export]
macro_rules! feedback_or_fast {
    ( $last:expr ) => { $last };

    ( $last:expr, ) => { $last };

    ( $head:expr, $($tail:expr),+ $(,)?) => {
        // recursive call
        $crate::feedbacks::FastOrFeedback::new($head , feedback_or_fast!($($tail),+))
    };
}

/// Variadic macro to create a [`NotFeedback`]
#[macro_export]
macro_rules! feedback_not {
    ($last:expr) => {
        $crate::feedbacks::NotFeedback::new($last)
    };
}

/// Logic for measuring whether a given [`ExitKind`] is interesting as a [`Feedback`]. Use with
/// [`ExitKindFeedback`].
pub trait ExitKindLogic {
    /// The name of this kind of logic
    const NAME: Cow<'static, str>;

    /// Check whether the provided [`ExitKind`] is actually interesting
    fn check_exit_kind(kind: &ExitKind) -> Result<bool>;
}
/// Name used by `CrashFeedback`
pub const CRASH_FEEDBACK_NAME: &str = "CrashFeedback";
/// Logic which finds all [`ExitKind::Crash`] exits interesting
#[derive(Debug, Copy, Clone)]
pub struct CrashLogic;

impl ExitKindLogic for CrashLogic {
    const NAME: Cow<'static, str> = Cow::Borrowed(CRASH_FEEDBACK_NAME);

    fn check_exit_kind(kind: &ExitKind) -> Result<bool> {
        Ok(matches!(kind, ExitKind::Crash))
    }
}
/// Name used by `TimeoutFeedback`
pub const TIMEOUT_FEEDBACK_NAME: &str = "TimeoutFeedback";

/// Logic which finds all [`ExitKind::Timeout`] exits interesting
#[derive(Debug, Copy, Clone)]
pub struct TimeoutLogic;

impl ExitKindLogic for TimeoutLogic {
    const NAME: Cow<'static, str> = Cow::Borrowed(TIMEOUT_FEEDBACK_NAME);

    fn check_exit_kind(kind: &ExitKind) -> Result<bool> {
        Ok(matches!(kind, ExitKind::Timeout))
    }
}

/// A generic exit type checking feedback.
/// Use [`CrashFeedback`] or [`TimeoutFeedback`] directly instead.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExitKindFeedback<L> {
    name: Cow<'static, str>,
    phantom: PhantomData<fn() -> L>,
}

impl<L> DependencyResolver for ExitKindFeedback<L> where L: ExitKindLogic {}

impl<I, L, OT, S> Feedback<I, OT, S> for ExitKindFeedback<L>
where
    L: ExitKindLogic,
{
    fn is_interesting(
        &mut self,
        _state: &mut S,
        _input: &I,
        _observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool> {
        let res = L::check_exit_kind(exit_kind)?;
        Ok(res)
    }
}

impl<L> Named for ExitKindFeedback<L> {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<L> ExitKindFeedback<L>
where
    L: ExitKindLogic,
{
    /// Creates a new [`ExitKindFeedback`]
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: L::NAME,
            phantom: PhantomData,
        }
    }
}

impl<L> Default for ExitKindFeedback<L>
where
    L: ExitKindLogic,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<L, T> FeedbackFactory<ExitKindFeedback<L>, T> for ExitKindFeedback<L>
where
    L: ExitKindLogic,
{
    fn create_feedback(&self, _ctx: &T) -> ExitKindFeedback<L> {
        Self::new()
    }
}

/// A [`CrashFeedback`] reports as interesting if the target crashed.
pub type CrashFeedback = ExitKindFeedback<CrashLogic>;
/// A [`TimeoutFeedback`] reduces the timeout value of a run.
pub type TimeoutFeedback = ExitKindFeedback<TimeoutLogic>;

/// A [`Feedback`] to track execution time.
///
/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimeFeedback {
    observer_handle: Handle<TimeObserver>,
}
impl DependencyResolver for TimeFeedback {}

impl<I, OT, S> Feedback<I, OT, S> for TimeFeedback
where
    OT: MatchName,
    S: State<Input = I>,
{
    /// Append to the testcase the generated metadata in case of a new corpus item
    #[inline]
    fn append_metadata(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase_id: &TestcaseId,
    ) -> Result<()> {
        let Some(observer) = observers.get(&self.observer_handle) else {
            return Err(Error::illegal_state(
                "Observer referenced by TimeFeedback is not found in observers given to the fuzzer",
            ));
        };

        *state.testcase_md_mut_from_id(testcase_id).exec_time_mut() = *observer.last_runtime();

        Ok(())
    }
}

impl Named for TimeFeedback {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        self.observer_handle.name()
    }
}

impl TimeFeedback {
    /// Creates a new [`TimeFeedback`], deciding if the given [`TimeObserver`] value of a run is interesting.
    #[must_use]
    pub fn new(observer: &TimeObserver) -> Self {
        Self {
            observer_handle: observer.handle(),
        }
    }
}

/// The [`ConstFeedback`] reports the same value, always.
/// It can be used to enable or disable feedback results through composition.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConstFeedback {
    /// Always returns `true`
    True,
    /// Always returns `false`
    False,
}

impl DependencyResolver for ConstFeedback {}

impl<I, OT, S> Feedback<I, OT, S> for ConstFeedback {
    #[inline]
    fn is_interesting(
        &mut self,
        _state: &mut S,
        _input: &I,
        _observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool> {
        Ok((*self).into())
    }
}

impl Named for ConstFeedback {
    #[inline]
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("ConstFeedback");
        &NAME
    }
}

impl ConstFeedback {
    /// Creates a new [`ConstFeedback`] from the given boolean
    #[must_use]
    pub fn new(val: bool) -> Self {
        Self::from(val)
    }
}

impl From<bool> for ConstFeedback {
    fn from(val: bool) -> Self {
        if val { Self::True } else { Self::False }
    }
}

impl From<ConstFeedback> for bool {
    fn from(value: ConstFeedback) -> Self {
        match value {
            ConstFeedback::True => true,
            ConstFeedback::False => false,
        }
    }
}

impl<T> FeedbackFactory<ConstFeedback, T> for ConstFeedback {
    fn create_feedback(&self, _ctx: &T) -> ConstFeedback {
        *self
    }
}
