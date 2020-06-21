//! An instruction is conceived in assembly language as a
//! [`Mnemonic`](super::token::Mnemonic) followed by up to two
//! [`Primitive`](super::token::Primitive) operands and fully translates to up
//! to several bytes.
//!
//! # Full instruction layout
//!
//! Some instructions have the same bit layout, so a decoder is employed to
//! disambiguate them. Each set of them is stored in "pages", and the special
//! instruction `0x07` turns the decoder's pages ín order to correctly interpret
//! the main instruction. The decoder always starts at page `0`.
//!
//! The main instruction byte follows the page turns, and is divided in three
//! segments:
//!
//! | Bits    | 7 6 5 |      4 3      |   2 1 0   |
//! |---------|-------|---------------|-----------|
//! | Segment |  ALU  | Port/Register | Data flow |
//!
//! Then, a single byte for any literal (ROM) value used follows, and, lastly,
//! two bytes for a memory location (RAM).
//!
//! # Arithmetic Logic Unit (ALU)
//!
//! The ALU has eight possible operations, and its encoding does not clash with
//! any other instruction, so the decoder is not necessary to disambiguate the
//! first three bits where they are stored. These operations take two operands.
//!
//! | [`Mnemonic`] | Encoding |
//! |--------------|----------|
//! |     Add      |   000    |
//! |     Sub      |   001    |
//! |     And      |   010    |
//! |     Or       |   011    |
//! |     Xor      |   100    |
//! |     Not      |   101    |
//! |     Mov      |   110    |
//! |     Inc      |   111    |
//!
//! # Ports and Registers
//!
//! Data can flow to and from [`Port`](super::token::Port)s and
//! [`Register`](super::token::Register)s, and, aside from the accumulator,
//! are in the middle bits of the main instruction. This is possible because the
//! architecture of the microprocessor does not support flow between more than
//! one [`Port`] or [`Register`], requiring a middle storage in memory or the
//! accumulator if that end effect is desired.
//!
//! |  Storage   | Encoding |
//! |------------|----------|
//! | B/In0/Out0 |    00    |
//! | C/In1/Out1 |    01    |
//! | D/In2/Out2 |    10    |
//! | E/In3/Out3 |    11    |
//!
//! # Data Flow
//!
//! This is the main source of instructions' need for disambiguation. The number
//! of possibilities for data to transit far exceeds the maximum that three bits
//! can storage, making the decoder's existance necessarity.
//!
//! | Data Flow       | Encoding | Decoder Page |
//! |-----------------|----------|--------------|
//! | Acc -> Acc      |   000    |      0       |
//! | Acc -> Register |   001    |      0       |
//! | Acc -> RAM      |   010    |      0       |
//! | Acc -> Output   |   011    |      0       |
//! | Register -> Acc |   100    |      0       |
//! | RAM -> Acc      |   101    |      0       |
//! | Input -> Acc    |   110    |      0       |
//! | ROM -> Acc      |   000    |      1       |
//! | ROM -> Register |   001    |      1       |
//! | ROM -> RAM      |   010    |      1       |
//! | Jmp             |   011    |      1       |
//! | Jmpc            |   100    |      1       |
//! | Jmpz            |   101    |      1       |
//! | Call            |   110    |      1       |
//! | Ret             |   000    |      2       |
//! | DyRAM -> Acc    |   001    |      2       |
//! | Acc -> DyRam    |   010    |      2       |
//! | Push            |   011    |      2       |
//! | Pop             |   100    |      2       |
//! | Pusha           |   101    |      2       |
//! | Popa            |   110    |      2       |

use super::token::*;
use crate::ErrorCode;

/// The container for a full instruction set. See the [module's](self) documentation for
/// a more detailed description.
#[derive(Debug, Default)]
pub struct Instruction {
    decoder_page: usize,
    main: u8,
    rom: Option<u8>,
    ram: Option<u16>,
}

impl Instruction {
    const DECODER_PAGE_TURN: u8 = 0b_0000_0111;

    /// Returns a new and unencoded `Instruction`.
    pub const fn new() -> Self {
        Self {
            decoder_page: 0,
            main: 0,
            rom: None,
            ram: None,
        }
    }

    fn encode_main(mut self, and: u8, or: u8) -> Self {
        self.main &= and;
        self.main |= or;
        self
    }

    fn encode_register(self, reg: Register) -> Self {
        use Register::*;
        let (and, or) = match reg {
            B => (0b_111_00_111, 0b_000_00_000),
            C => (0b_111_01_111, 0b_000_01_000),
            D => (0b_111_10_111, 0b_000_10_000),
            E => (0b_111_11_111, 0b_000_11_000),
        };
        self.encode_main(and, or)
    }

    fn encode_port(mut self, port: Port) -> Self {
        self = self.encode_port_number(port);
        // Encode the only data flows allowed with ports
        // They're both in the first page of the decoder
        let (and, or) = match port {
            Port::Output(_) => (0b_111_11_011, 0b_000_00_011),
            Port::Input(_) => (0b_111_11_110, 0b_000_00_110),
        };
        self.encode_main(and, or)
    }

    fn encode_port_number(self, port: Port) -> Self {
        let port_no = port.port_number();
        let (and, or) = match port_no {
            0 => (0b_111_00_111, 0b_000_00_000),
            1 => (0b_111_01_111, 0b_000_01_000),
            2 => (0b_111_10_111, 0b_000_10_000),
            3 => (0b_111_11_111, 0b_000_11_000),
            _ => unreachable!("attempted to encode invalid I/O port: {:?}", port),
        };
        self.encode_main(and, or)
    }

    /// Encodes an instruction according to its associated
    /// [`Mnemonic`](super::token::Mnemonic).
    ///
    /// # Return
    ///
    /// This function will return an `Instruction` that may still require its
    /// data flow to be encoded.
    ///
    /// # Examples
    ///
    /// TODO: add example where the further encoding is needed
    pub fn encode_mnemonic(mut self, mnemonic: Mnemonic) -> Self {
        use Mnemonic::*;
        let (and, or, page) = match mnemonic {
            // ALU
            Add => (0b_000_11_111, 0b_000_00_000, 0),
            Sub => (0b_001_11_111, 0b_001_00_000, 0),
            And => (0b_010_11_111, 0b_010_00_000, 0),
            Or => (0b_011_11_111, 0b_011_00_000, 0),
            Xor => (0b_100_11_111, 0b_100_00_000, 0),
            Not => (0b_101_11_111, 0b_101_00_000, 0),
            Mov => (0b_110_11_111, 0b_110_00_000, 0),
            Inc => (0b_111_11_111, 0b_111_00_000, 0),
            // Flow control
            Jmp => (0b_111_11_011, 0b_000_00_011, 1),
            Jmpc => (0b_111_11_100, 0b_000_00_100, 1),
            Jmpz => (0b_111_11_101, 0b_000_00_101, 1),
            Call => (0b_111_11_110, 0b_000_00_110, 1),
            Ret => (0b_111_11_000, 0b_000_00_000, 2),
            Push => (0b_111_11_011, 0b_000_00_011, 2),
            Pop => (0b_111_11_100, 0b_000_00_100, 2),
            Pusha => (0b_111_11_101, 0b_000_00_101, 2),
            Popa => (0b_111_11_110, 0b_000_00_110, 2),
        };
        self.decoder_page = page;
        self.encode_main(and, or)
    }

    /// Tries to encode the data flow of the instruction.
    ///
    /// # Return
    ///
    /// This function will return an `Instruction` that may still require its
    /// mnemonic to be encoded.
    ///
    /// # Examples
    ///
    /// TODO: example where further encoding is necessary
    pub fn try_encode_data_flow(
        mut self,
        origin: &Primitive,
        dest: &Primitive,
    ) -> Result<Instruction, ErrorCode> {
        use ErrorCode::*;
        let (and, or, page) = match origin {
            // Accumulator origin
            Primitive::Accumulator => match dest {
                Primitive::Accumulator => (0b_111_11_000, 0b_000_00_000, 0),
                Primitive::Register(reg) => {
                    self = self.encode_register(*reg);
                    (0b_111_11_001, 0b_000_00_001, 0)
                }
                Primitive::Memory(ram) => {
                    self.ram = Some(*ram);
                    (0b_111_11_010, 0b_000_00_010, 0)
                }
                Primitive::Port(out @ Port::Output(_)) => {
                    self = self.encode_port(*out);
                    (0b_111_11_011, 0b_000_00_011, 0)
                }
                Primitive::DynamicMemory(reg) => {
                    self = self.encode_register(*reg);
                    (0b_111_11_010, 0b_000_00_010, 2)
                }
                _ => return Err(BadDestination),
            },

            // Register origin
            Primitive::Register(reg) => match dest {
                Primitive::Accumulator => {
                    self = self.encode_register(*reg);
                    (0b_111_11_100, 0b_000_00_100, 0)
                }
                _ => return Err(BadDestination),
            },

            // Memory location origin
            Primitive::Memory(ram) => {
                self.ram = Some(*ram);
                match dest {
                    Primitive::Accumulator => (0b_111_11_101, 0b_000_00_101, 0),
                    _ => return Err(BadDestination),
                }
            }

            // Input origin
            Primitive::Port(input @ Port::Input(_)) => {
                self = self.encode_port(*input);
                match dest {
                    Primitive::Accumulator => (0b_111_11_110, 0b_000_00_110, 0),
                    _ => return Err(BadDestination),
                }
            }

            // Literal origin
            Primitive::Number(rom) => {
                self.rom = Some(*rom);
                match dest {
                    Primitive::Accumulator => (0b_111_11_000, 0b_000_00_000, 1),
                    Primitive::Register(reg) => {
                        self = self.encode_register(*reg);
                        (0b_111_11_001, 0b_000_00_001, 1)
                    }
                    Primitive::Memory(ram) => {
                        self.ram = Some(*ram);
                        (0b_111_11_010, 0b_000_00_010, 1)
                    }
                    _ => return Err(BadDestination),
                }
            }

            // Dynamic memory origin
            Primitive::DynamicMemory(reg) => {
                self = self.encode_register(*reg);
                match dest {
                    Primitive::Accumulator => (0b_111_11_001, 0b_000_00_001, 2),
                    _ => return Err(BadDestination),
                }
            }

            // Other origins
            _ => return Err(BadOrigin),
        };
        self.decoder_page = page;
        Ok(self.encode_main(and, or))
    }

    /// Return the underlaying bytes corresponding to this `Instruction`.
    ///
    /// # Safety
    ///
    /// No guarantees are given that this `Instruction` is fully formed, i.e.,
    /// it went through all the encoding steps required. Before calling this,
    /// make sure it was properly encoded by calling both functions beforehand:
    /// [`Self::encode_mnemonic`](Self::encode_mnemonic) into
    /// [`Self::try_encode_data_flow`](Self::try_encode_data_flow)
    ///
    /// # Examples
    ///
    /// TODO: example where the correct chain is followed
    pub unsafe fn as_bytes(self) -> Vec<u8> {
        use std::iter;
        let page = self.decoder_page;
        let mut vec = Vec::with_capacity(page + 1);
        vec.extend(iter::repeat(Self::DECODER_PAGE_TURN).take(page));
        vec.push(self.main);
        if let Some(byte) = self.rom {
            vec.push(byte);
        }
        if let Some(word) = self.ram {
            let bytes = word.to_be_bytes();
            vec.reserve(2);
            vec.push(bytes[0]);
            vec.push(bytes[1]);
        }
        vec
    }
}
