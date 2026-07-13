//! Fundamental types for Mossignal's closed signal-kind universe.

use core::fmt;
use core::ops::Not;

/// A type-level marker for persistent binary level signals.
///
/// This uninhabited type identifies a signal kind; it is not a signal value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {}

/// A type-level marker for reaction-scoped pulse signals.
///
/// This uninhabited type identifies a signal kind; it is not a signal value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pulse {}

/// The erased representation of Mossignal's closed signal kinds.
///
/// Only level and pulse signals belong to the core semantic universe.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SignalKind {
    /// A persistent binary level signal.
    Level,
    /// A reaction-scoped pulse signal.
    Pulse,
}

mod private {
    pub trait Sealed {}
}

/// Associates a core signal marker with its erased kind.
///
/// This trait is sealed: only [`Level`] and [`Pulse`] can implement it.
///
/// ```compile_fail
/// use mossignal::signal::{SignalKind, SignalType};
///
/// struct CustomSignal;
///
/// impl SignalType for CustomSignal {
///     const KIND: SignalKind = SignalKind::Level;
/// }
/// ```
pub trait SignalType: private::Sealed {
    /// The erased kind corresponding to this marker type.
    const KIND: SignalKind;
}

impl private::Sealed for Level {}

impl SignalType for Level {
    const KIND: SignalKind = SignalKind::Level;
}

impl private::Sealed for Pulse {}

impl SignalType for Pulse {
    const KIND: SignalKind = SignalKind::Pulse;
}

/// An established binary level signal value.
///
/// Both variants are real values: [`LogicLevel::Low`] is not absence or an
/// uninitialized state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LogicLevel {
    /// The established low level.
    Low,
    /// The established high level.
    High,
}

impl LogicLevel {
    /// Returns the opposite binary level.
    #[must_use]
    pub const fn invert(self) -> Self {
        match self {
            Self::Low => Self::High,
            Self::High => Self::Low,
        }
    }

    /// Returns whether this value is [`LogicLevel::Low`].
    #[must_use]
    pub const fn is_low(self) -> bool {
        matches!(self, Self::Low)
    }

    /// Returns whether this value is [`LogicLevel::High`].
    #[must_use]
    pub const fn is_high(self) -> bool {
        matches!(self, Self::High)
    }
}

impl Not for LogicLevel {
    type Output = LogicLevel;

    fn not(self) -> Self::Output {
        self.invert()
    }
}

/// A non-negative simultaneous pulse multiplicity.
///
/// Every `u64` value is valid, including zero. Addition is available only
/// through [`PulseCount::checked_add`] so that it cannot wrap or saturate.
///
/// ```compile_fail
/// use mossignal::signal::PulseCount;
///
/// let _ = PulseCount::ONE + PulseCount::ONE;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PulseCount(u64);

impl PulseCount {
    /// A count containing no pulse occurrences.
    pub const ZERO: Self = Self(0);

    /// A count containing one pulse occurrence.
    pub const ONE: Self = Self(1);

    /// Creates a pulse count from its complete `u64` representation domain.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the represented pulse multiplicity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns whether this count contains no pulse occurrences.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns whether this count contains at least one pulse occurrence.
    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Returns the exact sum, or an error if the mathematical sum exceeds
    /// the representable `u64` range.
    pub fn checked_add(self, other: Self) -> Result<Self, PulseCountOverflow> {
        match self.0.checked_add(other.0) {
            Some(sum) => Ok(Self(sum)),
            None => Err(PulseCountOverflow {
                left: self,
                right: other,
            }),
        }
    }
}

/// An error indicating that a pulse-count sum exceeded the `u64` range.
///
/// The failed operands are retained privately for future structured diagnostic
/// mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseCountOverflow {
    left: PulseCount,
    right: PulseCount,
}

impl fmt::Display for PulseCountOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("pulse count addition overflowed the representable u64 range")
    }
}

impl std::error::Error for PulseCountOverflow {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Debug;
    use core::hash::Hash;
    use core::mem::size_of;

    fn assert_marker_traits<T: Debug + Clone + Copy + PartialEq + Eq + Hash>() {}

    #[test]
    fn markers_are_zero_sized_and_have_their_erased_kinds() {
        assert_eq!(size_of::<Level>(), 0);
        assert_eq!(size_of::<Pulse>(), 0);
        assert_marker_traits::<Level>();
        assert_marker_traits::<Pulse>();
        assert_eq!(Level::KIND, SignalKind::Level);
        assert_eq!(Pulse::KIND, SignalKind::Pulse);
    }

    #[test]
    fn logic_level_laws_hold_over_the_complete_domain() {
        let cases = [
            (LogicLevel::Low, LogicLevel::High, true, false),
            (LogicLevel::High, LogicLevel::Low, false, true),
        ];

        for (value, inverse, is_low, is_high) in cases {
            assert_eq!(value.invert(), inverse);
            assert_eq!(!value, inverse);
            assert_eq!(value.is_low(), is_low);
            assert_eq!(value.is_high(), is_high);
            assert_eq!(value.invert().invert(), value);
        }

        assert!(LogicLevel::Low < LogicLevel::High);
    }

    #[test]
    fn pulse_count_values_cover_the_representation_boundaries() {
        assert_eq!(PulseCount::ZERO.get(), 0);
        assert_eq!(PulseCount::ONE.get(), 1);

        for value in [0, 1, 2, u64::MAX] {
            let count = PulseCount::new(value);
            assert_eq!(count.get(), value);
            assert_eq!(count.is_zero(), value == 0);
            assert_eq!(count.is_positive(), value > 0);
        }

        assert_eq!(PulseCount::new(1), PulseCount::ONE);
        assert!(PulseCount::new(1) < PulseCount::new(2));
        assert!(PulseCount::new(2) < PulseCount::new(u64::MAX));
    }

    #[test]
    fn checked_add_returns_every_required_representable_sum() {
        let maximum = PulseCount::new(u64::MAX);
        let cases = [
            (PulseCount::ZERO, PulseCount::ZERO, PulseCount::ZERO),
            (PulseCount::ZERO, maximum, maximum),
            (maximum, PulseCount::ZERO, maximum),
            (PulseCount::ONE, PulseCount::ONE, PulseCount::new(2)),
        ];

        for (left, right, expected) in cases {
            assert_eq!(left.checked_add(right), Ok(expected));
        }
    }

    #[test]
    fn checked_add_rejects_every_required_overflow_and_retains_operands() {
        let maximum = PulseCount::new(u64::MAX);
        let cases = [
            (maximum, PulseCount::ONE),
            (PulseCount::ONE, maximum),
            (maximum, maximum),
        ];

        for (left, right) in cases {
            let expected = PulseCountOverflow { left, right };
            assert_eq!(left.checked_add(right), Err(expected));
            assert_eq!(expected.left, left);
            assert_eq!(expected.right, right);
        }
    }

    #[test]
    fn overflow_error_is_a_standard_error_with_a_useful_description() {
        fn assert_error<T: std::error::Error + Clone + Copy + PartialEq + Eq>() {}

        assert_error::<PulseCountOverflow>();
        let error = PulseCount::new(u64::MAX)
            .checked_add(PulseCount::ONE)
            .expect_err("the maximum pulse count plus one must overflow");
        assert!(error.to_string().contains("pulse count"));
        assert!(error.to_string().contains("overflow"));
    }
}
