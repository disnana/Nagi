use crate::Error;

const INPUT_LIMIT_ERROR: &str = "HTML text exceeds configured input byte limit";
const OUTPUT_LIMIT_ERROR: &str = "HTML text exceeds configured output byte limit";
const CONTROL_ERROR: &str = "HTML text contains a disallowed control character";
const RESERVE_ERROR: &str = "HTML encoder allocation failed";

fn entity(character: char) -> Result<Option<&'static str>, Error> {
    match character {
        '&' => Ok(Some("&amp;")),
        '<' => Ok(Some("&lt;")),
        '>' => Ok(Some("&gt;")),
        '"' => Ok(Some("&quot;")),
        '\'' => Ok(Some("&#39;")),
        '\t' => Ok(Some("&#9;")),
        '\n' => Ok(Some("&#10;")),
        '\r' => Ok(Some("&#13;")),
        '\u{0}'..='\u{8}' | '\u{b}'..='\u{c}' | '\u{e}'..='\u{1f}' | '\u{7f}' => {
            Err(Error::invalid(CONTROL_ERROR))
        }
        _ => Ok(None),
    }
}

pub(super) fn encoded_len(
    value: &str,
    max_input: usize,
    max_output: usize,
) -> Result<usize, crate::Error> {
    if value.len() > max_input {
        return Err(Error::invalid(INPUT_LIMIT_ERROR));
    }

    let mut output_len = 0usize;
    for character in value.chars() {
        let character_len = match entity(character)? {
            Some(replacement) => replacement.len(),
            None => character.len_utf8(),
        };
        output_len = output_len
            .checked_add(character_len)
            .ok_or_else(|| Error::invalid(OUTPUT_LIMIT_ERROR))?;
    }
    if output_len > max_output {
        return Err(Error::invalid(OUTPUT_LIMIT_ERROR));
    }
    Ok(output_len)
}

pub(super) fn encode(
    value: &str,
    max_input: usize,
    max_output: usize,
) -> Result<String, crate::Error> {
    let output_len = encoded_len(value, max_input, max_output)?;
    let mut output = String::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|_| Error::internal(RESERVE_ERROR))?;

    for character in value.chars() {
        match entity(character)? {
            Some(replacement) => output.push_str(replacement),
            None => output.push(character),
        }
    }
    Ok(output)
}
