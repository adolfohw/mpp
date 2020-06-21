use crate::{error::AssemblyError, lexer, parser};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

pub struct Assembly {
    data: Vec<u8>,
    path: Option<PathBuf>,
}

impl Assembly {
    pub fn assemble(src: String) -> Result<Self, AssemblyError> {
        let (sender, receiver) = mpsc::channel();
        let lexer = thread::spawn(move || lexer::scan(&src, sender));
        let data = parser::eval(receiver)?;
        lexer.join().expect("lexer stopped unexpectedly")?;
        Ok(Assembly { data, path: None })
    }

    pub fn from_path<P>(path: P) -> Result<Self, AssemblyError>
    where
        P: AsRef<Path>,
    {
        Self::assemble(fs::read_to_string(path).unwrap())
    }

    pub fn to_logisim(&mut self) -> &mut Self {
        let mut vec = Vec::with_capacity(self.data.len() * 3 + 10);
        vec.extend_from_slice(b"v2.0 raw\r\n");
        for &byte in &self.data {
            let (hi, lo) = byte_as_hexadecimal(byte);
            if hi != b'0' {
                vec.push(hi as u8);
            }
            vec.push(lo as u8);
            vec.push(0x20);
        }
        self.data = vec;
        self
    }

    pub fn then_save_as<P>(&mut self, path: P) -> &mut Self
    where
        P: AsRef<Path>,
    {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn discard_save_path(&mut self) -> &mut Self {
        self.path = None;
        self
    }

    pub fn as_byte_code(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn as_mut_byte_code(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
}

impl AsRef<[u8]> for Assembly {
    fn as_ref(&self) -> &[u8] {
        self.as_byte_code()
    }
}

impl AsMut<[u8]> for Assembly {
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_mut_byte_code()
    }
}

impl Drop for Assembly {
    #[allow(unused_must_use)]
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            fs::write(path, self);
        }
    }
}

impl fmt::Debug for Assembly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.fmt(f)
    }
}

impl PartialEq for Assembly {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

fn byte_as_hexadecimal(byte: u8) -> (u8, u8) {
    (nibble_to_ascii(byte >> 4), nibble_to_ascii(byte & 0xF))
}

fn nibble_to_ascii(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        0xA..=0xF => b'a' + nibble - 0xA,
        _ => unreachable!("byte too large to represent a nibble"),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! assert_asm {
        ($inst:literal, $translation:tt) => {
            assert_eq!(
                Assembly::assemble($inst.into()).unwrap().as_ref(),
                $translation
            )
        };
    }

    #[test]
    fn test_instructions() {
        assert_asm!("_start:	jz	_start", [7, 5, 0, 0])
    }
}
