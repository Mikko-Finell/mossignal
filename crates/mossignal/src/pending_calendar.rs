//! Ordered semantic calendar with owner-maintained, provenance-free pulse totals.
use crate::key::NodeKey;
use crate::machine::{PendingEvent, PendingInertialDelay, PendingPeriodicBoundary};
use crate::time::Time;
use crate::transaction::CauseRef;
use std::collections::BTreeMap;
use std::ops::Deref;

#[derive(Clone, Default)]
struct DeadlineTotals {
    count: u128,
    groups: usize,
}
struct PulseTotals<D> {
    count: u128,
    deadlines: BTreeMap<Time<D>, DeadlineTotals>,
}
impl<D> Clone for PulseTotals<D> {
    fn clone(&self) -> Self {
        Self {
            count: self.count,
            deadlines: self.deadlines.clone(),
        }
    }
}
/// All ordinary calendar mutations pass through this owner. Read-only map
/// access preserves existing complete inspections/digests; no mutable map escapes.
pub(crate) struct PendingCalendar<D> {
    events: BTreeMap<Time<D>, Vec<PendingEvent<D>>>,
    pulses: BTreeMap<NodeKey, PulseTotals<D>>,
}
impl<D> Default for PendingCalendar<D> {
    fn default() -> Self {
        Self {
            events: BTreeMap::new(),
            pulses: BTreeMap::new(),
        }
    }
}
impl<D> Clone for PendingCalendar<D> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            pulses: self.pulses.clone(),
        }
    }
}
impl<D> Deref for PendingCalendar<D> {
    type Target = BTreeMap<Time<D>, Vec<PendingEvent<D>>>;
    fn deref(&self) -> &Self::Target {
        &self.events
    }
}
impl<'a, D> IntoIterator for &'a PendingCalendar<D> {
    type Item = (&'a Time<D>, &'a Vec<PendingEvent<D>>);
    type IntoIter = std::collections::btree_map::Iter<'a, Time<D>, Vec<PendingEvent<D>>>;
    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}
impl<D> PendingCalendar<D> {
    /// Rebuild only at explicit structural migration/restoration boundaries.
    pub(crate) fn from_events(events: BTreeMap<Time<D>, Vec<PendingEvent<D>>>) -> Self {
        let mut result = Self::default();
        for event in events.values().flatten() {
            result.add_pulse(*event);
        }
        result.events = events;
        result
    }
    fn add_pulse(&mut self, event: PendingEvent<D>) {
        if let PendingEvent::PulseDelay(event) = event {
            let totals = self
                .pulses
                .entry(event.node)
                .or_insert_with(|| PulseTotals {
                    count: 0,
                    deadlines: BTreeMap::new(),
                });
            // At most u64::MAX admitted groups each carry at most u64::MAX.
            // Wider indexing avoids making u64 summary exhaustion an admission law.
            let count = u128::from(event.count.get());
            totals.count = totals.count.checked_add(count).unwrap_or_else(|| {
                panic!(
                    "bounded calendar PulseDelay {:?} total exceeded u128",
                    event.node
                )
            });
            let deadline = totals.deadlines.entry(event.deadline).or_default();
            deadline.count = deadline.count.checked_add(count).unwrap_or_else(|| {
                panic!(
                    "bounded calendar PulseDelay {:?} deadline total exceeded u128",
                    event.node
                )
            });
            deadline.groups = deadline.groups.checked_add(1).unwrap_or_else(|| {
                panic!(
                    "bounded calendar PulseDelay {:?} deadline groups exceeded usize",
                    event.node
                )
            });
        }
    }
    pub(crate) fn push(&mut self, event: PendingEvent<D>) {
        self.add_pulse(event);
        self.events
            .entry(event.identity().3)
            .or_default()
            .push(event);
    }
    pub(crate) fn remove(&mut self, deadline: &Time<D>) -> Option<Vec<PendingEvent<D>>> {
        let batch = self.events.remove(deadline)?;
        for event in &batch {
            if let PendingEvent::PulseDelay(event) = event {
                let Some(totals) = self.pulses.get_mut(&event.node) else {
                    panic!(
                        "calendar PulseDelay {:?} lost its indexed total",
                        event.node
                    );
                };
                let count = u128::from(event.count.get());
                totals.count = totals.count.checked_sub(count).unwrap_or_else(|| {
                    panic!(
                        "calendar PulseDelay {:?} indexed total underflow",
                        event.node
                    )
                });
                let Some(remaining) = totals.deadlines.get_mut(deadline) else {
                    panic!("calendar PulseDelay {:?} lost indexed deadline", event.node);
                };
                remaining.count = remaining.count.checked_sub(count).unwrap_or_else(|| {
                    panic!(
                        "calendar PulseDelay {:?} indexed deadline underflow",
                        event.node
                    )
                });
                remaining.groups = remaining.groups.checked_sub(1).unwrap_or_else(|| {
                    panic!(
                        "calendar PulseDelay {:?} indexed deadline groups underflow",
                        event.node
                    )
                });
                if remaining.groups == 0 {
                    assert_eq!(
                        remaining.count, 0,
                        "calendar PulseDelay {:?} exhausted groups with remaining pulses",
                        event.node
                    );
                    totals.deadlines.remove(deadline);
                }
                if totals.deadlines.is_empty() {
                    assert_eq!(
                        totals.count, 0,
                        "calendar PulseDelay {:?} exhausted deadlines with remaining pulses",
                        event.node
                    );
                    self.pulses.remove(&event.node);
                }
            }
        }
        Some(batch)
    }
    pub(crate) fn pulse_totals(&self, node: NodeKey) -> (u128, Option<Time<D>>) {
        #[cfg(test)]
        crate::execution_work::update(|work| work.pulse_summary_lookups += 1);
        self.pulses.get(&node).map_or((0, None), |totals| {
            (
                totals.count,
                totals.deadlines.first_key_value().map(|(at, _)| *at),
            )
        })
    }
    fn take_nonpulse(
        &mut self,
        matches: impl Fn(PendingEvent<D>) -> bool,
    ) -> Option<PendingEvent<D>> {
        let found = self.events.iter().find_map(|(at, events)| {
            events
                .iter()
                .position(|event| matches(*event))
                .map(|index| (*at, index))
        })?;
        let Some(batch) = self.events.get_mut(&found.0) else {
            panic!("selected calendar deadline must exist");
        };
        let event = batch.remove(found.1);
        assert!(
            !matches!(event, PendingEvent::PulseDelay(_)),
            "nonpulse calendar extraction must preserve pulse totals"
        );
        if batch.is_empty() {
            self.events.remove(&found.0);
        }
        Some(event)
    }
    pub(crate) fn take_inertial(&mut self, node: NodeKey) -> Option<PendingInertialDelay<D>> {
        match self
            .take_nonpulse(|event| matches!(event, PendingEvent::Inertial(e) if e.node == node))?
        {
            PendingEvent::Inertial(event) => Some(event),
            _ => panic!("inertial calendar extraction must return an inertial candidate"),
        }
    }
    pub(crate) fn take_periodic(&mut self, node: NodeKey) -> Option<PendingPeriodicBoundary<D>> {
        match self
            .take_nonpulse(|event| matches!(event, PendingEvent::Periodic(e) if e.node == node))?
        {
            PendingEvent::Periodic(event) => Some(event),
            _ => panic!("periodic calendar extraction must return a periodic boundary"),
        }
    }
    /// Provenance replacement may rewrite causes, never index-determining facts.
    pub(crate) fn remap_causes(&mut self, mut remap: impl FnMut(PendingEvent<D>) -> CauseRef) {
        for event in self.events.values_mut().flatten() {
            let cause = remap(*event);
            match event {
                PendingEvent::PulseDelay(e) => e.cause = cause,
                PendingEvent::TransportDelay(e) => e.cause = cause,
                PendingEvent::Inertial(e) => e.cause = cause,
                PendingEvent::Periodic(e) => e.cause = cause,
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn reverse_batches(&mut self) {
        for batch in self.events.values_mut() {
            batch.reverse();
        }
    }
}

#[cfg(test)]
mod tests;
