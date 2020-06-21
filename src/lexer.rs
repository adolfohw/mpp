use crate::intel::token::*;
use crate::{AssemblyError, ErrorCode};

pub fn scan(src: &str, channel: TokenSender) -> Result<(), AssemblyError> {
    let mut token = String::new();
    for (mut line_no, line) in src.lines().enumerate() {
        line_no += 1;
        // We chain as to always pack the token at the end of a line
        for (col, ch) in line.chars().chain(std::iter::once(' ')).enumerate() {
            let span = col - token.len()..col;
            let mut skip_rest_of_line = false;
            let attempt = match ch {
                ';' => {
                    skip_rest_of_line = true;
                    Token::try_send(&mut token, span, line_no, &channel)
                }
                ',' => {
                    let res = Token::try_send(&mut token, span, line_no, &channel);
                    token.push(ch);
                    Token::try_send(&mut token, col - 1..col, line_no, &channel).unwrap();
                    res
                }
                _ if ch.is_whitespace() => Token::try_send(&mut token, span, line_no, &channel),
                _ => Ok(token.push(ch.to_ascii_lowercase())),
            };
            attempt.map_err(|(token, err)| AssemblyError::new(token, ErrorCode::Token(err)))?;
            if skip_rest_of_line {
                break;
            }
        }
        channel.send(Err(EOL)).expect("parser stopped unexpectedly");
    }
    Ok(())
}
