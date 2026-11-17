use super::*;
use crate::{BoundedRestoreFailure, DecodePolicy, SnapshotLimits, SnapshotResourceFailure};
fn limits() -> SnapshotLimits {
    SnapshotLimits {
        records: 100_000,
        edges: 1_000_000,
        artifact_bytes: 64_000_000,
        allocation_bytes: 1_000_000_000,
    }
}
fn decode() -> DecodePolicy {
    DecodePolicy::new(
        64_000_000, 64, 64_000_000, 64_000_000, 1_000_000, 100_000, 100_000, 100_000, 100_000,
        100_000, 100_000, 1_000_000, 100_000, 0, 0,
    )
}
#[test]
fn bounded_export_rejects_each_limit_before_projection_and_moves_unchanged_bytes() {
    let machine = families();
    let before = encode(&machine).1;
    let (snapshot, allocation) = machine.snapshot_bounded(limits()).expect("admitted export");
    let pointer = snapshot.artifact_bytes().as_ptr();
    let bytes = snapshot
        .into_artifact(&PersistenceContext::new(
            machine.compiled().time_domain_id(),
        ))
        .expect("matching domain");
    assert_eq!(
        pointer,
        bytes.as_bytes().as_ptr(),
        "consuming encoding must not clone"
    );
    assert_eq!(bytes.as_bytes(), before);
    let owned = bytes.into_vec();
    assert_eq!(
        pointer,
        owned.as_ptr(),
        "host container transfer must not clone"
    );
    assert_eq!(owned, before);
    for (resource, limit) in [
        (
            "records",
            SnapshotLimits {
                records: 0,
                ..limits()
            },
        ),
        (
            "edges",
            SnapshotLimits {
                edges: 0,
                ..limits()
            },
        ),
        (
            "artifact_bytes",
            SnapshotLimits {
                artifact_bytes: 0,
                ..limits()
            },
        ),
        (
            "allocation_bytes",
            SnapshotLimits {
                allocation_bytes: allocation.bytes - 1,
                ..limits()
            },
        ),
    ] {
        crate::projection_work::reset();
        assert!(
            matches!(machine.snapshot_bounded(limit), Err(SnapshotResourceFailure::Limit { resource: actual, .. }) if actual == resource)
        );
        assert_eq!(
            crate::projection_work::read(),
            crate::projection_work::Work::default()
        );
    }
    assert_eq!(encode(&machine).1, before);
    let (snapshot, _) = machine
        .snapshot_bounded(SnapshotLimits {
            allocation_bytes: allocation.bytes,
            ..limits()
        })
        .expect("exact allocation boundary");
    assert!(matches!(
        snapshot.into_artifact(&PersistenceContext::new(TimeDomainId::from_u128(u128::MAX))),
        Err(EncodeFailure::TimeDomainMismatch)
    ));
}
#[test]
fn bounded_restore_keeps_all_families_episodes_and_future_output() {
    for mut original in [uninitialized_toggle(), families()] {
        let compiled = original.compiled().clone();
        let (snapshot, _) = original.snapshot_bounded(limits()).expect("export");
        let bytes = snapshot
            .into_artifact(&PersistenceContext::new(compiled.time_domain_id()))
            .expect("artifact");
        let (mut restored, allocation) = compiled
            .restore_snapshot_bounded(bytes.as_bytes(), &decode(), policy(), 1_000_000_000)
            .expect("bounded restoration");
        assert_eq!(encode(&restored).1, bytes.as_bytes());
        for (limit, resource) in [
            (0, "decode_allocation_bytes"),
            (allocation.bytes - 1, "restore_allocation_bytes"),
        ] {
            assert!(
                matches!(compiled.restore_snapshot_bounded(bytes.as_bytes(), &decode(), policy(), limit), Err(BoundedRestoreFailure::Resource(SnapshotResourceFailure::Limit { resource: actual, .. })) if actual == resource)
            );
        }
        compiled
            .restore_snapshot_bounded(bytes.as_bytes(), &decode(), policy(), allocation.bytes)
            .expect("exact admission allowance");
        if original.is_initialized() {
            let delta = compiled.input_delta().finish().expect("empty delta");
            let tx = Transaction::advance(Time::from_ticks(100), original.revision(), delta);
            let expected = original.apply(tx.clone()).expect("original continuation");
            let actual = restored.apply(tx).expect("restored continuation");
            let facts = |events: &[crate::OutputEvent<()>]| {
                events
                    .iter()
                    .map(|event| match event {
                        crate::OutputEvent::LevelEstablished {
                            output,
                            value,
                            stamp,
                            revision,
                            ..
                        } => (
                            0,
                            output.as_u128(),
                            None,
                            Some(*value),
                            0,
                            *stamp,
                            *revision,
                        ),
                        crate::OutputEvent::LevelChanged {
                            output,
                            from,
                            to,
                            stamp,
                            revision,
                            ..
                        } => (
                            1,
                            output.as_u128(),
                            Some(*from),
                            Some(*to),
                            0,
                            *stamp,
                            *revision,
                        ),
                        crate::OutputEvent::Pulsed {
                            output,
                            count,
                            stamp,
                            revision,
                            ..
                        } => (
                            2,
                            output.as_u128(),
                            None,
                            None,
                            count.get(),
                            *stamp,
                            *revision,
                        ),
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                facts(actual.output_events()),
                facts(expected.output_events())
            );
            assert_eq!(encode(&original).1, encode(&restored).1);
        }
    }
}
#[test]
fn bounded_restore_keeps_policy_topology_and_decode_rejections() {
    let original = families();
    let bytes = encode(&original).1;
    let wrong_policy = RuntimePolicy::builder()
        .max_internal_reactions(101)
        .max_evaluated_operations(10_000)
        .max_pending_events(100)
        .max_events_created_per_transaction(100)
        .max_required_provenance_growth(10_000)
        .build()
        .expect("policy");
    assert!(matches!(
        original.compiled().restore_snapshot_bounded(
            &bytes,
            &decode(),
            wrong_policy,
            1_000_000_000
        ),
        Err(BoundedRestoreFailure::Restore(failure)) if matches!(*failure, crate::RestoreFailure::RuntimePolicyMismatch(_))
    ));
    assert!(matches!(
        uninitialized_toggle().compiled().restore_snapshot_bounded(
            &bytes,
            &decode(),
            policy(),
            1_000_000_000
        ),
        Err(BoundedRestoreFailure::Restore(_))
    ));
    assert!(matches!(
        original.compiled().restore_snapshot_bounded(
            &bytes[..bytes.len() - 1],
            &decode(),
            policy(),
            1_000_000_000
        ),
        Err(BoundedRestoreFailure::Restore(failure)) if matches!(*failure, crate::RestoreFailure::Decode(_))
    ));
    assert_eq!(encode(&original).1, bytes);
}
