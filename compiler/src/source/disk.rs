use std::io::{self, Read};

pub(super) struct DiskSource {
    pub(super) text: Option<String>,
    pub(super) bytes: usize,
    #[cfg(test)]
    peak_retained: usize,
}

pub(super) fn read_source(reader: &mut impl Read, retain_limit: usize) -> io::Result<DiskSource> {
    // Reading to EOF preserves read-error precedence, including errors after an
    // oversize or invalid UTF-8 prefix. This bounds retained input, not read work.
    const CHUNK: usize = 8192;
    let mut buffer = [0_u8; CHUNK + 3];
    let mut pending = 0;
    let mut invalid_utf8 = false;
    let mut retained = Vec::new();
    let mut bytes = 0_usize;
    loop {
        let n = match reader.read(&mut buffer[pending..pending + CHUNK]) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        bytes = bytes.saturating_add(n);
        let keep = n.min(retain_limit - retained.len());
        let required = retained.len() + keep;
        if required > retained.capacity() {
            // Do not let geometric growth request capacity beyond the limit.
            // Allocator rounding is not a physical-memory/RSS guarantee.
            let capacity = retained
                .capacity()
                .saturating_mul(2)
                .max(required)
                .min(retain_limit);
            retained.try_reserve_exact(capacity - retained.len())?;
        }
        retained.extend_from_slice(&buffer[pending..pending + keep]);
        if !invalid_utf8 {
            let end = pending + n;
            match std::str::from_utf8(&buffer[..end]) {
                Ok(_) => pending = 0,
                Err(error) if error.error_len().is_none() => {
                    let valid_end = error.valid_up_to();
                    pending = end - valid_end;
                    debug_assert!(pending <= 3);
                    buffer.copy_within(valid_end..end, 0);
                }
                Err(_) => {
                    invalid_utf8 = true;
                    pending = 0;
                }
            }
        }
    }
    if invalid_utf8 || pending != 0 {
        // Use the standard Read implementation's UTF-8 error rather than
        // duplicating its classification or diagnostic text here.
        return Err([0xff]
            .as_slice()
            .read_to_string(&mut String::new())
            .unwrap_err());
    }
    #[cfg(test)]
    let peak_retained = retained.len();
    let text = if bytes <= retain_limit {
        Some(String::from_utf8(retained).expect("the complete retained source was UTF-8 validated"))
    } else {
        None
    };
    Ok(DiskSource {
        text,
        bytes,
        #[cfg(test)]
        peak_retained,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture<'a> {
        input: &'a [u8],
        position: usize,
        chunk: usize,
        fail_at: Option<usize>,
        interrupt_once: bool,
        eof_reads: usize,
    }

    impl<'a> Fixture<'a> {
        fn new(input: &'a [u8], chunk: usize, fail_at: Option<usize>) -> Self {
            Self {
                input,
                position: 0,
                chunk,
                fail_at,
                interrupt_once: true,
                eof_reads: 0,
            }
        }
    }

    impl Read for Fixture<'_> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.interrupt_once {
                self.interrupt_once = false;
                return Err(io::ErrorKind::Interrupted.into());
            }
            if self.fail_at == Some(self.position) {
                return Err(io::Error::other("owned read failure"));
            }
            let remaining = self.input.len() - self.position;
            let until_error = self.fail_at.map_or(remaining, |at| at - self.position);
            let n = out.len().min(self.chunk).min(remaining).min(until_error);
            out[..n].copy_from_slice(&self.input[self.position..self.position + n]);
            self.position += n;
            if n == 0 {
                self.eof_reads += 1;
            }
            Ok(n)
        }
    }

    fn compare(input: &[u8], chunk: usize, limit: usize, fail_at: Option<usize>) {
        let mut standard = Fixture::new(input, chunk, fail_at);
        let mut expected_text = String::new();
        let expected = standard.read_to_string(&mut expected_text);
        let mut bounded = Fixture::new(input, chunk, fail_at);
        let actual = read_source(&mut bounded, limit);
        assert_eq!(bounded.position, standard.position);
        assert_eq!(bounded.eof_reads, standard.eof_reads);
        match (expected, actual) {
            (Ok(bytes), Ok(source)) => {
                assert_eq!(source.bytes, bytes);
                assert_eq!(source.text, (bytes <= limit).then_some(expected_text));
                assert!(source.peak_retained <= limit);
            }
            (Err(expected), Err(actual)) => {
                assert_eq!(actual.kind(), expected.kind());
                assert_eq!(actual.to_string(), expected.to_string());
            }
            _ => panic!("standard and bounded reader classifications differ"),
        }
    }

    #[test]
    fn retained_bytes_stop_at_small_limit_after_overflow() {
        let mut reader = Fixture::new(b"abcdefgh", 2, None);
        let result = read_source(&mut reader, 4).unwrap();
        assert_eq!(reader.position, 8);
        assert_eq!(reader.eof_reads, 1);
        assert_eq!(result.bytes, 8);
        assert!(result.text.is_none());
        assert!(result.peak_retained <= 4);
    }

    #[test]
    fn matches_standard_read_for_small_boundaries_and_utf8_splits() {
        let inputs: &[&[u8]] = &[
            b"",
            b"abcd",
            b"abcde",
            "aé中💡z".as_bytes(),
            b"abc\xffz",
            b"\xc0\x80",
            b"\xed\xa0\x80",
            b"\xf4\x90\x80\x80",
            b"abcd\xe2\x82",
            b"ab\xc3x",
            b"abcdefgh\xff",
        ];
        for input in inputs {
            for chunk in 1..=4 {
                for limit in [0, 1, 4, 8, 16] {
                    compare(input, chunk, limit, None);
                }
            }
        }
    }

    #[test]
    fn io_error_wins_over_overflow_and_invalid_utf8() {
        for input in [&b"abcdefgh"[..], &b"ab\xffdefgh"[..], &b"abcde\xe2\x82"[..]] {
            for fail_at in 0..=input.len() {
                for chunk in 1..=3 {
                    compare(input, chunk, 4, Some(fail_at));
                }
            }
        }
    }

    #[test]
    fn utf8_suffix_crosses_the_fixed_buffer_boundary_after_overflow() {
        let mut input = vec![b'a'; 8191];
        input.extend_from_slice("💡".as_bytes());
        compare(&input, 8192, 4, None);
        compare(&input, 8192, 4, Some(input.len()));
        input.pop();
        compare(&input, 8192, 4, None);
    }
}
