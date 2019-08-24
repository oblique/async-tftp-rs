use bytes::BufMut;
use enum_primitive_derive::Primitive;
use std::convert::From;
use std::io;
use std::str;

use crate::error::*;
use crate::parse::*;

#[derive(Debug, Clone, Copy, PartialEq, Primitive)]
pub(crate) enum PacketType {
    Rrq = 1,
    Wrq = 2,
    Data = 3,
    Ack = 4,
    Error = 5,
    OAck = 6,
}

#[derive(Debug, Clone)]
pub enum TftpError {
    Msg(String),
    UnknownError,
    FileNotFound,
    PermissionDenied,
    DiskFull,
    IllegalOperation,
    UnknownTransferId,
    FileAlreadyExists,
    NoSuchUser,
}

#[derive(Debug)]
pub enum Packet<'a> {
    Rrq(RwReq),
    Wrq(RwReq),
    Data(u16, &'a [u8]),
    Ack(u16),
    Error(TftpError),
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
            Packet::Error(error) => {
                buf.put_u16_be(PacketType::Error as u16);
                buf.put_u16_be(error.code());
                buf.put(error.msg());
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

impl TftpError {
    pub(crate) fn from_code(code: u16, msg: Option<&str>) -> Self {
        match code {
            1 => TftpError::FileNotFound,
            2 => TftpError::PermissionDenied,
            3 => TftpError::DiskFull,
            4 => TftpError::IllegalOperation,
            5 => TftpError::UnknownTransferId,
            6 => TftpError::FileAlreadyExists,
            7 => TftpError::NoSuchUser,
            0 | _ => match msg {
                Some(msg) => TftpError::Msg(msg.to_string()),
                None => TftpError::UnknownError,
            },
        }
    }

    pub(crate) fn code(&self) -> u16 {
        match self {
            TftpError::Msg(..) => 0,
            TftpError::UnknownError => 0,
            TftpError::FileNotFound => 1,
            TftpError::PermissionDenied => 2,
            TftpError::DiskFull => 3,
            TftpError::IllegalOperation => 4,
            TftpError::UnknownTransferId => 5,
            TftpError::FileAlreadyExists => 6,
            TftpError::NoSuchUser => 7,
        }
    }

    pub fn msg(&self) -> &str {
        match self {
            TftpError::Msg(msg) => msg,
            TftpError::UnknownError => "Unknown error",
            TftpError::FileNotFound => "File not found",
            TftpError::PermissionDenied => "Permission denied",
            TftpError::DiskFull => "Disk is full",
            TftpError::IllegalOperation => "Illegal operation",
            TftpError::UnknownTransferId => "Unknown transfer ID",
            TftpError::FileAlreadyExists => "File already exists",
            TftpError::NoSuchUser => "No such user",
        }
    }
}

impl From<TftpError> for Packet<'_> {
    fn from(inner: TftpError) -> Self {
        Packet::Error(inner)
    }
}

impl From<io::Error> for TftpError {
    fn from(io_err: io::Error) -> Self {
        match io_err.kind() {
            io::ErrorKind::NotFound => TftpError::FileNotFound,
            io::ErrorKind::PermissionDenied => TftpError::PermissionDenied,
            io::ErrorKind::WriteZero => TftpError::DiskFull,
            io::ErrorKind::AlreadyExists => TftpError::FileAlreadyExists,
            _ => match io_err.raw_os_error() {
                Some(rc) => TftpError::Msg(format!("IO error: {}", rc)),
                None => TftpError::UnknownError,
            },
        }
    }
}

impl From<crate::Error> for TftpError {
    fn from(err: crate::Error) -> Self {
        match err {
            Error::Tftp(e) => e,
            Error::Io(e) => e.into(),
            Error::InvalidPacket => TftpError::IllegalOperation,
            _ => TftpError::UnknownError,
        }
    }
}
