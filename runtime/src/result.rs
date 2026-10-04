//! Explicit conversion of a Result's failure value.

/// Move the success value through unchanged, or call `mapper` once on failure.
/// The helper does not clone, allocate, or catch panics from the mapper.
#[inline]
pub fn map_error<T, E, F>(value: Result<T, E>, mapper: fn(E) -> F) -> Result<T, F> {
    value.map_err(mapper)
}

#[cfg(test)]
mod tests {
    use super::map_error;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn mapper_runs_once_only_for_failure() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn convert(error: usize) -> usize {
            CALLS.fetch_add(1, Ordering::SeqCst);
            error + 1
        }
        assert_eq!(map_error(Ok::<_, usize>(42), convert), Ok(42));
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(map_error(Err::<i64, _>(7), convert), Err(8));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn owned_payloads_are_moved_without_cloning() {
        struct Cause(Box<str>);
        fn convert(error: Box<str>) -> Cause {
            Cause(error)
        }
        let success: Box<str> = "success".into();
        let success_pointer = success.as_ptr();
        match map_error(Ok::<_, Box<str>>(success), convert) {
            Ok(value) => assert_eq!(value.as_ptr(), success_pointer),
            Err(_) => panic!("success must be preserved"),
        }
        let failure: Box<str> = "cause".into();
        let failure_pointer = failure.as_ptr();
        match map_error(Err::<Box<str>, _>(failure), convert) {
            Err(Cause(value)) => assert_eq!(value.as_ptr(), failure_pointer),
            Ok(_) => panic!("failure must be mapped"),
        }
    }

    #[test]
    fn borrowed_success_keeps_its_owner() {
        fn convert(error: u8) -> u16 {
            u16::from(error)
        }
        let owner = String::from("borrowed");
        let value = map_error(Ok::<_, u8>(owner.as_str()), convert).unwrap();
        assert_eq!(value.as_ptr(), owner.as_ptr());
    }
}
