#[derive(Clone, Debug, PartialEq)]
pub enum K {
    Id(String),
    Num(String),
    Str(String),
    Sym(String),
    Newline,
    Indent,
    Dedent,
    Eof,
}
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: K,
    pub line: usize,
    pub col: usize,
}
pub fn lex(src: &str, high: bool) -> Result<Vec<Token>, String> {
    let mut out = vec![];
    let mut indents = vec![0];
    let mut depth = 0i32;
    for (ln, text) in src.lines().enumerate() {
        let line = ln + 1;
        if text.trim().is_empty() || text.trim_start().starts_with('#') {
            continue;
        }
        if text.contains('\t') {
            return Err(format!(
                "line {line}: タブは使えません。空白で字下げしてください"
            ));
        }
        let cs: Vec<char> = text.chars().collect();
        let mut i = cs.iter().take_while(|&&c| c == ' ').count();
        // 字下げはHighの文法。Lowは波括弧とセミコロンを独立して読む。
        if high && depth == 0 {
            if i > *indents.last().unwrap() {
                indents.push(i);
                out.push(Token {
                    kind: K::Indent,
                    line,
                    col: 1,
                });
            }
            while i < *indents.last().unwrap() {
                indents.pop();
                out.push(Token {
                    kind: K::Dedent,
                    line,
                    col: 1,
                });
            }
            if i != *indents.last().unwrap() {
                return Err(format!("line {line}: 字下げが外側のブロックと一致しません"));
            }
        }
        while i < cs.len() {
            let col = i + 1;
            let c = cs[i];
            if c == ' ' || c == '\r' {
                i += 1;
                continue;
            }
            if c == '#' {
                break;
            }
            let kind = if c.is_ascii_alphabetic() || c == '_' {
                let start = i;
                i += 1;
                while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                K::Id(cs[start..i].iter().collect())
            } else if c.is_ascii_digit() {
                let start = i;
                i += 1;
                while i < cs.len()
                    && (cs[i].is_ascii_digit()
                        || cs[i] == '_'
                        || cs[i] == '.' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit())
                {
                    i += 1;
                }
                K::Num(cs[start..i].iter().filter(|&&x| x != '_').collect())
            } else if c == '"' || c == '\'' {
                let quote = c;
                i += 1;
                let mut s = String::new();
                let mut closed = false;
                while i < cs.len() {
                    let v = cs[i];
                    i += 1;
                    if v == quote {
                        closed = true;
                        break;
                    }
                    if v == '\\' {
                        if i >= cs.len() {
                            break;
                        }
                        let v = cs[i];
                        i += 1;
                        s.push(match v {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '"' => '"',
                            '\'' => '\'',
                            '\\' => '\\',
                            _ => {
                                return Err(format!("line {line}:{col}: 未対応の文字列エスケープ"))
                            }
                        });
                    } else {
                        s.push(v);
                    }
                }
                if !closed {
                    return Err(format!("line {line}:{col}: 文字列が閉じていません"));
                }
                K::Str(s)
            } else {
                let pair = cs.get(i + 1).map(|n| format!("{c}{n}")).unwrap_or_default();
                let s = if ["->", "==", "!=", "<=", ">=", "+=", "-=", "*=", "::"]
                    .contains(&pair.as_str())
                {
                    i += 2;
                    pair
                } else if "()[]{}:,.@?+-*/%<>=!".contains(c) || c == ';' {
                    i += 1;
                    c.to_string()
                } else {
                    return Err(format!("line {line}:{col}: 未対応の文字 {c:?}"));
                };
                // Parentheses and brackets continue expressions in both
                // syntaxes. Braces still delimit Low statement blocks.
                if s == "(" || s == "[" {
                    depth += 1;
                }
                if s == ")" || s == "]" {
                    depth -= 1;
                }
                if depth < 0 {
                    return Err(format!("line {line}:{col}: 対応する開き括弧がありません"));
                }
                K::Sym(s)
            };
            out.push(Token { kind, line, col });
        }
        if depth == 0 {
            out.push(Token {
                kind: K::Newline,
                line,
                col: cs.len() + 1,
            });
        }
    }
    if depth != 0 {
        return Err("EOF: 括弧が閉じていません".into());
    }
    while indents.len() > 1 {
        indents.pop();
        out.push(Token {
            kind: K::Dedent,
            line: src.lines().count() + 1,
            col: 1,
        });
    }
    out.push(Token {
        kind: K::Eof,
        line: src.lines().count() + 1,
        col: 1,
    });
    Ok(out)
}
