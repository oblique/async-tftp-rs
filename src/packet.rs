use bytes::BufMut;
use std::io;
use std::result::Result as StdResult;
use std::str::{self, FromStr};

use nom::{
    alt_complete, be_u16, call, do_parse, error_position, many0_count, map_res,
    named, named_args, rest, switch, tag, tag_no_case, take_till,
};

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
const ERR_INVALID_TFTP: u16 = 4;
const ERR_ALREADY_EXISTS: u16 = 6;

#[derive(Debug, PartialEq)]
pub enum Packet {
    Rrq(RwReq),
    Wrq(RwReq),
    Data(u16, Vec<u8>),
    Ack(u16),
    Error(u16, String),
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

named!(nul_str<&[u8], &str>,
    do_parse!(
        s: map_res!(take_till!(|ch| ch == b'\0'), str::from_utf8) >>
        tag!("\0") >>

        (s)
    )
);

named!(mode<&[u8], Mode>,
     do_parse!(
        mode: map_res!(map_res!(alt_complete!(
            tag_no_case!("netascii") |
            tag_no_case!("octet") |
            tag_no_case!("mail")
        ), str::from_utf8), Mode::from_str) >>
        tag!("\0") >>

        (mode)
    )
);

named_args!(parse_opts<'a>(opts: &mut Opts)<&'a [u8], usize>,
    many0_count!(alt_complete!(
        do_parse!(
            tag_no_case!("blksize\0") >>
            n: map_res!(nul_str, u16::from_str) >>

            (opts.block_size = Some(n).filter(|x| *x >= 8 && *x <= 65464))
        ) |
        do_parse!(
            tag_no_case!("timeout\0") >>
            n: map_res!(nul_str, u8::from_str) >>

            (opts.timeout = Some(n).filter(|x| *x >= 1))
        ) |
        do_parse!(
            tag_no_case!("tsize\0") >>
            n: map_res!(nul_str, u64::from_str) >>

            (opts.transfer_size = Some(n))
        )
    ))
);

fn opts(i: &[u8]) -> nom::IResult<&[u8], Opts> {
    let mut opts = Opts::default();
    let (i, _) = parse_opts(i, &mut opts)?;
    Ok((i, opts))
}

named!(rrq<&[u8], Packet>,
    do_parse!(
        filename: nul_str >>
        mode: mode >>
        opts: opts >>

        (Packet::Rrq(RwReq { filename: filename.to_owned(), mode, opts }))
    )
);

named!(wrq<&[u8], Packet>,
    do_parse!(
        filename: nul_str >>
        mode: mode >>
        opts: opts >>

        (Packet::Wrq(RwReq { filename: filename.to_owned(), mode, opts }))
    )
);

named!(data<&[u8], Packet>,
    do_parse!(
        block: be_u16 >>
        data: rest >>

        (Packet::Data(block, data.to_vec()))
    )
);

named!(ack<&[u8], Packet>,
    do_parse!(
        block: be_u16 >>

        (Packet::Ack(block))
    )
);

named!(error<&[u8], Packet>,
    do_parse!(
        code: be_u16 >>
        msg: nul_str >>

        (Packet::Error(code, msg.to_owned()))
    )
);

named!(oack<&[u8], Packet>,
    do_parse!(
        opts: opts >>

        (Packet::OAck(opts))
    )
);

named!(packet<&[u8], Packet>,
    do_parse!(
        packet: switch!(be_u16,
            RRQ => call!(rrq) |
            WRQ => call!(wrq) |
            DATA => call!(data) |
            ACK => call!(ack) |
            ERROR => call!(error) |
            OACK => call!(oack)
        ) >>

        (packet)
    )
);

impl Packet {
    pub fn from_bytes(data: &[u8]) -> Result<Packet> {
        let (rest, p) = packet(data).map_err(|_| Error::InvalidPacket)?;

        // ensure that whole packet was consumed
        if rest.is_empty() {
            Ok(p)
        } else {
            Err(Error::InvalidPacket)
        }
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
                buf.put(data);
            }
            Packet::Ack(block) => {
                buf.put_u16_be(ACK);
                buf.put_u16_be(*block);
            }
            Packet::Error(code, msg) => {
                buf.put_u16_be(ERROR);
                buf.put_u16_be(*code);
                buf.put(msg);
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

impl From<Error> for Packet {
    fn from(err: Error) -> Self {
        let (err_id, err_msg) = match err {
            Error::Io(err) => match err.kind() {
                io::ErrorKind::NotFound => {
                    (ERR_NOT_FOUNT, "File not found".to_string())
                }
                io::ErrorKind::PermissionDenied => {
                    (ERR_PERM_DENIED, "Access violation".to_string())
                }
                io::ErrorKind::WriteZero => (
                    ERR_FULL_DISK,
                    "Disk full or allocation exceeded".to_string(),
                ),
                io::ErrorKind::AlreadyExists => {
                    (ERR_ALREADY_EXISTS, "File already exists".to_string())
                }
                _ => match err.raw_os_error() {
                    Some(rc) => (ERR_NOT_DEFINED, format!("IO error: {}", rc)),
                    None => (ERR_NOT_DEFINED, "Unknown IO error".to_string()),
                },
            },
            Error::InvalidMode | Error::InvalidPacket => {
                (ERR_INVALID_TFTP, "Illegal TFTP operation".to_string())
            }
            Error::Bind(_) => unreachable!(),
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

impl FromStr for Mode {
    type Err = Error;

    fn from_str(s: &str) -> StdResult<Self, Self::Err> {
        match s.to_owned().to_lowercase().as_str() {
            "netascii" => Ok(Mode::Netascii),
            "octet" => Ok(Mode::Octet),
            "mail" => Ok(Mode::Mail),
            _ => Err(Error::InvalidMode),
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
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));
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

        let packet =
            Packet::from_bytes(b"\x00\x02abc\0netascii\0blksizeX\0123\0");
        assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));
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
        let (_, opt) = opts(b"blksize\07\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                block_size: None,
                ..Opts::default()
            }
        );

        let (_, opt) = opts(b"blksize\08\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                block_size: Some(8),
                ..Opts::default()
            }
        );

        let (_, opt) = opts(b"blksize\065464\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                block_size: Some(65464),
                ..Opts::default()
            }
        );

        let (_, opt) = opts(b"blksize\065465\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                block_size: None,
                ..Opts::default()
            }
        );
    }

    #[test]
    fn check_timeout_boundaries() {
        let (_, opt) = opts(b"timeout\00\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                timeout: None,
                ..Opts::default()
            }
        );

        let (_, opt) = opts(b"timeout\01\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                timeout: Some(1),
                ..Opts::default()
            }
        );

        let (_, opt) = opts(b"timeout\0255\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                timeout: Some(255),
                ..Opts::default()
            }
        );

        let (_, opt) = opts(b"timeout\0256\0").unwrap();
        assert_eq!(
            opt,
            Opts {
                timeout: None,
                ..Opts::default()
            }
        );
    }
}
