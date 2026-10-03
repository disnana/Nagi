//! Retained owned storage accounting for actor messages and business replies.
//! Root inline storage is charged once; recursive visits return heap bytes only.
use std::{fmt, mem::size_of, time::Instant};

pub const MAX_DEPTH: usize = 64;
pub const MAX_WORK: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeError {
    Overflow,
    TooLarge,
    Depth,
    Work,
    Deadline,
}

impl fmt::Display for ChargeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Overflow => "actor payload charge overflow",
            Self::TooLarge => "actor payload exceeds the byte limit",
            Self::Depth => "actor payload exceeds the charge depth limit",
            Self::Work => "actor payload exceeds the charge work limit",
            Self::Deadline => "actor admission deadline elapsed during payload charging",
        })
    }
}

impl std::error::Error for ChargeError {}

/// Implementations report only retained owned allocations, excluding Self's
/// inline bytes. Shared graphs and resources require a separate trusted policy.
pub trait ChargeOwned {
    const INLINE_ONLY: bool;
    fn owned_heap_bytes(&self, walk: &mut ChargeWalk) -> Result<usize, ChargeError>;
}

pub struct ChargeWalk {
    heap_limit: usize,
    deadline: Option<Instant>,
    depth: usize,
    work: usize,
}

impl ChargeWalk {
    pub fn new(heap_limit: usize, deadline: Option<Instant>) -> Self {
        Self {
            heap_limit,
            deadline,
            depth: 0,
            work: 0,
        }
    }

    fn check_deadline(&self) -> Result<(), ChargeError> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            Err(ChargeError::Deadline)
        } else {
            Ok(())
        }
    }

    pub fn visit<T: ChargeOwned>(&mut self, value: &T) -> Result<usize, ChargeError> {
        self.check_deadline()?;
        if self.depth >= MAX_DEPTH {
            return Err(ChargeError::Depth);
        }
        if self.work >= MAX_WORK {
            return Err(ChargeError::Work);
        }
        self.work += 1;
        self.depth += 1;
        let result = if T::INLINE_ONLY {
            Ok(0)
        } else {
            value.owned_heap_bytes(self)
        };
        self.depth -= 1;
        result
    }

    pub fn add(&self, left: usize, right: usize) -> Result<usize, ChargeError> {
        self.check_deadline()?;
        let total = left.checked_add(right).ok_or(ChargeError::Overflow)?;
        if total > self.heap_limit {
            Err(ChargeError::TooLarge)
        } else {
            Ok(total)
        }
    }

    pub fn allocation(&self, capacity: usize, element_size: usize) -> Result<usize, ChargeError> {
        let bytes = capacity
            .checked_mul(element_size)
            .ok_or(ChargeError::Overflow)?;
        self.add(0, bytes)
    }
}

/// `envelope_bytes` is fixed metadata outside T, excluding inline T storage.
/// Waiting callers, actor state/context, and returned caller values are outside
/// this accepted-payload budget. A deadline bounds the walk as well as admission.
pub fn charged_bytes<T: ChargeOwned>(
    value: &T,
    envelope_bytes: usize,
    byte_limit: usize,
    deadline: Option<Instant>,
) -> Result<usize, ChargeError> {
    let mut walk = ChargeWalk::new(byte_limit, deadline);
    walk.check_deadline()?;
    let inline = envelope_bytes
        .checked_add(size_of::<T>())
        .ok_or(ChargeError::Overflow)?;
    walk.heap_limit = byte_limit
        .checked_sub(inline)
        .ok_or(ChargeError::TooLarge)?;
    let heap = walk.visit(value)?;
    let total = inline.checked_add(heap).ok_or(ChargeError::Overflow)?;
    if total > byte_limit {
        Err(ChargeError::TooLarge)
    } else {
        Ok(total)
    }
}

macro_rules! inline_owned {
    ($($ty:ty),* $(,)?) => {$ (
        impl ChargeOwned for $ty {
            const INLINE_ONLY: bool = true;
            fn owned_heap_bytes(&self, _: &mut ChargeWalk) -> Result<usize, ChargeError> {
                Ok(0)
            }
        }
    )*};
}

inline_owned!(
    (),
    bool,
    i8,
    i16,
    i32,
    i64,
    u8,
    u16,
    u32,
    u64,
    f32,
    f64,
    crate::Uuid,
    crate::Timestamp,
);

impl ChargeOwned for String {
    const INLINE_ONLY: bool = false;
    fn owned_heap_bytes(&self, walk: &mut ChargeWalk) -> Result<usize, ChargeError> {
        walk.allocation(self.capacity(), size_of::<u8>())
    }
}

impl<T: ChargeOwned> ChargeOwned for Vec<T> {
    const INLINE_ONLY: bool = false;
    fn owned_heap_bytes(&self, walk: &mut ChargeWalk) -> Result<usize, ChargeError> {
        let mut heap = walk.allocation(self.capacity(), size_of::<T>())?;
        if !T::INLINE_ONLY {
            for value in self {
                let nested = walk.visit(value)?;
                heap = walk.add(heap, nested)?;
            }
        }
        Ok(heap)
    }
}

impl<T: ChargeOwned> ChargeOwned for Option<T> {
    const INLINE_ONLY: bool = T::INLINE_ONLY;
    fn owned_heap_bytes(&self, walk: &mut ChargeWalk) -> Result<usize, ChargeError> {
        match self {
            Some(value) => walk.visit(value),
            None => Ok(0),
        }
    }
}

impl<T: ChargeOwned, E: ChargeOwned> ChargeOwned for Result<T, E> {
    const INLINE_ONLY: bool = T::INLINE_ONLY && E::INLINE_ONLY;
    fn owned_heap_bytes(&self, walk: &mut ChargeWalk) -> Result<usize, ChargeError> {
        match self {
            Ok(value) => walk.visit(value),
            Err(value) => walk.visit(value),
        }
    }
}

impl ChargeOwned for crate::Error {
    const INLINE_ONLY: bool = false;
    fn owned_heap_bytes(&self, walk: &mut ChargeWalk) -> Result<usize, ChargeError> {
        walk.allocation(self.message.capacity(), size_of::<u8>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of_val;

    #[test]
    fn root_inline_and_envelope_are_counted_once() {
        let bytes = 19 + size_of::<i64>();
        assert_eq!(charged_bytes(&7_i64, 19, bytes, None), Ok(bytes));
        assert_eq!(
            charged_bytes(&7_i64, 19, bytes - 1, None),
            Err(ChargeError::TooLarge)
        );
        assert_eq!(charged_bytes(&(), 19, 19, None), Ok(19));
    }

    #[test]
    fn retained_string_bytes_and_error_capacities_are_charged() {
        let mut text = String::with_capacity(128);
        text.push_str("short");
        let expected = size_of::<String>() + text.capacity();
        assert_eq!(charged_bytes(&text, 0, expected, None), Ok(expected));
        assert_eq!(
            charged_bytes(&text, 0, expected - 1, None),
            Err(ChargeError::TooLarge)
        );
        let bytes = Vec::<u8>::with_capacity(256);
        let expected = size_of::<Vec<u8>>() + bytes.capacity();
        assert_eq!(charged_bytes(&bytes, 0, expected, None), Ok(expected));
        let error = crate::Error::invalid(text);
        let expected = size_of::<crate::Error>() + error.message.capacity();
        assert_eq!(charged_bytes(&error, 0, expected, None), Ok(expected));
    }

    #[test]
    fn list_capacity_and_only_initialized_elements_contribute_heap() {
        let mut values = Vec::with_capacity(17);
        values.push(String::with_capacity(31));
        values.push(String::with_capacity(79));
        let expected = size_of::<Vec<String>>()
            + values.capacity() * size_of::<String>()
            + values.iter().map(String::capacity).sum::<usize>();
        assert_eq!(charged_bytes(&values, 0, expected, None), Ok(expected));
        assert_eq!(
            charged_bytes(&values, 0, expected - 1, None),
            Err(ChargeError::TooLarge)
        );
    }

    #[test]
    fn inline_only_lists_skip_every_element_and_zero_sized_capacity_is_safe() {
        struct Inline;
        impl ChargeOwned for Inline {
            const INLINE_ONLY: bool = true;
            fn owned_heap_bytes(&self, _: &mut ChargeWalk) -> Result<usize, ChargeError> {
                panic!("inline-only elements must not be visited");
            }
        }
        let mut values = Vec::new();
        values.resize_with(MAX_WORK * 2, || Inline);
        assert_eq!(values.capacity(), usize::MAX);
        assert_eq!(
            charged_bytes(&values, 0, size_of::<Vec<Inline>>(), None),
            Ok(size_of::<Vec<Inline>>())
        );
        let numbers = vec![1_i64; MAX_WORK * 2];
        let expected = size_of::<Vec<i64>>() + numbers.capacity() * size_of::<i64>();
        assert_eq!(charged_bytes(&numbers, 0, expected, None), Ok(expected));
    }

    #[test]
    fn option_and_result_visit_only_the_active_payload() {
        let empty: Option<String> = None;
        assert_eq!(
            charged_bytes(&empty, 0, size_of_val(&empty), None),
            Ok(size_of_val(&empty))
        );
        let value = Some(String::with_capacity(83));
        let expected = size_of_val(&value) + value.as_ref().unwrap().capacity();
        assert_eq!(charged_bytes(&value, 0, expected, None), Ok(expected));
        let value: Result<Vec<String>, String> = Err(String::with_capacity(71));
        let expected = size_of_val(&value) + value.as_ref().unwrap_err().capacity();
        assert_eq!(charged_bytes(&value, 0, expected, None), Ok(expected));
    }

    #[test]
    fn work_budget_includes_root_and_initialized_non_inline_elements() {
        let mut values = Vec::new();
        values.resize_with(MAX_WORK - 1, String::new);
        assert!(charged_bytes(&values, 0, usize::MAX, None).is_ok());
        values.push(String::new());
        assert_eq!(
            charged_bytes(&values, 0, usize::MAX, None),
            Err(ChargeError::Work)
        );
    }

    #[test]
    fn recursive_owned_graphs_obey_depth_limit_and_restore_walk_after_errors() {
        struct Node(Vec<Node>);
        impl ChargeOwned for Node {
            const INLINE_ONLY: bool = false;
            fn owned_heap_bytes(&self, walk: &mut ChargeWalk) -> Result<usize, ChargeError> {
                walk.visit(&self.0)
            }
        }
        let chain = |levels| {
            let mut node = Node(Vec::new());
            for _ in 0..levels {
                node = Node(vec![node]);
            }
            node
        };
        assert!(charged_bytes(&chain(MAX_DEPTH / 2 - 1), 0, usize::MAX, None).is_ok());
        let mut walk = ChargeWalk::new(usize::MAX, None);
        assert_eq!(walk.visit(&chain(MAX_DEPTH / 2)), Err(ChargeError::Depth));
        assert_eq!(walk.depth, 0);
        assert_eq!(walk.visit(&1_i64), Ok(0));
    }

    #[test]
    fn arithmetic_and_expired_deadlines_fail_before_acceptance() {
        assert_eq!(
            charged_bytes(&1_u8, usize::MAX, usize::MAX, None),
            Err(ChargeError::Overflow)
        );
        let walk = ChargeWalk::new(usize::MAX, None);
        assert_eq!(walk.allocation(usize::MAX, 2), Err(ChargeError::Overflow));
        assert_eq!(walk.add(usize::MAX, 1), Err(ChargeError::Overflow));
        assert_eq!(
            charged_bytes(&1_i64, 0, usize::MAX, Some(Instant::now())),
            Err(ChargeError::Deadline)
        );
    }
}
