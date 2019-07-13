use bytes::BufMut;
use enum_primitive_derive::Primitive;
use std::borrow::Cow;
use std::io;
use std::str;

use crate::error::*;
use crate::parse::*;

#[derive(Primitive)]
pub(crate) enum PacketType {
    Rrq = 1,
    Wrq = 2,
    Data = 3,
    Ack = 4,
    Error = 5,
    OAck = 6,
}

const ERR_NOT_DEFINED: u16 = 0;
const ERR_NOT_FOUNT: u16 = 1;
const ERR_PERM_DENIED: u16 = 2;
const ERR_FULL_DISK: u16 = 3;
const ERR_INVALID_OP: u16 = 4;
const ERR_ALREADY_EXISTS: u16 = 6;

#[derive(Debug, PartialEq)]
pub enum Packet<'a> {
    Rrq(RwReq),
    Wrq(RwReq),
    Data(u16, &'a [u8]),
    Ack(u16),
    Error(u16, Cow<'a, str>),
    OAck(Opts),
}

#[derive(Debug, PartialEq)]
pub enum Mode {
    Netascii,
    Octet,
    Mail,
}

#[derive(Debug, PartialEq)]
pub struct RwReq {
    pub filename: String,
    pub mode: Mode,
    pub opts: Opts,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opts {
    pub block_size: Option<u16>,
    pub timeout: Option<u8>,
    pub transfer_size: Option<u64>,
}

impl<'a> Packet<'a> {
    pub fn from_bytes(data: &[u8]) -> Result<Packet> {
        parse_packet(data)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        match self {
            Packet::Rrq(req) => {
                buf.put_u16_be(PacketType::Rrq as u16);
                buf.put(&req.filename);
                buf.put_u8(0);
                buf.put(req.mode.to_str());
                buf.put_u8(0);
                req.opts.encode(&mut buf);
            }
            Packet::Wrq(req) => {
                buf.put_u16_be(PacketType::Wrq as u16);
                buf.put(&req.filename);
                buf.put_u8(0);
                buf.put(req.mode.to_str());
                buf.put_u8(0);
                req.opts.encode(&mut buf);
            }
            Packet::Data(block, data) => {
                buf.put_u16_be(PacketType::Data as u16);
                buf.put_u16_be(*block);
                buf.put(data.as_ref());
            }
            Packet::Ack(block) => {
                buf.put_u16_be(PacketType::Ack as u16);
                buf.put_u16_be(*block);
            }
            Packet::Error(code, msg) => {
                buf.put_u16_be(PacketType::Error as u16);
                buf.put_u16_be(*code);
                buf.put(msg.as_ref());
                buf.put_u8(0);
            }
            Packet::OAck(opts) => {
                buf.put_u16_be(PacketType::OAck as u16);
                opts.encode(&mut buf);
            }
        }

        buf
    }
}

impl<'a> From<Error> for Packet<'a> {
    fn from(err: Error) -> Self {
        let (err_id, err_msg) = match err {
            Error::Io(err) => match err.kind() {
                io::ErrorKind::NotFound => {
                    (ERR_NOT_FOUNT, "File not found".into())
                }
                io::ErrorKind::PermissionDenied => {
                    (ERR_PERM_DENIED, "Access violation".into())
                }
                io::ErrorKind::WriteZero => {
                    (ERR_FULL_DISK, "Disk full or allocation exceeded".into())
                }
                io::ErrorKind::AlreadyExists => {
                    (ERR_ALREADY_EXISTS, "File already exists".into())
                }
                _ => match err.raw_os_error() {
                    Some(rc) => {
                        (ERR_NOT_DEFINED, format!("IO error: {}", rc).into())
                    }
                    None => (ERR_NOT_DEFINED, "Unknown IO error".into()),
                },
            },
            Error::InvalidPacket | Error::InvalidOperation => {
                (ERR_INVALID_OP, "Illegal TFTP operation".into())
            }
            _ => (ERR_NOT_DEFINED, "Unknown error".into()),
        };

        Packet::Error(err_id, err_msg)
    }
}

impl Opts {
    fn encode(&self, buf: &mut Vec<u8>) {
        if let Some(x) = self.block_size {
            buf.put("blksize\0");
            buf.put(x.to_string());
            buf.put_u8(0);
        }

        if let Some(x) = self.timeout {
            buf.put("timeout\0");
            buf.put(x.to_string());
            buf.put_u8(0);
        }

        if let Some(x) = self.transfer_size {
            buf.put("tsize\0");
            buf.put(x.to_string());
            buf.put_u8(0);
        }
    }

    pub fn all_none(&self) -> bool {
        *self == Opts::default()
    }
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
