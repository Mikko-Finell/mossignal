//! Exact logical-time values for caller-defined discrete time domains.

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::num::NonZeroU64;

macro_rules! impl_tick_value_traits {
    ($type:ident) => {
        impl<D> Clone for $type<D> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<D> Copy for $type<D> {}

        impl<D> PartialEq for $type<D> {
            fn eq(&self, other: &Self) -> bool {
                self.ticks == other.ticks
            }
        }

        impl<D> Eq for $type<D> {}

        impl<D> PartialOrd for $type<D> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<D> Ord for $type<D> {
            fn cmp(&self, other: &Self) -> Ordering {
                self.ticks.cmp(&other.ticks)
            }
        }

        impl<D> Hash for $type<D> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.ticks.hash(state);
            }
        }

        impl<D> fmt::Debug for $type<D> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($type))
                    .field("ticks", &self.ticks)
                    .finish()
            }
        }
    };
}

/// An exact discrete logical instant in the caller-defined time domain `D`.
///
/// The caller defines what one tick means. Different marker types create
/// statically incompatible domains. Tick zero is a valid caller-selected
/// instant, not an absent or uninitialized value, and time need not begin
/// there. Arithmetic is checked and never wraps or saturates.
///
/// Distinct domains cannot be mixed:
///
/// ```compile_fail
/// use mossignal::time::{Span, Time};
///
/// struct GameTicks;
/// struct AudioTicks;
///
/// let time = Time::<GameTicks>::from_ticks(5);
/// let span = Span::<AudioTicks>::from_ticks(2);
/// let _ = time.checked_add(span);
/// ```
///
/// Potentially failing arithmetic has no infallible operator form:
///
/// ```compile_fail
/// use mossignal::time::{Span, Time};
///
/// struct Ticks;
/// let _ = Time::<Ticks>::from_ticks(1) + Span::<Ticks>::from_ticks(1);
/// ```
///
/// ```compile_fail
/// use mossignal::time::Time;
///
/// struct Ticks;
/// let _ = Time::<Ticks>::from_ticks(2) - Time::<Ticks>::from_ticks(1);
/// ```
#[repr(transparent)]
pub struct Time<D> {
    ticks: u64,
    marker: PhantomData<fn() -> D>,
}

impl_tick_value_traits!(Time);

impl<D> Time<D> {
    /// Creates an instant from any tick value in the complete `u64` domain.
    #[must_use]
    pub const fn from_ticks(ticks: u64) -> Self {
        Self {
            ticks,
            marker: PhantomData,
        }
    }

    /// Returns the exact represented tick value.
    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.ticks
    }

    /// Adds a non-negative duration, rejecting an unrepresentable sum.
    pub fn checked_add(self, span: Span<D>) -> Result<Self, TimeArithmeticError> {
        match self.ticks.checked_add(span.ticks) {
            Some(ticks) => Ok(Self::from_ticks(ticks)),
            None => Err(TimeArithmeticError::overflow(
                TimeArithmeticOperation::TimeAddition,
                self.ticks,
                span.ticks,
            )),
        }
    }

    /// Adds a positive duration, rejecting an unrepresentable sum.
    pub fn checked_add_nonzero(self, span: NonZeroSpan<D>) -> Result<Self, TimeArithmeticError> {
        match self.ticks.checked_add(span.ticks()) {
            Some(ticks) => Ok(Self::from_ticks(ticks)),
            None => Err(TimeArithmeticError::overflow(
                TimeArithmeticOperation::NonZeroTimeAddition,
                self.ticks,
                span.ticks(),
            )),
        }
    }

    /// Returns the exact duration since `earlier`.
    ///
    /// A reverse subtraction is invalid rather than wrapping or producing an
    /// absolute duration.
    pub fn checked_duration_since(self, earlier: Self) -> Result<Span<D>, TimeArithmeticError> {
        match self.ticks.checked_sub(earlier.ticks) {
            Some(ticks) => Ok(Span::from_ticks(ticks)),
            None => Err(TimeArithmeticError::invalid_subtraction(
                self.ticks,
                earlier.ticks,
            )),
        }
    }
}

/// A non-negative integral tick count in the caller-defined time domain `D`.
///
/// The caller defines what one tick means. Arithmetic is checked and never
/// wraps or saturates.
///
/// ```compile_fail
/// use mossignal::time::Span;
///
/// struct Ticks;
/// let _ = Span::<Ticks>::from_ticks(1) + Span::<Ticks>::from_ticks(1);
/// ```
#[repr(transparent)]
pub struct Span<D> {
    ticks: u64,
    marker: PhantomData<fn() -> D>,
}

impl_tick_value_traits!(Span);

impl<D> Span<D> {
    /// A duration containing no ticks.
    pub const ZERO: Self = Self::from_ticks(0);

    /// Creates a span from any tick value in the complete `u64` domain.
    #[must_use]
    pub const fn from_ticks(ticks: u64) -> Self {
        Self {
            ticks,
            marker: PhantomData,
        }
    }

    /// Returns the exact represented tick count.
    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.ticks
    }

    /// Returns whether this duration contains no ticks.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.ticks == 0
    }

    /// Adds two spans, rejecting an unrepresentable sum.
    pub fn checked_add(self, other: Self) -> Result<Self, TimeArithmeticError> {
        match self.ticks.checked_add(other.ticks) {
            Some(ticks) => Ok(Self::from_ticks(ticks)),
            None => Err(TimeArithmeticError::overflow(
                TimeArithmeticOperation::SpanAddition,
                self.ticks,
                other.ticks,
            )),
        }
    }

    /// Converts this duration to a positive duration, rejecting zero.
    pub fn try_nonzero(self) -> Result<NonZeroSpan<D>, ZeroSpanError> {
        NonZeroSpan::from_ticks(self.ticks)
    }
}

/// A positive integral tick count in the caller-defined time domain `D`.
///
/// The caller defines what one tick means. Construction rejects zero, so every
/// value is positive by construction and remains statically tied to `D`.
#[repr(transparent)]
pub struct NonZeroSpan<D> {
    ticks: NonZeroU64,
    marker: PhantomData<fn() -> D>,
}

impl_tick_value_traits!(NonZeroSpan);

impl<D> NonZeroSpan<D> {
    /// Creates a positive duration, rejecting a tick count of zero.
    pub fn from_ticks(ticks: u64) -> Result<Self, ZeroSpanError> {
        match NonZeroU64::new(ticks) {
            Some(ticks) => Ok(Self {
                ticks,
                marker: PhantomData,
            }),
            None => Err(ZeroSpanError::new()),
        }
    }

    /// Returns the exact positive tick count.
    #[must_use]
    pub const fn ticks(self) -> u64 {
        self.ticks.get()
    }

    /// Returns the underlying nonzero integer.
    #[must_use]
    pub const fn get(self) -> NonZeroU64 {
        self.ticks
    }

    /// Returns this positive duration as a non-negative span in the same domain.
    #[must_use]
    pub const fn as_span(self) -> Span<D> {
        Span::from_ticks(self.ticks.get())
    }
}

/// The structured classification of a checked logical-time arithmetic failure.
///
/// Callers can inspect this kind instead of parsing the error's display text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeArithmeticErrorKind {
    /// An exact arithmetic result exceeded the representable `u64` range.
    Overflow,
    /// A duration was requested with the later and earlier instants reversed.
    InvalidSubtraction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeArithmeticOperation {
    TimeAddition,
    NonZeroTimeAddition,
    SpanAddition,
    DurationSubtraction,
}

/// An opaque error from checked logical-time arithmetic.
///
/// The operation and operands are retained privately for future structured
/// diagnostic mapping. Use [`TimeArithmeticError::kind`] to distinguish
/// overflow from invalid reverse subtraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeArithmeticError {
    kind: TimeArithmeticErrorKind,
    operation: TimeArithmeticOperation,
    left: u64,
    right: u64,
}

impl TimeArithmeticError {
    const fn overflow(operation: TimeArithmeticOperation, left: u64, right: u64) -> Self {
        Self {
            kind: TimeArithmeticErrorKind::Overflow,
            operation,
            left,
            right,
        }
    }

    const fn invalid_subtraction(left: u64, right: u64) -> Self {
        Self {
            kind: TimeArithmeticErrorKind::InvalidSubtraction,
            operation: TimeArithmeticOperation::DurationSubtraction,
            left,
            right,
        }
    }

    /// Returns the structured failure classification.
    #[must_use]
    pub const fn kind(self) -> TimeArithmeticErrorKind {
        self.kind
    }
}

impl fmt::Display for TimeArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TimeArithmeticErrorKind::Overflow => formatter
                .write_str("logical-time arithmetic overflowed the representable u64 range"),
            TimeArithmeticErrorKind::InvalidSubtraction => formatter.write_str(
                "cannot calculate a duration from a later instant to an earlier instant",
            ),
        }
    }
}

impl std::error::Error for TimeArithmeticError {}

/// An opaque error indicating that a positive duration was given zero ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroSpanError {
    private: (),
}

impl ZeroSpanError {
    const fn new() -> Self {
        Self { private: () }
    }
}

impl fmt::Display for ZeroSpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a positive duration cannot contain zero ticks")
    }
}

impl std::error::Error for ZeroSpanError {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hash::Hash;
    use core::mem::{align_of, size_of};
    use std::collections::hash_map::DefaultHasher;

    struct UncooperativeDomain;

    const TIME_ZERO: Time<UncooperativeDomain> = Time::from_ticks(0);
    const TIME_ONE: Time<UncooperativeDomain> = Time::from_ticks(1);
    const TIME_MAX: Time<UncooperativeDomain> = Time::from_ticks(u64::MAX);
    const SPAN_ZERO: Span<UncooperativeDomain> = Span::ZERO;
    const SPAN_ONE: Span<UncooperativeDomain> = Span::from_ticks(1);
    const SPAN_MAX: Span<UncooperativeDomain> = Span::from_ticks(u64::MAX);
    const NONZERO_ONE: NonZeroSpan<UncooperativeDomain> = NonZeroSpan {
        ticks: NonZeroU64::MIN,
        marker: PhantomData,
    };
    const NONZERO_ONE_TICKS: u64 = NONZERO_ONE.ticks();
    const NONZERO_ONE_GET: NonZeroU64 = NONZERO_ONE.get();
    const NONZERO_ONE_SPAN: Span<UncooperativeDomain> = NONZERO_ONE.as_span();

    fn clone_value<T: Clone>(value: &T) -> T {
        value.clone()
    }

    fn hash_value<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn assert_value_traits<
        T: Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Hash + fmt::Debug,
    >(
        value: T,
    ) {
        let copied = value;
        let cloned = clone_value(&value);
        assert_eq!(copied, value);
        assert_eq!(cloned, value);
        assert_eq!(value.cmp(&copied), Ordering::Equal);
        assert_eq!(hash_value(&value), hash_value(&copied));
        assert!(!format!("{value:?}").is_empty());
    }

    #[test]
    fn representations_are_transparent_and_traits_ignore_the_domain_marker() {
        assert_eq!(size_of::<Time<UncooperativeDomain>>(), size_of::<u64>());
        assert_eq!(align_of::<Time<UncooperativeDomain>>(), align_of::<u64>());
        assert_eq!(size_of::<Span<UncooperativeDomain>>(), size_of::<u64>());
        assert_eq!(align_of::<Span<UncooperativeDomain>>(), align_of::<u64>());
        assert_eq!(
            size_of::<NonZeroSpan<UncooperativeDomain>>(),
            size_of::<NonZeroU64>()
        );
        assert_eq!(
            align_of::<NonZeroSpan<UncooperativeDomain>>(),
            align_of::<NonZeroU64>()
        );

        assert_value_traits(TIME_ONE);
        assert_value_traits(SPAN_ONE);
        assert_value_traits(NONZERO_ONE);
        assert!(TIME_ZERO < TIME_ONE);
        assert!(SPAN_ZERO < SPAN_ONE);
        assert_eq!(NONZERO_ONE_TICKS, 1);
        assert_eq!(NONZERO_ONE_GET, NonZeroU64::MIN);
        assert_eq!(NONZERO_ONE_SPAN, SPAN_ONE);
    }

    #[test]
    fn time_construction_and_order_cover_the_complete_boundaries() {
        for ticks in [0, 1, 2, u64::MAX - 1, u64::MAX] {
            assert_eq!(
                Time::<UncooperativeDomain>::from_ticks(ticks).ticks(),
                ticks
            );
        }
        assert_eq!(TIME_ZERO.ticks(), 0);
        assert_eq!(TIME_ONE.ticks(), 1);
        assert_eq!(TIME_MAX.ticks(), u64::MAX);
        assert!(Time::<UncooperativeDomain>::from_ticks(2) < TIME_MAX);
    }

    #[test]
    fn time_addition_accepts_exact_sums_and_retains_overflow_evidence() {
        let maximum = TIME_MAX;
        let cases = [
            (0, 0, 0),
            (0, 1, 1),
            (1, 0, 1),
            (1, 1, 2),
            (u64::MAX - 1, 1, u64::MAX),
            (u64::MAX, 0, u64::MAX),
        ];
        for (time, span, expected) in cases {
            assert_eq!(
                Time::<UncooperativeDomain>::from_ticks(time).checked_add(Span::from_ticks(span)),
                Ok(Time::from_ticks(expected))
            );
        }

        for (time, span) in [(u64::MAX, 1), (u64::MAX - 1, 2), (u64::MAX, u64::MAX)] {
            let error = Time::<UncooperativeDomain>::from_ticks(time)
                .checked_add(Span::from_ticks(span))
                .expect_err("the sum must exceed u64");
            assert_eq!(error.kind(), TimeArithmeticErrorKind::Overflow);
            assert_eq!(error.operation, TimeArithmeticOperation::TimeAddition);
            assert_eq!((error.left, error.right), (time, span));
        }

        assert_eq!(maximum.checked_add(SPAN_ZERO), Ok(maximum));
    }

    #[test]
    fn nonzero_time_addition_accepts_boundaries_and_retains_overflow_evidence() {
        for (time, span, expected) in [(0, 1, 1), (1, 1, 2), (u64::MAX - 1, 1, u64::MAX)] {
            let span = NonZeroSpan::from_ticks(span).expect("the span is positive");
            assert_eq!(
                Time::<UncooperativeDomain>::from_ticks(time).checked_add_nonzero(span),
                Ok(Time::from_ticks(expected))
            );
        }

        for (time, span) in [(u64::MAX, 1), (u64::MAX - 1, 2), (u64::MAX, u64::MAX)] {
            let span_value = NonZeroSpan::from_ticks(span).expect("the span is positive");
            let error = Time::<UncooperativeDomain>::from_ticks(time)
                .checked_add_nonzero(span_value)
                .expect_err("the sum must exceed u64");
            assert_eq!(error.kind(), TimeArithmeticErrorKind::Overflow);
            assert_eq!(
                error.operation,
                TimeArithmeticOperation::NonZeroTimeAddition
            );
            assert_eq!((error.left, error.right), (time, span));
        }
    }

    #[test]
    fn duration_subtraction_is_directional_and_retains_operand_order() {
        for (later, earlier, expected) in [
            (0, 0, 0),
            (1, 0, 1),
            (2, 1, 1),
            (u64::MAX, 0, u64::MAX),
            (u64::MAX, u64::MAX, 0),
        ] {
            assert_eq!(
                Time::<UncooperativeDomain>::from_ticks(later)
                    .checked_duration_since(Time::from_ticks(earlier)),
                Ok(Span::from_ticks(expected))
            );
        }

        for (later, earlier) in [(0, 1), (1, 2), (0, u64::MAX)] {
            let error = Time::<UncooperativeDomain>::from_ticks(later)
                .checked_duration_since(Time::from_ticks(earlier))
                .expect_err("the earlier instant is later");
            assert_eq!(error.kind(), TimeArithmeticErrorKind::InvalidSubtraction);
            assert_eq!(
                error.operation,
                TimeArithmeticOperation::DurationSubtraction
            );
            assert_eq!((error.left, error.right), (later, earlier));
        }
    }

    #[test]
    fn span_construction_order_and_addition_cover_boundaries() {
        for ticks in [0, 1, 2, u64::MAX - 1, u64::MAX] {
            let span = Span::<UncooperativeDomain>::from_ticks(ticks);
            assert_eq!(span.ticks(), ticks);
            assert_eq!(span.is_zero(), ticks == 0);
        }
        assert_eq!(SPAN_ZERO.ticks(), 0);
        assert_eq!(SPAN_MAX.ticks(), u64::MAX);
        assert!(SPAN_ONE < Span::from_ticks(2));

        for (left, right, expected) in [
            (0, 0, 0),
            (0, u64::MAX, u64::MAX),
            (u64::MAX, 0, u64::MAX),
            (1, 1, 2),
            (u64::MAX - 1, 1, u64::MAX),
        ] {
            assert_eq!(
                Span::<UncooperativeDomain>::from_ticks(left).checked_add(Span::from_ticks(right)),
                Ok(Span::from_ticks(expected))
            );
        }

        for (left, right) in [(u64::MAX, 1), (1, u64::MAX), (u64::MAX, u64::MAX)] {
            let error = Span::<UncooperativeDomain>::from_ticks(left)
                .checked_add(Span::from_ticks(right))
                .expect_err("the span sum must exceed u64");
            assert_eq!(error.kind(), TimeArithmeticErrorKind::Overflow);
            assert_eq!(error.operation, TimeArithmeticOperation::SpanAddition);
            assert_eq!((error.left, error.right), (left, right));
        }
    }

    #[test]
    fn nonzero_span_construction_and_conversion_cover_boundaries() {
        let direct_zero =
            NonZeroSpan::<UncooperativeDomain>::from_ticks(0).expect_err("zero must be rejected");
        let converted_zero = SPAN_ZERO
            .try_nonzero()
            .expect_err("the zero span must be rejected");
        assert_eq!(direct_zero, converted_zero);

        for ticks in [1, 2, u64::MAX] {
            let nonzero = NonZeroSpan::<UncooperativeDomain>::from_ticks(ticks)
                .expect("the tick count is positive");
            assert_eq!(nonzero.ticks(), ticks);
            assert_eq!(nonzero.get().get(), ticks);
            assert_eq!(nonzero.as_span(), Span::from_ticks(ticks));
            assert_eq!(Span::from_ticks(ticks).try_nonzero(), Ok(nonzero));
        }
    }

    #[test]
    fn errors_are_copyable_standard_errors_with_useful_descriptions() {
        fn assert_error<T: std::error::Error + Clone + Copy + PartialEq + Eq>() {}
        assert_error::<TimeArithmeticError>();
        assert_error::<ZeroSpanError>();

        let overflow = TIME_MAX
            .checked_add(SPAN_ONE)
            .expect_err("maximum plus one must overflow");
        let invalid = TIME_ZERO
            .checked_duration_since(TIME_ONE)
            .expect_err("reverse subtraction must fail");
        assert_ne!(overflow, invalid);
        assert!(overflow.to_string().contains("overflow"));
        assert!(invalid.to_string().contains("duration"));

        let zero =
            NonZeroSpan::<UncooperativeDomain>::from_ticks(0).expect_err("zero must be rejected");
        assert!(zero.to_string().contains("zero"));
    }
}
