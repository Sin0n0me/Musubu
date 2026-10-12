use crate::{VMResult, errors::VMError};
use alloc::vec::IntoIter;
use musubu_primitive::{Integer, Value};

#[derive(Debug)]
pub(crate) enum IteratorState {
    Array(IntoIter<Value>),
    Range {
        next: Option<Integer>,
        end: Integer,
        inclusive: bool,
    },
}

impl IteratorState {
    pub fn new(value: Value) -> VMResult<Self> {
        match value {
            Value::Array { elements, .. } => Ok(Self::Array(elements.into_iter())),
            Value::Range {
                start,
                end,
                inclusive,
            } => {
                if core::mem::discriminant(&start) != core::mem::discriminant(&end) {
                    return Err(VMError::InvalidOperand);
                }
                Ok(Self::Range {
                    next: Some(start),
                    end,
                    inclusive,
                })
            }
            _ => Err(VMError::InvalidOperand),
        }
    }

    pub fn next(&mut self) -> Option<Value> {
        match self {
            Self::Array(elements) => elements.next(),
            Self::Range {
                next,
                end,
                inclusive,
            } => {
                let current = next.take()?;
                if current > *end || (!*inclusive && current == *end) {
                    return None;
                }
                // Emit the endpoint once; never increment an inclusive maximum endpoint.
                if current != *end {
                    *next = current.checked_successor();
                }
                Some(Value::Integer(current))
            }
        }
    }
}
