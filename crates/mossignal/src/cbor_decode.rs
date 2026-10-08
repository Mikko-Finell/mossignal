//! Iterative canonical CBOR decoder for one snapshot artifact.
//!
//! Lengths are checked against the remaining input and the caller limit
//! before any buffer is allocated. Nesting uses a heap stack.

use std::string::FromUtf8Error;

#[derive(Clone, Debug)]
pub(crate) enum Value {
    Uint(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Value>),
    Bool(bool),
    Null,
}

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub total_bytes: u64,
    pub nesting: u64,
    pub text_bytes: u64,
    pub byte_string_bytes: u64,
    pub collection_items: u64,
}

#[derive(Debug)]
pub(crate) enum DecodeError {
    Truncated,
    Noncanonical {
        violation: &'static str,
        encountered: String,
    },
    Limit {
        budget: &'static str,
        limit: u64,
        consumed: u64,
    },
}

struct Frame {
    items: Vec<Value>,
    remaining: u64,
}

pub(crate) fn parse(input: &[u8], limits: Limits) -> Result<(Value, usize), DecodeError> {
    if input.len() as u64 > limits.total_bytes {
        return Err(DecodeError::Limit {
            budget: "total_bytes",
            limit: limits.total_bytes,
            consumed: input.len() as u64,
        });
    }
    let mut position = 0;
    let mut stack: Vec<Frame> = Vec::new();
    let mut done = None;
    loop {
        if let Some(value) = done.take() {
            if let Some(frame) = stack.last_mut() {
                frame.items.push(value);
                frame.remaining = frame.remaining.saturating_sub(1);
                if frame.remaining == 0 {
                    let frame = match stack.pop() {
                        Some(frame) => frame,
                        None => {
                            return Err(DecodeError::Noncanonical {
                                violation: "length_overflow",
                                encountered: "array frame".to_owned(),
                            });
                        }
                    };
                    done = Some(Value::Array(frame.items));
                }
                continue;
            }
            return Ok((value, position));
        }
        let depth = stack.len() as u64 + 1;
        if depth > limits.nesting {
            return Err(DecodeError::Limit {
                budget: "nesting",
                limit: limits.nesting,
                consumed: depth,
            });
        }
        let (major, info, argument, width) = header(input, position)?;
        position = position
            .checked_add(width)
            .ok_or(DecodeError::Noncanonical {
                violation: "length_overflow",
                encountered: width.to_string(),
            })?;
        match major {
            0 => done = Some(Value::Uint(argument)),
            2 => {
                let bytes = take_bytes(
                    input,
                    &mut position,
                    argument,
                    limits.byte_string_bytes,
                    "byte_string_bytes",
                )?;
                done = Some(Value::Bytes(bytes));
            }
            3 => {
                let bytes = take_bytes(
                    input,
                    &mut position,
                    argument,
                    limits.text_bytes,
                    "text_bytes",
                )?;
                let text = String::from_utf8(bytes).map_err(|error: FromUtf8Error| {
                    DecodeError::Noncanonical {
                        violation: "invalid_utf8",
                        encountered: error.utf8_error().valid_up_to().to_string(),
                    }
                })?;
                done = Some(Value::Text(text));
            }
            4 => {
                if argument > limits.collection_items {
                    return Err(DecodeError::Limit {
                        budget: "collection_items",
                        limit: limits.collection_items,
                        consumed: argument,
                    });
                }
                // SPEC: docs/specs/contracts/machine-snapshot-restoration.yaml "hostile-and-bounded-decode"
                // Each item needs at least one byte; a declared length cannot justify allocation.
                if argument > (input.len() - position) as u64 {
                    return Err(DecodeError::Truncated);
                }
                if argument == 0 {
                    done = Some(Value::Array(Vec::new()));
                } else {
                    stack.push(Frame {
                        items: Vec::new(),
                        remaining: argument,
                    });
                }
            }
            1 => {
                return Err(DecodeError::Noncanonical {
                    violation: "forbidden_negative",
                    encountered: argument.to_string(),
                });
            }
            5 => {
                return Err(DecodeError::Noncanonical {
                    violation: "forbidden_map",
                    encountered: info.to_string(),
                });
            }
            6 => {
                return Err(DecodeError::Noncanonical {
                    violation: "forbidden_tag",
                    encountered: argument.to_string(),
                });
            }
            _ => done = Some(simple(info, argument)?),
        }
    }
}

fn take_bytes(
    input: &[u8],
    position: &mut usize,
    length: u64,
    limit: u64,
    budget: &'static str,
) -> Result<Vec<u8>, DecodeError> {
    if length > limit {
        return Err(DecodeError::Limit {
            budget,
            limit,
            consumed: length,
        });
    }
    let length = usize::try_from(length).map_err(|_| DecodeError::Noncanonical {
        violation: "length_overflow",
        encountered: length.to_string(),
    })?;
    let end = position
        .checked_add(length)
        .ok_or_else(|| DecodeError::Noncanonical {
            violation: "length_overflow",
            encountered: length.to_string(),
        })?;
    if end > input.len() {
        return Err(DecodeError::Truncated);
    }
    let bytes = input[*position..end].to_vec();
    *position = end;
    Ok(bytes)
}

fn simple(info: u8, argument: u64) -> Result<Value, DecodeError> {
    match (info, argument) {
        (20, _) => Ok(Value::Bool(false)),
        (21, _) => Ok(Value::Bool(true)),
        (22, _) => Ok(Value::Null),
        (25..=27, _) => Err(DecodeError::Noncanonical {
            violation: "forbidden_float",
            encountered: info.to_string(),
        }),
        _ => Err(DecodeError::Noncanonical {
            violation: "forbidden_simple_value",
            encountered: argument.to_string(),
        }),
    }
}

fn header(input: &[u8], position: usize) -> Result<(u8, u8, u64, usize), DecodeError> {
    let initial = *input.get(position).ok_or(DecodeError::Truncated)?;
    let major = initial >> 5;
    let info = initial & 0x1f;
    if major == 7 && matches!(info, 25..=27) {
        return Err(DecodeError::Noncanonical {
            violation: "forbidden_float",
            encountered: info.to_string(),
        });
    }
    if info == 31 {
        return Err(DecodeError::Noncanonical {
            violation: "indefinite_length",
            encountered: major.to_string(),
        });
    }
    let (argument, width) = argument(input, position, major, info)?;
    Ok((major, info, argument, width))
}

fn argument(
    input: &[u8],
    position: usize,
    major: u8,
    info: u8,
) -> Result<(u64, usize), DecodeError> {
    if info < 24 {
        return Ok((u64::from(info), 1));
    }
    let (value, width) = match info {
        24 => {
            let byte = *input.get(position + 1).ok_or(DecodeError::Truncated)?;
            (u64::from(byte), 2)
        }
        25 => {
            let bytes = input
                .get(position + 1..position + 3)
                .ok_or(DecodeError::Truncated)?;
            let mut encoded = [0; 2];
            encoded.copy_from_slice(bytes);
            (u64::from(u16::from_be_bytes(encoded)), 3)
        }
        26 => {
            let bytes = input
                .get(position + 1..position + 5)
                .ok_or(DecodeError::Truncated)?;
            let mut encoded = [0; 4];
            encoded.copy_from_slice(bytes);
            (u64::from(u32::from_be_bytes(encoded)), 5)
        }
        27 => {
            let bytes = input
                .get(position + 1..position + 9)
                .ok_or(DecodeError::Truncated)?;
            let mut encoded = [0; 8];
            encoded.copy_from_slice(bytes);
            (u64::from_be_bytes(encoded), 9)
        }
        _ => {
            return Err(DecodeError::Noncanonical {
                violation: "forbidden_simple_value",
                encountered: info.to_string(),
            });
        }
    };
    let shortest = if value <= 23 {
        1
    } else if value <= u64::from(u8::MAX) {
        2
    } else if value <= u64::from(u16::MAX) {
        3
    } else if value <= u64::from(u32::MAX) {
        5
    } else {
        9
    };
    if width != shortest {
        let violation = if matches!(major, 2..=5) {
            "non_shortest_length"
        } else {
            "non_shortest_integer"
        };
        return Err(DecodeError::Noncanonical {
            violation,
            encountered: value.to_string(),
        });
    }
    Ok((value, width))
}

pub(crate) fn encode(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_value(&mut bytes, value);
    bytes
}

pub(crate) fn encode_named_pairs(pairs: &[(&str, &Value)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_major(&mut bytes, 4, pairs.len() as u64);
    for (name, value) in pairs {
        write_major(&mut bytes, 4, 2);
        write_major(&mut bytes, 3, name.len() as u64);
        bytes.extend(name.as_bytes());
        bytes.extend(encode(value));
    }
    bytes
}

fn write_value(bytes: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Uint(value) => write_major(bytes, 0, *value),
        Value::Bytes(value) => {
            write_major(bytes, 2, value.len() as u64);
            bytes.extend(value);
        }
        Value::Text(value) => {
            write_major(bytes, 3, value.len() as u64);
            bytes.extend(value.as_bytes());
        }
        Value::Array(values) => {
            write_major(bytes, 4, values.len() as u64);
            for value in values {
                write_value(bytes, value);
            }
        }
        Value::Bool(true) => bytes.push(0xf5),
        Value::Bool(false) => bytes.push(0xf4),
        Value::Null => bytes.push(0xf6),
    }
}

fn write_major(bytes: &mut Vec<u8>, major: u8, value: u64) {
    let initial = major << 5;
    if value <= 23 {
        bytes.push(initial | value as u8);
    } else if u8::try_from(value).is_ok() {
        bytes.extend([initial | 24, value as u8]);
    } else if u16::try_from(value).is_ok() {
        bytes.push(initial | 25);
        bytes.extend_from_slice(&(value as u16).to_be_bytes());
    } else if u32::try_from(value).is_ok() {
        bytes.push(initial | 26);
        bytes.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        bytes.push(initial | 27);
        bytes.extend_from_slice(&value.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(collection_items: u64) -> Limits {
        Limits {
            total_bytes: 100,
            nesting: 16,
            text_bytes: 100,
            byte_string_bytes: 100,
            collection_items,
        }
    }

    #[test]
    fn arrays_accept_complete_values_and_preserve_canonical_bytes() {
        for bytes in [
            vec![0x80],
            vec![0x83, 0x00, 0xf4, 0xf6],
            vec![0x82, 0x82, 0x00, 0x01, 0x80],
            [vec![0x98, 24], vec![0x00; 24]].concat(),
        ] {
            let (value, consumed) = parse(&bytes, limits(24)).unwrap();
            assert_eq!(consumed, bytes.len());
            assert_eq!(encode(&value), bytes);
        }
    }

    #[test]
    fn truncated_arrays_fail_at_root_and_nested_lengths() {
        for bytes in [vec![0x82, 0x00], vec![0x81, 0x83, 0x00, 0x01]] {
            assert!(matches!(
                parse(&bytes, limits(3)),
                Err(DecodeError::Truncated)
            ));
        }
    }

    #[test]
    fn collection_limit_precedes_truncation_and_accepts_exact_boundary() {
        assert!(matches!(
            parse(&[0x83], limits(2)),
            Err(DecodeError::Limit {
                budget: "collection_items",
                limit: 2,
                consumed: 3,
            })
        ));
        assert!(parse(&[0x82, 0x00, 0x01], limits(2)).is_ok());
    }
}
