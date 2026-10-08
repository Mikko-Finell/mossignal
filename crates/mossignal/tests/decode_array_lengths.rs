use mossignal::{
    DecodePolicy, PersistenceContext, TimeDomainId, decode_replay_frame, decode_replay_log,
    decode_snapshot,
};

fn limits(collection_items: u64) -> DecodePolicy {
    DecodePolicy::new(
        100,
        16,
        100,
        100,
        collection_items,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        1,
        0,
    )
}

fn array_header(length: u64) -> Vec<u8> {
    let mut bytes = b"MSIG\r\n\x1a\n".to_vec();
    bytes.push(0x9b);
    bytes.extend(length.to_be_bytes());
    bytes
}

#[test]
fn snapshot_rejects_impossible_array_length_without_panicking() {
    let context = PersistenceContext::<()>::new(TimeDomainId::from_u128(1));
    let bytes = array_header(u64::MAX);
    let before = bytes.clone();
    let failure = decode_snapshot(&context, &bytes, &limits(u64::MAX)).unwrap_err();
    assert_eq!(failure.code().as_str(), "persistence.truncated_artifact");
    assert_eq!(bytes, before);
}

#[test]
fn replay_frame_rejects_impossible_array_length_without_panicking() {
    let context = PersistenceContext::<()>::new(TimeDomainId::from_u128(1));
    let bytes = array_header(u64::MAX);
    let before = bytes.clone();
    let failure = decode_replay_frame(&context, &bytes, &limits(u64::MAX)).unwrap_err();
    assert_eq!(failure.code().as_str(), "persistence.truncated_artifact");
    assert_eq!(bytes, before);
}

#[test]
fn replay_log_rejects_impossible_array_length_without_panicking() {
    let context = PersistenceContext::<()>::new(TimeDomainId::from_u128(1));
    let bytes = array_header(u64::MAX);
    let before = bytes.clone();
    let failure = decode_replay_log(&context, &bytes, &limits(u64::MAX)).unwrap_err();
    assert_eq!(failure.code().as_str(), "persistence.truncated_artifact");
    assert_eq!(bytes, before);
}
