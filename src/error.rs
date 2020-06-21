use crate::intel::token::{Token, TokenizingError};
use std::{fmt, path::Path};
use thiserror::Error;

const TAB_SIZE: usize = 4;

#[derive(Debug, Error)]
pub enum ErrorCode {
    #[error("Invalid data origin")]
    BadOrigin,
    #[error("Invalid data destination")]
    BadDestination,
    #[error("Too many operands found")]
    ExcessiveOperands(usize),
    #[error("Multiple mnemonics in a single statement")]
    MultipleMnemonics,
    #[error("Destination label not found")]
    NoLabel,
    #[error("No mnemonic found")]
    NoMnemonic,
    #[error("Too few operands provided")]
    NotEnoughOperands(usize, usize),
    #[error("Redefined label")]
    RedefinedLabel,
    #[error("Unexpected comma")]
    UnexpectedComma,
    #[error("Unexpected label")]
    UnexpectedLabel,
    #[error("Undefined label")]
    UnknownLabel(String),
    #[error(transparent)]
    Token(#[from] TokenizingError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl ErrorCode {
    fn help_msg(&self) -> Vec<String> {
        use ErrorCode::*;
        vec![match self {
            BadOrigin | BadDestination => {
                return vec![
                    "valid data flows are: `ROM -> Acc | Register | RAM`,".into(),
                    "`Acc -> Acc | Register | RAM | Output`,".into(),
                    "`Register -> Acc`,".into(),
                    "`RAM -> Acc`,".into(),
                    "and `Input -> Acc`".into(),
                ];
            }
            ExcessiveOperands(req) => format!(
                "only {} operand{} required",
                req,
                if *req > 1 { "s are" } else { " is" }
            ),
            MultipleMnemonics => "remove this mnemonic".into(),
            NoLabel => "add a label operand".into(),
            NoMnemonic => "add a mnemonic".into(),
            NotEnoughOperands(found, req) => {
                let amt = req - found;
                format!("add {} operand{}", amt, if amt > 1 { "s" } else { "" })
            }
            RedefinedLabel => "remove this label or rename it".into(),
            UnexpectedComma => "remove this comma".into(),
            UnexpectedLabel => "this mnemonic does not accept labels".into(),
            UnknownLabel(label) => format!(
                "add this label somewhere either before a mnemonic, or alone, as `{}:`",
                label
            ),
            Token(err) => err.help_msg().into(),
            Io(io) => io.to_string(),
        }]
    }
}

#[derive(Debug, Error)]
pub struct AssemblyError {
    pub token: Token,
    #[source]
    pub code: ErrorCode,
}

impl fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)
    }
}

impl AssemblyError {
    pub fn new(token: Token, code: ErrorCode) -> Self {
        Self { token, code }
    }

    pub fn throw<P>(&self, src: &str, src_path: &P, note: Option<&str>)
    where
        P: AsRef<Path>,
    {
        let mut err_col = self.token.span.start;
        let line_src = src
            .lines()
            .nth(self.token.line - 1)
            .expect("error line could not be found");
        let mut line = String::with_capacity(line_src.len());
        // Replace tabs with spaces
        for ch in line_src.chars() {
            if ch == '\t' {
                let amt = TAB_SIZE - ((line.len() + 1) % TAB_SIZE);
                line.extend(std::iter::repeat('\x20').take(amt));
                if err_col == self.token.span.start {
                    err_col += amt;
                }
            } else {
                line.push(ch);
            }
        }
        let ruler_width = (self.token.line as f64).log10() as usize + 1;
        let help_msg = self.code.help_msg();
        eprintln!(
            "\
        {err_msg} @ {file_name}:{line_no}:{col_no}\n\
		{line_no:width$} │ {line}\n\
		{spacing:width$} │ {spacing:col_pad$}{indicator:^<indicator_width$} help: {help_msg}\
		",
            err_msg = self.code,
            file_name = src_path.as_ref().to_string_lossy(),
            line_no = self.token.line,
            col_no = err_col,
            line = line,
            spacing = "",
            width = ruler_width,
            col_pad = err_col - 1,
            indicator = "",
            indicator_width = self.token.span.len(),
            help_msg = help_msg[0]
        );
        for msg in help_msg.iter().skip(1) {
            eprintln!(
                "{spacing:width$} │ {spacing:col_pad$}{pad:pad_width$}{help_msg}",
                spacing = "",
                width = ruler_width,
                col_pad = err_col - 1,
                pad = "",
                pad_width = self.token.span.len() + 7,
                help_msg = msg
            );
        }
        if let Some(note) = note {
            eprintln!(
                "{:width$} = note: {note}",
                "",
                width = ruler_width,
                note = note
            );
        }
    }
}
