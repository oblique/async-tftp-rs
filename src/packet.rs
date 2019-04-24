// Clippy gives us a false possitive on FromPrimitive/ToPrimitive derives.
// This is fixed on rust-clippy#3932.
// TODO: remove this on the next clippy release.
#![allow(clippy::useless_attribute)]

use bytes::Buf;
use failure::ResultExt;
use memchr::memchr;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use std::io::Cursor;
use std::str::{self, FromStr};

use crate::error::*;

#[derive(Debug)]
pub enum Packet {
    Rrq(String, Mode),
    Wrq(String, Mode),
    Data(u16, Vec<u8>),
    Ack(u16),
    Error(u16, String),
}

#[derive(Debug)]
pub enum Mode {
    Netascii,
    Octet,
    Mail,
}

#[derive(Debug, FromPrimitive, ToPrimitive)]
pub enum OpCode {
    Rrq = 1,
    Wrq = 2,
    Data = 3,
    Ack = 4,
    Error = 5,
}

impl Mode {
    pub fn to_str(&self) -> &'static str {
        match self {
            Mode::Netascii => "netascii",
            Mode::Octet => "octet",
            Mode::Mail => "mail",
        }
    }
}

impl FromStr for Mode {
    type Err = Error;

    fn from_str(s: &str) -> StdResult<Self, Self::Err> {
        match s.to_owned().to_lowercase().as_str() {
            "netascii" => Ok(Mode::Netascii),
            "octet" => Ok(Mode::Octet),
            "mail" => Ok(Mode::Mail),
            _ => Err(ErrorKind::DecodeError("Invalid mode").into()),
        }
    }
}

fn read_string(buf: &mut Cursor<&[u8]>) -> Result<String> {
    let b = buf.bytes();

    let pos =
        memchr(0, b).ok_or_else(|| Error::from(ErrorKind::DecodeError("No string ending")))?;

    let s = str::from_utf8(&b[..pos])
        .context(ErrorKind::DecodeError("Invalid UTF-8 string"))?
        .to_string();

    buf.advance(pos + 1);

    Ok(s)
}

impl Packet {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut buf = Cursor::new(bytes);

        if buf.remaining() < 2 {
            return Err(ErrorKind::DecodeError("Insufficient packet length").into());
        }

        match FromPrimitive::from_u16(buf.get_u16_be()) {
            Some(OpCode::Rrq) => {
                let filename = read_string(&mut buf)?;
                let mode = Mode::from_str(read_string(&mut buf)?.as_str())?;
                Ok(Packet::Rrq(filename, mode))
            }
            Some(OpCode::Wrq) => {
                let filename = read_string(&mut buf)?;
                let mode = Mode::from_str(read_string(&mut buf)?.as_str())?;
                Ok(Packet::Wrq(filename, mode))
            }
            Some(OpCode::Data) => {
                if buf.remaining() < 2 {
                    return Err(ErrorKind::DecodeError("Insufficient packet length").into());
                }
                let block_nr = buf.get_u16_be();
                let data = buf.collect();
                Ok(Packet::Data(block_nr, data))
            }
            Some(OpCode::Ack) => {
                if buf.remaining() < 2 {
                    return Err(ErrorKind::DecodeError("Insufficient packet length").into());
                }
                let block_nr = buf.get_u16_be();
                Ok(Packet::Ack(block_nr))
            }
            Some(OpCode::Error) => {
                if buf.remaining() < 2 {
                    return Err(ErrorKind::DecodeError("Insufficient packet length").into());
                }
                let code = buf.get_u16_be();
                let msg = read_string(&mut buf)?;
                Ok(Packet::Error(code, msg))
            }
            None => Err(ErrorKind::DecodeError("Invalid opcode").into()),
        }
    }

    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        let mut buf = Vec::new();

        match self {
            Packet::Rrq(filename, mode) => {
                let opcode = ToPrimitive::to_u16(&OpCode::Rrq)?.to_be_bytes();
                buf.extend_from_slice(&opcode[..]);
                buf.extend_from_slice(filename.as_bytes());
                buf.push(0);
                buf.extend_from_slice(mode.to_str().as_bytes());
                buf.push(0);
                Some(buf)
            }
            Packet::Wrq(filename, mode) => {
                let opcode = ToPrimitive::to_u16(&OpCode::Wrq)?.to_be_bytes();
                buf.extend_from_slice(&opcode[..]);
                buf.extend_from_slice(filename.as_bytes());
                buf.push(0);
                buf.extend_from_slice(mode.to_str().as_bytes());
                buf.push(0);
                Some(buf)
            }
            _ => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::IntoBuf;

    #[test]
    fn check() {
        let mut b = b"abc\0def\0".into_buf();

        assert_eq!(read_string(&mut b), Ok("abc".to_string()));
        assert_eq!(read_string(&mut b), Ok("def".to_string()));

        //let b = Packet::from_buf(vec![1u8]).expect("XXX");
    }
}
