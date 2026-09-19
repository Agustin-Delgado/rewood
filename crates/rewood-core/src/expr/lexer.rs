use super::ExprError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    /// Identifier, possibly dotted: `carcass.inner_width`.
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
    Comma,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
    And,
    Or,
    Not,
    True,
    False,
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, ExprError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let v: f64 = text
                    .parse()
                    .map_err(|_| ExprError::Lex(format!("número inválido '{text}'")))?;
                out.push(Token::Number(v));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                out.push(match text.as_str() {
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "true" => Token::True,
                    "false" => Token::False,
                    _ => Token::Ident(text),
                });
            }
            '+' => {
                out.push(Token::Plus);
                i += 1
            }
            '-' => {
                out.push(Token::Minus);
                i += 1
            }
            '*' => {
                out.push(Token::Star);
                i += 1
            }
            '/' => {
                out.push(Token::Slash);
                i += 1
            }
            '%' => {
                out.push(Token::Percent);
                i += 1
            }
            '(' => {
                out.push(Token::LParen);
                i += 1
            }
            ')' => {
                out.push(Token::RParen);
                i += 1
            }
            ',' => {
                out.push(Token::Comma);
                i += 1
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Token::Le);
                    i += 2
                } else {
                    out.push(Token::Lt);
                    i += 1
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Token::Ge);
                    i += 2
                } else {
                    out.push(Token::Gt);
                    i += 1
                }
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Token::EqEq);
                    i += 2
                } else {
                    return Err(ExprError::Lex(
                        "se esperaba '==' (un solo '=' no es una comparación)".into(),
                    ));
                }
            }
            '!' => {
                if chars.get(i + 1) == Some(&'=') {
                    out.push(Token::Ne);
                    i += 2
                } else {
                    out.push(Token::Not);
                    i += 1
                }
            }
            '&' if chars.get(i + 1) == Some(&'&') => {
                out.push(Token::And);
                i += 2
            }
            '|' if chars.get(i + 1) == Some(&'|') => {
                out.push(Token::Or);
                i += 2
            }
            other => return Err(ExprError::Lex(format!("carácter inesperado '{other}'"))),
        }
    }
    Ok(out)
}
