//! Recognize the final libtest summary, not a successful empty executable.
fn count(part: &str, suffix: &str) -> Option<u64> {
    let digits = part.strip_suffix(suffix)?;
    (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

pub(crate) fn successful_native_test(exit_success: bool, stdout: &[u8]) -> bool {
    if !exit_success {
        return false;
    }
    let Some(text) = std::str::from_utf8(stdout).ok() else {
        return false;
    };
    let Some(last) = text.lines().rev().find(|line| !line.is_empty()) else {
        return false;
    };
    let Some(summary) = last
        .trim_end_matches('\r')
        .strip_prefix("test result: ok. ")
    else {
        return false;
    };
    let mut fields = summary.split("; ");
    let passed = fields.next().and_then(|part| count(part, " passed"));
    let failed = fields.next().and_then(|part| count(part, " failed"));
    let ignored = fields.next().and_then(|part| count(part, " ignored"));
    let measured = fields.next().and_then(|part| count(part, " measured"));
    let filtered = fields.next().and_then(|part| count(part, " filtered out"));
    let duration = fields
        .next()
        .and_then(|part| part.strip_prefix("finished in "));
    let valid_duration = duration.is_some_and(|part| {
        part.strip_suffix('s').is_some_and(|seconds| {
            !seconds.is_empty()
                && seconds
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b'.')
        })
    });
    passed.is_some_and(|n| n > 0)
        && failed == Some(0)
        && ignored == Some(0)
        && measured.is_some()
        && filtered == Some(0)
        && valid_duration
        && fields.next().is_none()
}
