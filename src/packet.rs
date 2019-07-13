use bytes::BufMut;
use std::borrow::Cow;
use std::io;
use std::str::{self, FromStr};

use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_till};
use nom::combinator::{map, map_opt, map_res, rest};
use nom::multi::many0;
use nom::number::complete::be_u16;
use nom::sequence::tuple;
use nom::IResult;

use crate::error::*;

const RRQ: u16 = 1;
const WRQ: u16 = 2;
const DATA: u16 = 3;
const ACK: u16 = 4;
const ERROR: u16 = 5;
const OACK: u16 = 6;

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

#[derive(Debug)]
enum Opt<'a> {
    BlkSize(u16),
    Timeout(u8),
    Tsize(u64),
    Invalid(&'a str, &'a str),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opts {
    pub block_size: Option<u16>,
    pub timeout: Option<u8>,
    pub transfer_size: Option<u64>,
}

fn nul_str(input: &[u8]) -> IResult<&[u8], &str> {
    map_res(
        tuple((take_till(|c| c == b'\0'), tag(b"\0"))),
        |(s, _): (&[u8], _)| str::from_utf8(s),
    )(input)
}

fn parse_mode(input: &[u8]) -> IResult<&[u8], Mode> {
    alt((
        map(tag_no_case(b"netascii\0"), |_| Mode::Netascii),
        map(tag_no_case(b"octet\0"), |_| Mode::Octet),
        map(tag_no_case(b"mail\0"), |_| Mode::Mail),
    ))(input)
}

fn parse_opt_blksize(input: &[u8]) -> IResult<&[u8], Opt> {
    map_opt(tuple((tag_no_case(b"blksize\0"), nul_str)), |(_, n): (_, &str)| {
        u16::from_str(n)
            .ok()
            .filter(|n| *n >= 8 && *n <= 65464)
            .map(Opt::BlkSize)
    })(input)
}

fn parse_opt_timeout(input: &[u8]) -> IResult<&[u8], Opt> {
    map_opt(tuple((tag_no_case(b"timeout\0"), nul_str)), |(_, n): (_, &str)| {
        u8::from_str(n).ok().filter(|n| *n >= 1).map(Opt::Timeout)
    })(input)
}

fn parse_opt_tsize(input: &[u8]) -> IResult<&[u8], Opt> {
    map_opt(tuple((tag_no_case(b"tsize\0"), nul_str)), |(_, n): (_, &str)| {
        u64::from_str(n).ok().map(Opt::Tsize)
    })(input)
}

fn parse_opts(input: &[u8]) -> IResult<&[u8], Opts> {
    many0(alt((
        parse_opt_blksize,
        parse_opt_timeout,
        parse_opt_tsize,
        map(tuple((nul_str, nul_str)), |(k, v)| Opt::Invalid(k, v)),
    )))(input)
    .map(|(i, opt_vec)| (i, to_opts(opt_vec)))
}

fn to_opts(opt_vec: Vec<Opt>) -> Opts {
    let mut opts = Opts::default();

    for opt in opt_vec {
        match opt {
            Opt::BlkSize(size) => {
                if opts.block_size.is_none() {
                    opts.block_size.replace(size);
                }
            }
            Opt::Timeout(timeout) => {
                if opts.timeout.is_none() {
                    opts.timeout.replace(timeout);
                }
            }
            Opt::Tsize(size) => {
                if opts.transfer_size.is_none() {
                    opts.transfer_size.replace(size);
                }
            }
            Opt::Invalid(..) => {}
        }
    }

    opts
}

fn parse_rrq(input: &[u8]) -> IResult<&[u8], Packet> {
    let (input, (filename, mode, opts)) =
        tuple((nul_str, parse_mode, parse_opts))(input)?;

    Ok((
        input,
        Packet::Rrq(RwReq {
            filename: filename.to_owned(),
            mode,
            opts,
        }),
    ))
}

fn parse_wrq(input: &[u8]) -> IResult<&[u8], Packet> {
    let (input, (filename, mode, opts)) =
        tuple((nul_str, parse_mode, parse_opts))(input)?;

    Ok((
        input,
        Packet::Wrq(RwReq {
            filename: filename.to_owned(),
            mode,
            opts,
        }),
    ))
}

fn parse_data(input: &[u8]) -> IResult<&[u8], Packet> {
    tuple((be_u16, rest))(input)
        .map(|(i, (block_nr, data))| (i, Packet::Data(block_nr, data)))
}

fn parse_ack(input: &[u8]) -> IResult<&[u8], Packet> {
    be_u16(input).map(|(i, block_nr)| (i, Packet::Ack(block_nr)))
}

fn parse_error(input: &[u8]) -> IResult<&[u8], Packet> {
    tuple((be_u16, nul_str))(input)
        .map(|(i, (code, msg))| (i, Packet::Error(code, Cow::Borrowed(msg))))
}

fn parse_oack(input: &[u8]) -> IResult<&[u8], Packet> {
    parse_opts(input).map(|(i, opts)| (i, Packet::OAck(opts)))
}

fn parse_packet(input: &[u8]) -> Result<Packet> {
    let (rest, packet) = match be_u16(input)? {
        (data, RRQ) => parse_rrq(data)?,
        (data, WRQ) => parse_wrq(data)?,
        (data, DATA) => parse_data(data)?,
        (data, ACK) => parse_ack(data)?,
        (data, ERROR) => parse_error(data)?,
        (data, OACK) => parse_oack(data)?,
        _ => return Err(Error::InvalidPacket),
    };

    if rest.len() > 0 {
        Err(Error::InvalidPacket)
    } else {
        Ok(packet)
    }
}

impl<'a> Packet<'a> {
    pub fn from_bytes(data: &[u8]) -> Result<Packet> {
        parse_packet(data)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        match self {
            Packet::Rrq(req) => {
                buf.put_u16_be(RRQ);
                buf.put(&req.filename);
                buf.put_u8(0);
                buf.put(req.mode.to_str());
                buf.put_u8(0);
                req.opts.encode(&mut buf);
            }
            Packet::Wrq(req) => {
                buf.put_u16_be(WRQ);
                buf.put(&req.filename);
                buf.put_u8(0);
                buf.put(req.mode.to_str());
                buf.put_u8(0);
                req.opts.encode(&mut buf);
            }
            Packet::Data(block, data) => {
                buf.put_u16_be(DATA);
                buf.put_u16_be(*block);
                buf.put(data.as_ref());
            }
            Packet::Ack(block) => {
                buf.put_u16_be(ACK);
                buf.put_u16_be(*block);
            }
            Packet::Error(code, msg) => {
                buf.put_u16_be(ERROR);
                buf.put_u16_be(*code);
                buf.put(msg.as_ref());
                buf.put_u8(0);
            }
            Packet::OAck(opts) => {
                buf.put_u16_be(OACK);
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

#[cfg(test)]
mod tests {
    use super::*;
    use matches::{assert_matches, matches};

    #[test]
    fn check_rrq() {
        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii\0");

        assert_matches!(packet, Ok(Packet::Rrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Netascii,
                            opts: Opts::default()
                        }
        );

        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x01abc\0netascii\0".to_vec()
        );

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascII\0");

        assert_matches!(packet, Ok(Packet::Rrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Netascii,
                            opts: Opts::default()
                        }
        );

        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x01abc\0netascii\0".to_vec()
        );

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii\0more");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascXX\0");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(
            b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\05556\0",
        );

        assert_matches!(packet, Ok(Packet::Rrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Netascii,
                            opts: Opts {
                                block_size: Some(123),
                                timeout: Some(3),
                                transfer_size: Some(5556)
                            }
                        }
        );

        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\05556\0"
                .to_vec()
        );

        let packet = Packet::from_bytes(
            b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\0",
        );
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet =
            Packet::from_bytes(b"\x00\x01abc\0netascii\0blksizeX\0123\0");
        assert_matches!(packet, Ok(Packet::Rrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Netascii,
                            opts: Opts::default()
                        }
        );
    }

    #[test]
    fn check_wrq() {
        let packet = Packet::from_bytes(b"\x00\x02abc\0octet\0");

        assert_matches!(packet, Ok(Packet::Wrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Octet,
                            opts: Opts::default()
                        }
        );

        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x02abc\0octet\0".to_vec()
        );

        let packet = Packet::from_bytes(b"\x00\x02abc\0OCTet\0");

        assert_matches!(packet, Ok(Packet::Wrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Octet,
                            opts: Opts::default()
                        }
        );

        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x02abc\0octet\0".to_vec()
        );

        let packet = Packet::from_bytes(b"\x00\x02abc\0octet\0more");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octet");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octex\0");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(
            b"\x00\x02abc\0octet\0blksize\0123\0timeout\03\0tsize\05556\0",
        );

        assert_matches!(packet, Ok(Packet::Wrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Octet,
                            opts: Opts {
                                block_size: Some(123),
                                timeout: Some(3),
                                transfer_size: Some(5556)
                            }
                        }
        );

        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x02abc\0octet\0blksize\0123\0timeout\03\0tsize\05556\0"
                .to_vec()
        );

        let packet = Packet::from_bytes(b"\x00\x02abc\0octet\0blksizeX\0123\0");
        assert_matches!(packet, Ok(Packet::Wrq(ref req))
                        if req == &RwReq {
                            filename: "abc".to_string(),
                            mode: Mode::Octet,
                            opts: Opts::default()
                        }
        );
    }

    #[test]
    fn check_data() {
        let packet = Packet::from_bytes(b"\x00\x03\x00\x09abcde");
        assert_matches!(packet, Ok(Packet::Data(9, ref data)) if &data[..] == b"abcde");

        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x03\x00\x09abcde".to_vec()
        );

        let packet = Packet::from_bytes(b"\x00\x03\x00\x09");
        assert_matches!(packet, Ok(Packet::Data(9, ref data)) if data.is_empty());
        assert_eq!(packet.unwrap().to_bytes(), b"\x00\x03\x00\x09".to_vec());
    }

    #[test]
    fn check_ack() {
        let packet = Packet::from_bytes(b"\x00\x04\x00\x09");
        assert_matches!(packet, Ok(Packet::Ack(9)));
        assert_eq!(packet.unwrap().to_bytes(), b"\x00\x04\x00\x09".to_vec());

        let packet = Packet::from_bytes(b"\x00\x04\x00");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x04\x00\x09a");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));
    }

    #[test]
    fn check_error() {
        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg\0");
        assert_matches!(packet, Ok(Packet::Error(8, ref errmsg)) if errmsg == "msg");
        assert_eq!(
            packet.unwrap().to_bytes(),
            b"\x00\x05\x00\x08msg\0".to_vec()
        );

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg\0more");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));
    }

    #[test]
    fn check_oack() {
        let packet = Packet::from_bytes(b"\x00\x06");
        assert_matches!(packet, Ok(Packet::OAck(ref opts)) if opts == &Opts::default());

        let packet = Packet::from_bytes(b"\x00\x06blksize\0123\0");
        assert_matches!(packet, Ok(Packet::OAck(ref opts))
                        if opts == &Opts {
                            block_size: Some(123),
                            timeout: None,
                            transfer_size: None
                        }
        );

        let packet = Packet::from_bytes(b"\x00\x06timeout\03\0");
        assert_matches!(packet, Ok(Packet::OAck(ref opts))
                        if opts == &Opts {
                            block_size: None,
                            timeout: Some(3),
                            transfer_size: None
                        }
        );

        let packet = Packet::from_bytes(b"\x00\x06tsize\05556\0");
        assert_matches!(packet, Ok(Packet::OAck(ref opts))
                        if opts == &Opts {
                            block_size: None,
                            timeout: None,
                            transfer_size: Some(5556),
                        }
        );

        let packet = Packet::from_bytes(
            b"\x00\x06tsize\05556\0blksize\0123\0timeout\03\0",
        );
        assert_matches!(packet, Ok(Packet::OAck(ref opts))
                        if opts == &Opts {
                            block_size: Some(123),
                            timeout: Some(3),
                            transfer_size: Some(5556),
                        }
        );
    }

    #[test]
    fn check_packet() {
        let packet = Packet::from_bytes(b"\x00\x07");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

        let packet = Packet::from_bytes(b"\x00\x05\x00");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));
    }

    #[test]
    fn check_blksize_boundaries() {
        let (_, opts) = parse_opts(b"blksize\07\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                block_size: None,
                ..Opts::default()
            }
        );

        let (_, opts) = parse_opts(b"blksize\08\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                block_size: Some(8),
                ..Opts::default()
            }
        );

        let (_, opts) = parse_opts(b"blksize\065464\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                block_size: Some(65464),
                ..Opts::default()
            }
        );

        let (_, opts) = parse_opts(b"blksize\065465\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                block_size: None,
                ..Opts::default()
            }
        );
    }

    #[test]
    fn check_timeout_boundaries() {
        let (_, opts) = parse_opts(b"timeout\00\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                timeout: None,
                ..Opts::default()
            }
        );

        let (_, opts) = parse_opts(b"timeout\01\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                timeout: Some(1),
                ..Opts::default()
            }
        );

        let (_, opts) = parse_opts(b"timeout\0255\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                timeout: Some(255),
                ..Opts::default()
            }
        );

        let (_, opts) = parse_opts(b"timeout\0256\0").unwrap();
        assert_eq!(
            opts,
            Opts {
                timeout: None,
                ..Opts::default()
            }
        );
    }
}
