use enum_utils::FromStr;
use std::{ops::Range, str::FromStr, sync::mpsc::Sender};
use thiserror::Error;

#[derive(Copy, Clone, Error, Debug)]
pub enum TokenizingError {
    #[error("Unsupported architecture")]
    BadArchitecture,
    #[error("Malformed label")]
    BadLabel,
    #[error("Malformed memory location")]
    BadMemory,
    #[error("Malformed number")]
    BadNumber,
    #[error("Unsupported port")]
    BadPort,
    #[error("High byte used")]
    HighByte,
    #[error("Could not form a token")]
    UnknownToken,
}

impl TokenizingError {
    pub fn help_msg(self) -> &'static str {
        use TokenizingError::*;
        match self {
            BadArchitecture => "only 8-bits architecture is supported",
            BadLabel => "valid labels are formed by letters, numbers, and underscores; and may not start with numbers",
            BadMemory => "only number literals and registers may be memory locations",
            BadNumber => "number literals must start with a digit. Decimals may have a trailing `d`. Hexadecimals must either start with `0x` or end with an `h`; binaries with `0b` or `b`.",
            BadPort => "only I/O ports from 0 to 3 are currently supported",
            HighByte => "use the lower byte, by switching from `h` to `l`",
            UnknownToken => "???"
        }
    }
}

pub struct EOL;

pub type TokenSender = Sender<Result<Token, EOL>>;

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Range<usize>,
    pub line: usize,
}

impl Token {
    pub(crate) fn try_send(
        src: &mut String,
        span: Range<usize>,
        line: usize,
        channel: &TokenSender,
    ) -> Result<(), (Self, TokenizingError)> {
        if src.is_empty() {
            return Ok(());
        }
        let kind = src.parse().map_err(|err| {
            (
                Self {
                    kind: TokenKind::Error,
                    span: span.clone(),
                    line,
                },
                err,
            )
        })?;
        src.clear();
        channel
            .send(Ok(Self { kind, span, line }))
            .expect("parser stopped unexpectedly");
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum TokenKind {
    /// Any pattern that matches the regex `(?P<name>\w+):` with the `name`
    /// group being the contents of the inner `String`
    Label(String),
    /// See [`Mnemonic`](Mnemonic) for all available mnemonics
    Mnemonic(Mnemonic),
    /// See [`Primitive`](Primitive) for all available primitives
    Operand(Primitive),
    /// The operands' separator
    Comma,
    /// An unknown token
    Error,
}

impl FromStr for TokenKind {
    type Err = TokenizingError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        Ok(match src {
            "," => Self::Comma,
            _ if src.ends_with(':') => {
                if Primitive::is_label(&src[..src.len() - 1]) {
                    Self::Label(src.into())
                } else {
                    return Err(TokenizingError::BadLabel);
                }
            }
            _ => match src.parse() {
                Ok(mnemonic) => Self::Mnemonic(mnemonic),
                _ => Self::Operand(src.parse()?),
            },
        })
    }
}

#[derive(Copy, Clone, PartialEq, Debug, FromStr)]
#[enumeration(case_insensitive)]
pub enum Mnemonic {
    /// Adds a value to the accumulator and stores it in the destination
    Add,
    /// Subtracts a value to the accumulator and stores it in the destination
    Sub,
    /// Performs a bitwise or operation to the accumulator with a value and
    /// stores it in the destination
    Or,
    /// Performs a bitwise and operation to the accumulator with a value and
    /// stores it in the destination
    And,
    /// Performs a bitwise xor operation to the accumulator with a value and
    /// stores it in the destination
    Xor,
    /// Performs a bitwise not operation on a value and stores it in the
    /// destination
    Not,
    /// Copies a value to a destination
    Mov,
    /// Increments a value by `1` and stores it in the destination
    Inc,
    /// Redirects the flow of operation to a memory location
    Jmp,
    /// Redirects the flow of operation to a memory location if the `C` flag is
    /// high, i.e., a carry/borrow has occurred in the ALU
    #[enumeration(rename = "jc")]
    Jmpc,
    /// Redirects the flow of operation to a memory location if the `Z` flag is
    /// high, i.e., the result of an ALU operation was `0`
    #[enumeration(rename = "je", alias = "jz")]
    Jmpz,
    /// Stores the current memory location in the call stack and redirects the
    /// flow of operation to another memory location
    Call,
    /// Pops the call stack and returns to that memory location
    Ret,
    /// Pushes a register's value to the stack
    Push,
    /// Pops the stack and stores the value into a register
    Pop,
    /// Pushes the accumulator's value to the stack
    Pusha,
    /// Pops the stack and stores the value into the accumulator
    Popa,
}

impl Mnemonic {
    pub(crate) fn operands_required(self) -> usize {
        use Mnemonic::*;
        match self {
            Add | Sub | Or | And | Xor | Not | Mov | Inc => 2,
            Jmp | Jmpc | Jmpz | Call | Push | Pop => 1,
            _ => 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Port {
    /// Input ports 0 through 3
    Input(u8),
    /// Output ports 0 through 3
    Output(u8),
}

impl FromStr for Port {
    type Err = TokenizingError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let num = match src.chars().last() {
            Some(port @ '0'..='3') => port.to_digit(10).unwrap() as u8,
            Some('4'..='9') => return Err(TokenizingError::BadPort),
            _ => return Err(TokenizingError::UnknownToken),
        };
        Ok(match &src[..src.len() - 1] {
            "in" => Self::Input(num),
            "out" => Self::Output(num),
            _ => return Err(TokenizingError::UnknownToken),
        })
    }
}

impl Port {
    pub(super) fn port_number(self) -> u8 {
        match self {
            Self::Input(port) => port,
            Self::Output(port) => port,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Register {
    /// Register B `bl`
    B,
    /// Register C `cl`
    C,
    /// Register D `dl`
    D,
    /// Register E `el`
    E,
}

impl FromStr for Register {
    type Err = TokenizingError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        Ok(match src {
            "rbx" | "rcx" | "rdx" | "rex" | "ebx" | "ecx" | "edx" | "eex" | "bx" | "cx" | "dx"
            | "ex" => return Err(TokenizingError::BadArchitecture),
            "bh" | "ch" | "dh" | "eh" => return Err(TokenizingError::HighByte),
            "bl" => Self::B,
            "cl" => Self::C,
            "dl" => Self::D,
            "el" => Self::E,
            _ => return Err(TokenizingError::UnknownToken), // generic error
        })
    }
}

#[derive(Clone, Debug)]
pub enum Primitive {
    /// An ASCII character surrounded by single or double quotes, or a sequence
    /// that matches `[+-]?(\d+d?|\d[\da-f]*h|0x[\da-f]+|[01]+b|0b[01]+)`
    Number(u8),
    /// An I/O port
    Port(Port),
    /// A register
    Register(Register),
    /// The accumulator
    Accumulator,
    /// A numeric memory location: any `Self::Number` surrounded by square brackets
    Memory(u16),
    /// A dynamic memory location: a `Register` surrounded by square brackets
    DynamicMemory(Register),
    /// A dynamic memory location: the accumulator surrounded by square brackets
    DynamicMemoryAccumulator,
    /// A memory location label matching the regex `\w+`
    Label(String),
}

impl FromStr for Primitive {
    type Err = TokenizingError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        use TokenizingError::*;
        Ok(match src.as_bytes() {
            // Raw number
            [b'+', head, ..] | [b'-', head, ..] | [head, ..]
                if (*head as char).is_ascii_digit() =>
            {
                let num = try_to_number(src).ok_or(BadNumber)?;
                Self::Number(num)
            }
            // ASCII character
            [b'"', ch, b'"'] | [b'\'', ch, b'\''] if (*ch as char).is_ascii() => Self::Number(*ch),
            // Accumulator
            b"rax" | b"eax" | b"ax" => return Err(BadArchitecture),
            b"ah" => return Err(HighByte),
            b"al" => Self::Accumulator,
            // Memory location
            [b'[', mem @ .., b']'] => unsafe {
                // Given that `src` is a valid `&str`, therefore `mem` must be
                // as well, making this operation safe
                match std::str::from_utf8_unchecked(mem).parse::<Self>() {
                    Ok(Self::Number(byte)) => Self::Memory(byte as u16),
                    Ok(Self::Register(reg)) => Self::DynamicMemory(reg),
                    Ok(Self::Accumulator) => Self::DynamicMemoryAccumulator,
                    _ => return Err(BadMemory),
                }
            },
            // Other
            _ => match src.parse() {
                // Port
                Ok(port) => Self::Port(port),
                Err(BadPort) => return Err(BadPort), // relevant port error
                // Register
                _ => match src.parse() {
                    Ok(reg) => Self::Register(reg),
                    Err(UnknownToken) if Self::is_label(src) => Self::Label(src.into()),
                    Err(UnknownToken) => return Err(BadLabel),
                    Err(err) => return Err(err),
                },
            },
        })
    }
}

impl Primitive {
    fn is_label(src: &str) -> bool {
        if src.starts_with(|ch: char| ch.is_ascii_digit()) {
            return false;
        }
        src.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    }
}

fn try_to_number(mut src: &str) -> Option<u8> {
    fn fold_byte(src: &[u8], radix: u32) -> Option<u8> {
        use std::convert::TryInto;
        let mut num = 0;
        for &byte in src {
            num = num * radix + (byte as char).to_digit(radix)?;
        }
        num.try_into().ok()
    }
    let is_complement = src.starts_with('-');
    if is_complement || src.starts_with('+') {
        src = &src[1..];
    }
    let byte = match src.as_bytes() {
        [head @ .., b'b'] => fold_byte(head, 2),
        [head @ .., b'd'] => fold_byte(head, 10),
        [head @ .., b'h'] => fold_byte(head, 16),
        [b'0', b'b', tail @ ..] => fold_byte(tail, 2),
        [b'0', b'x', tail @ ..] => fold_byte(tail, 16),
        _ => None,
    };
    if is_complement {
        byte.map(|b| b.overflowing_neg().0)
    } else {
        byte
    }
}
