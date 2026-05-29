//! The [`BoolValueFeedback`] is a [`Feedback`] returning `true` or `false` as the `is_interesting` value.

use alloc::borrow::Cow;

use libaflmm_bolts::{
    Error, Named,
    tuples::{Handle, MatchNameRef},
};
use libaflmm_core::Result;

use crate::{
    common::DependencyResolver,
    corpus::TestcaseId,
    feedbacks::Feedback,
    observers::{ObserversTuple, ValueObserver},
};

/// This feedback returns `true` or `false` as the `is_interesting` value.
#[derive(Debug)]
pub struct BoolValueFeedback<'a> {
    name: Cow<'static, str>,
    observer_hnd: Handle<ValueObserver<'a, bool>>,
}

impl<'a> BoolValueFeedback<'a> {
    /// Create a new [`BoolValueFeedback`]
    #[must_use]
    pub fn new(observer_hnd: &Handle<ValueObserver<'a, bool>>) -> Self {
        Self::with_name(observer_hnd.name().clone(), observer_hnd)
    }

    /// Create a new [`BoolValueFeedback`] with a given name
    #[must_use]
    pub fn with_name(
        name: Cow<'static, str>,
        observer_hnd: &Handle<ValueObserver<'a, bool>>,
    ) -> Self {
        Self {
            name,
            observer_hnd: observer_hnd.clone(),
        }
    }
}

impl Named for BoolValueFeedback<'_> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl DependencyResolver for BoolValueFeedback<'_> {}

impl<I, OT, S> Feedback<I, OT, S> for BoolValueFeedback<'_>
where
    OT: ObserversTuple<S>,
{
    fn is_interesting(
        &mut self,
        _state: &mut S,
        _input: &I,
        observers: &OT,
        _exit_kind: &crate::executors::ExitKind,
    ) -> Result<bool> {
        let Some(observer) = observers.get(&self.observer_hnd) else {
            return Err(Error::illegal_state(format!(
                "Observer {:?} not found",
                self.observer_hnd
            )));
        };

        let val = *observer.value.as_ref();

        Ok(val)
    }

    fn append_metadata(
        &mut self,
        _state: &mut S,
        _observers: &OT,
        _testcase_id: &TestcaseId,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use core::{cell::UnsafeCell, ptr::write_volatile};

    use libaflmm_bolts::{ownedref::OwnedRef, tuples::Handled};
    use tuple_list::tuple_list;

    use crate::{
        executors::ExitKind,
        feedbacks::{BoolValueFeedback, Feedback},
        observers::ValueObserver,
        states::NopState,
    };

    #[test]
    fn test_bool_value_feedback() {
        let value: UnsafeCell<bool> = false.into();

        // # Safety
        // The value is only read from in the feedback, not while we change the value.
        let value_ptr = unsafe { OwnedRef::from_ptr(value.get()) };

        let observer = ValueObserver::new("test_value", value_ptr);

        let mut bool_feedback = BoolValueFeedback::new(&observer.handle());

        let mut state = NopState::nop().unwrap();
        // bool_feedback.init_state(&mut state).unwrap();

        let observers = tuple_list!(observer);
        let input = ();
        let exit_ok = ExitKind::Ok;

        let false_eval = bool_feedback
            .is_interesting(&mut state, &input, &observers, &exit_ok)
            .unwrap();
        assert!(!false_eval);

        // # Safety
        // The feedback is not keeping a borrow around, only the pointer.
        unsafe {
            write_volatile(value.get(), true);
        }

        let true_eval = bool_feedback
            .is_interesting(&mut state, &input, &observers, &exit_ok)
            .unwrap();
        assert!(true_eval);
    }
}
