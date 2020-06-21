use crate::{
    intel::{
        instruction::Instruction,
        token::{self, *},
    },
    AssemblyError, ErrorCode,
};
use std::{collections::HashMap, sync::mpsc::Receiver};

enum ByteCode {
    Byte(u8),
    Addr((Token, String)),
}

pub fn eval(receiver: Receiver<Result<Token, EOL>>) -> Result<Vec<u8>, AssemblyError> {
    let mut byte_code = Vec::<ByteCode>::new();
    let mut buffer = Vec::new();
    let mut labels_idx = HashMap::new();
    for maybe_token in receiver.iter() {
        match maybe_token {
            Ok(token) => buffer.push(token),
            Err(_) => translate_buffer(&mut buffer, &mut byte_code, &mut labels_idx)?,
        }
    }
    fill_addresses(byte_code, &labels_idx)
}

macro_rules! throw {
    ($token:expr, $code:ident$(($($qty:ident),+))?) => {
        return Result::Err(AssemblyError::new($token, ErrorCode::$code$(($($qty),+))?));
    };
}

fn translate_buffer(
    buffer: &mut Vec<Token>,
    byte_code: &mut Vec<ByteCode>,
    labels_idx: &mut HashMap<String, u16>,
) -> Result<(), AssemblyError> {
    use TokenKind::*;
    // Inside the buffer we have a line of mpp assembly tokens,
    // which is structured as:
    // [label] [mnemonic [operands]]
    let mut operands_req = 0;
    let mut operands_found = 0;
    let mut operands: [Option<(Token, Primitive)>; 2] = [None, None];
    let mut stmt_mnemonic: Option<(Token, token::Mnemonic)> = None;
    for token in buffer.drain(..) {
        match &token.kind {
            Label(label) => {
                if labels_idx
                    .insert(label.clone(), byte_code.len() as u16)
                    .is_some()
                {
                    throw!(token, RedefinedLabel);
                }
            }
            Mnemonic(mnemonic) => {
                if stmt_mnemonic.is_some() {
                    throw!(token, MultipleMnemonics);
                }
                operands_req = mnemonic.operands_required();
                stmt_mnemonic = Some((token.clone(), *mnemonic));
            }
            Operand(primitive) => {
                operands[operands_found].replace((token.clone(), primitive.clone()));
                operands_found += 1;
            }
            Comma => match stmt_mnemonic {
                None => throw!(token, NoMnemonic),
                Some((ref mnemonic_token, _)) => {
                    if operands_req == operands_found {
                        throw!(mnemonic_token.clone(), ExcessiveOperands(operands_req));
                    } else if operands_found == 0 {
                        throw!(token.clone(), UnexpectedComma);
                    }
                }
            },
            Error => unreachable!("tried to parse bad token"),
        }
    }
    let (mnemonic_token, mnemonic) = match stmt_mnemonic {
        Some(pair) => pair,
        None => return Ok(()),
    };
    if operands_found != operands_req {
        throw!(
            mnemonic_token,
            NotEnoughOperands(operands_found, operands_req)
        )
    }
    let inst = Instruction::new().encode_mnemonic(mnemonic);
    match operands {
        // intel => dest, origin
        [Some((dest_token, dest)), Some((origin_token, origin))] => {
            match inst.try_encode_data_flow(&origin, &dest) {
                Ok(inst) => unsafe {
                    let bytes = inst.as_bytes().into_iter().map(ByteCode::Byte);
                    byte_code.extend(bytes);
                },
                Err(ErrorCode::BadOrigin) => throw!(origin_token, BadOrigin),
                Err(ErrorCode::BadDestination) => throw!(dest_token, BadDestination),
                Err(err) => unreachable!("unexpected data flow error: {:?}", err),
            }
        }
        // Flow control takes a single label operand
        [Some((label_dest_token, Primitive::Label(label))), None] => unsafe {
            let bytes = inst.as_bytes().into_iter().map(ByteCode::Byte);
            byte_code.extend(bytes);
            byte_code.push(ByteCode::Addr((label_dest_token, label)))
        },
        [Some(_), None] => throw!(mnemonic_token, NoLabel),
        [None, Some(_)] => unreachable!("primitive parsed out of order"),
        [None, None] => (),
    }
    Ok(())
}

fn fill_addresses(
    byte_code: Vec<ByteCode>,
    labels_idx: &HashMap<String, u16>,
) -> Result<Vec<u8>, AssemblyError> {
    let mut final_byte_code = Vec::with_capacity(byte_code.len());
    for maybe_byte in byte_code {
        match maybe_byte {
            ByteCode::Byte(byte) => final_byte_code.push(byte),
            ByteCode::Addr((token, label)) => {
                if let Some(word) = labels_idx.get(&label) {
                    let [hi, lo] = word.to_be_bytes();
                    final_byte_code.push(hi);
                    final_byte_code.push(lo);
                } else {
                    throw!(token, UnknownLabel(label))
                }
            }
        }
    }
    Ok(final_byte_code)
}
