use bytes::BytesMut;
use num_derive::FromPrimitive;
use std::convert::From;
use std::io;
use std::str;

use crate::bytes_ext::BytesMutExt;
use crate::error::*;
use crate::parse::*;

pub const PACKET_DATA_HEADER_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, FromPrimitive)]
#[repr(u16)]
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
    pub fn decode(data: &[u8]) -> Result<Packet> {
        parse_packet(data)
    }

    pub fn encode(&self, buf: &mut BytesMut) {
        match self {
            Packet::Rrq(req) => {
                buf.extend_u16_be(PacketType::Rrq as u16);
                buf.extend_buf(&req.filename);
                buf.extend_u8(0);
                buf.extend_buf(req.mode.to_str());
                buf.extend_u8(0);
                req.opts.encode(buf);
            }
            Packet::Wrq(req) => {
                buf.extend_u16_be(PacketType::Wrq as u16);
                buf.extend_buf(&req.filename);
                buf.extend_u8(0);
                buf.extend_buf(req.mode.to_str());
                buf.extend_u8(0);
                req.opts.encode(buf);
            }
            Packet::Data(block, data) => {
                buf.extend_u16_be(PacketType::Data as u16);
                buf.extend_u16_be(*block);
                buf.extend_buf(&data[..]);
            }
            Packet::Ack(block) => {
                buf.extend_u16_be(PacketType::Ack as u16);
                buf.extend_u16_be(*block);
            }
            Packet::Error(error) => {
                buf.extend_u16_be(PacketType::Error as u16);
                buf.extend_u16_be(error.code());
                buf.extend_buf(error.msg());
                buf.extend_u8(0);
            }
            Packet::OAck(opts) => {
                buf.extend_u16_be(PacketType::OAck as u16);
                opts.encode(buf);
            }
        }
    }

    pub fn encode_data_head(block_id: u16, buf: &mut BytesMut) {
        buf.extend_u16_be(PacketType::Data as u16);
        buf.extend_u16_be(block_id);
    }
}

impl Opts {
    fn encode(&self, buf: &mut BytesMut) {
        if let Some(block_size) = self.block_size {
            buf.extend_buf("blksize\0");
            buf.extend_buf(block_size.to_string());
            buf.extend_u8(0);
        }

        if let Some(timeout) = self.timeout {
            buf.extend_buf("timeout\0");
            buf.extend_buf(timeout.to_string());
            buf.extend_u8(0);
        }

        if let Some(transfer_size) = self.transfer_size {
            buf.extend_buf("tsize\0");
            buf.extend_buf(transfer_size.to_string());
            buf.extend_u8(0);
        }
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
