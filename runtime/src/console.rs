use crate::Error;
use std::io::Write;

/// Console-only blocking input. Flush pending output before waiting; preserve spaces.
pub fn read_line() -> Result<String, Error> {
    read_line_io(
        |line| std::io::stdin().read_line(line),
        &mut std::io::stdout(),
        None,
    )
}

/// Write a prompt without a newline, flush it, then read one console line.
pub fn read_line_prompt(prompt: &str) -> Result<String, Error> {
    read_line_io(
        |line| std::io::stdin().read_line(line),
        &mut std::io::stdout(),
        Some(prompt),
    )
}

fn read_line_io(
    input: impl FnOnce(&mut String) -> std::io::Result<usize>,
    output: &mut impl Write,
    prompt: Option<&str>,
) -> Result<String, Error> {
    let io_error = |error: std::io::Error| Error::internal(error.to_string());
    if let Some(prompt) = prompt {
        output.write_all(prompt.as_bytes()).map_err(io_error)?;
    }
    output.flush().map_err(io_error)?;
    let mut line = String::new();
    input(&mut line).map_err(io_error)?;
    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        io::{self, BufRead},
        rc::Rc,
    };

    struct Output {
        events: Rc<RefCell<Vec<&'static str>>>,
        bytes: Vec<u8>,
        fail: Option<&'static str>,
    }
    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.events.borrow_mut().push("write");
            if self.fail == Some("write") {
                return Err(io::Error::other("write failed"));
            }
            // Exercise write_all, including partial writes and UTF-8 byte boundaries.
            let count = bytes.len().min(1);
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.events.borrow_mut().push("flush");
            if self.fail == Some("flush") {
                Err(io::Error::other("flush failed"))
            } else {
                Ok(())
            }
        }
    }
    struct Input {
        events: Rc<RefCell<Vec<&'static str>>>,
        data: io::Cursor<Vec<u8>>,
        fail: bool,
    }
    impl io::Read for Input {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.data.read(bytes)
        }
    }
    impl BufRead for Input {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.events.borrow_mut().push("read");
            if self.fail {
                Err(io::Error::other("read failed"))
            } else {
                self.data.fill_buf()
            }
        }
        fn consume(&mut self, amount: usize) {
            self.data.consume(amount);
        }
    }
    fn fixture(data: &[u8], fail: Option<&'static str>) -> (Input, Output) {
        let events = Rc::new(RefCell::new(Vec::new()));
        (
            Input {
                events: events.clone(),
                data: io::Cursor::new(data.to_vec()),
                fail: fail == Some("read"),
            },
            Output {
                events,
                bytes: Vec::new(),
                fail,
            },
        )
    }
    #[test]
    fn prompt_is_written_fully_then_flushed_before_input() {
        let (mut input, mut output) = fixture(b"  Nagi  \r\n", None);
        assert_eq!(
            read_line_io(|line| input.read_line(line), &mut output, Some("名前: ")).unwrap(),
            "  Nagi  "
        );
        assert_eq!(output.bytes, "名前: ".as_bytes());
        let events = output.events.borrow();
        let flush = events.iter().position(|event| *event == "flush").unwrap();
        assert!(events[..flush].iter().all(|event| *event == "write"));
        assert_eq!(flush, "名前: ".len());
        assert_eq!(events[flush + 1], "read");
    }
    #[test]
    fn zero_argument_empty_prompt_eof_and_line_endings_preserve_existing_values() {
        for data in [b"".as_slice(), b"\n", b"a\nnext\n", b" a \r\n", b"last"] {
            for prompt in [None, Some("")] {
                let (mut input, mut output) = fixture(data, None);
                let expected = match data {
                    b"a\nnext\n" => "a",
                    b" a \r\n" => " a ",
                    b"last" => "last",
                    _ => "",
                };
                assert_eq!(
                    read_line_io(|line| input.read_line(line), &mut output, prompt).unwrap(),
                    expected
                );
                assert!(output.bytes.is_empty());
                assert_eq!(output.events.borrow()[0], "flush");
            }
        }
    }
    #[test]
    fn write_flush_and_read_failures_return_errors_and_stop_later_operations() {
        for phase in ["write", "flush", "read"] {
            let (mut input, mut output) = fixture(b"input\n", Some(phase));
            let error = read_line_io(|line| input.read_line(line), &mut output, Some("prompt"))
                .unwrap_err();
            assert_eq!(crate::error_kind(&error), "internal");
            assert_eq!(error.message, format!("{phase} failed"));
            let events = output.events.borrow();
            assert_eq!(events.last(), Some(&phase));
            if phase == "write" {
                assert!(!events.contains(&"flush"));
            }
            if phase != "read" {
                assert!(!events.contains(&"read"));
            }
        }
        let (mut input, mut output) = fixture(b"\xff\n", None);
        assert_eq!(
            crate::error_kind(
                &read_line_io(|line| input.read_line(line), &mut output, None).unwrap_err()
            ),
            "internal"
        );
        for phase in ["flush", "read"] {
            let (mut input, mut output) = fixture(b"input\n", Some(phase));
            let error = read_line_io(|line| input.read_line(line), &mut output, None).unwrap_err();
            assert_eq!(crate::error_kind(&error), "internal");
            assert_eq!(error.message, format!("{phase} failed"));
            assert_eq!(output.events.borrow().last(), Some(&phase));
            assert!(!output.events.borrow().contains(&"write"));
        }
    }
}
