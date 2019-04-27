// clippy finds redundant closures in nom macros.
// allow this until it is fixed in nom.
#![allow(clippy::redundant_closure)]

use bytes::BufMut;
use nom::{be_u16, rest};
use std::str::{self, FromStr};

use crate::error::*;

const RRQ: u16 = 1;
const WRQ: u16 = 2;
const DATA: u16 = 3;
const ACK: u16 = 4;
const ERROR: u16 = 5;
const OACK: u16 = 6;

#[derive(Debug, PartialEq)]
pub enum Packet {
    Rrq(String, Mode, Opts),
    Wrq(String, Mode, Opts),
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

enum Opt {
    BlockSize(u16),
    Timeout(u8),
    TransferSize(u64)
}

#[derive(Debug, PartialEq)]
pub struct Opts {
    block_size: Option<u16>,
    timeout: Option<u8>,
    transfer_size: Option<u64>,
}

named!(nul_str<&[u8], &str>,
    do_parse!(
        s: map_res!(take_till!(|ch| ch == b'\0'), str::from_utf8) >>
        tag!("\0") >>

        (s)
    )
);

named!(filename_mode<&[u8], (&str, Mode)>,
     do_parse!(
        filename: nul_str >>
        mode: alt!(
            tag_no_case!("netascii") |
            tag_no_case!("octet") |
            tag_no_case!("mail")
        ) >>
        tag!("\0") >>

        ({
            let mode = str::from_utf8(mode).unwrap();
            let mode = Mode::from_str(mode).unwrap();
            (filename, mode)
        })
    )
);

named!(
    blksize<Opt>,
    map!(map_res!(tuple!(
        tag_no_case!("blksize\0"),
        nul_str
    ), |(_, value)| u16::from_str(value)), Opt::BlockSize)
);

named!(
    timeout<Opt>,
    map!(map_res!(tuple!(
        tag_no_case!("timeout\0"),
        nul_str
    ), |(_, value)| u8::from_str(value)), Opt::Timeout)
);

named!(
    tsize<Opt>,
    map!(map_res!(tuple!(
        tag_no_case!("tsize\0"),
        nul_str
    ), |(_, value)| u64::from_str(value)), Opt::TransferSize)
);

named!(
    opts<Opts>,
    fold_many0!(
        alt_complete!(
            blksize |
            timeout |
            tsize
        ),
        Opts::new(),
        |mut cur_opts: Opts, opt| {
            match opt {
                Opt::BlockSize(b) => {
                    cur_opts.block_size = Some(b)
                }
                Opt::Timeout(t) => {
                    cur_opts.timeout = Some(t)
                },
                Opt::TransferSize(t) => {
                    cur_opts.transfer_size = Some(t)
                }
            };
            cur_opts
        }
    )
);

named!(rrq<&[u8], Packet>,
    do_parse!(
        fm: filename_mode >>
        opts: opts >>

        ({
            let (filename, mode) = fm;
            Packet::Rrq(filename.to_owned(), mode, opts)
        })
    )
);

named!(wrq<&[u8], Packet>,
    do_parse!(
        fm: filename_mode >>
        opts: opts >>

        ({
            let (filename, mode) = fm;
            Packet::Wrq(filename.to_owned(), mode, opts)
        })
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
        let (rest, p) = packet(data).map_err(|_| Error::from(ErrorKind::InvalidPacket))?;

        // ensure that whole packet was consumed
        if rest.is_empty() {
            Ok(p)
        } else {
            Err(ErrorKind::InvalidPacket.into())
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        match self {
            Packet::Rrq(filename, mode, opts) => {
                buf.put_u16_be(RRQ);
                buf.put(filename);
                buf.put_u8(0);
                buf.put(mode.to_str());
                buf.put_u8(0);
                opts.encode(&mut buf);
            }
            Packet::Wrq(filename, mode, opts) => {
                buf.put_u16_be(WRQ);
                buf.put(filename);
                buf.put_u8(0);
                buf.put(mode.to_str());
                buf.put_u8(0);
                opts.encode(&mut buf);
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

        Ok(buf)
    }
}

impl Opts {
    fn new() -> Self {
        Opts {
            block_size: None,
            timeout: None,
            transfer_size: None,
        }
    }

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
            _ => Err(ErrorKind::InvalidMode.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_rrq() {
        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii\0");
        assert_eq!(packet, Ok(Packet::Rrq("abc".to_string(), Mode::Netascii, Opts::new())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x01abc\0netascii\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascII\0");
        assert_eq!(packet, Ok(Packet::Rrq("abc".to_string(), Mode::Netascii, Opts::new())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x01abc\0netascii\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii\0more");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascXX\0");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet =
            Packet::from_bytes(b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\05556\0");
        assert_eq!(
            packet,
            Ok(Packet::Rrq(
                "abc".to_string(),
                Mode::Netascii,
                Opts {
                    block_size: Some(123),
                    timeout: Some(3),
                    transfer_size: Some(5556)
                }
            ))
        );
        assert_eq!(
            packet.unwrap().to_bytes(),
            Ok(b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\05556\0".to_vec())
        );

        let packet =
            Packet::from_bytes(b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\0");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii\0blksizeX\0123\0");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

    #[test]
    fn check_wrq() {
        let packet = Packet::from_bytes(b"\x00\x02abc\0octet\0");
        assert_eq!(packet, Ok(Packet::Wrq("abc".to_string(), Mode::Octet, Opts::new())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x02abc\0octet\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0OCTet\0");
        assert_eq!(packet, Ok(Packet::Wrq("abc".to_string(), Mode::Octet, Opts::new())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x02abc\0octet\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octet\0more");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octet");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octex\0");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet =
            Packet::from_bytes(b"\x00\x02abc\0octet\0blksize\0123\0timeout\03\0tsize\05556\0");
        assert_eq!(
            packet,
            Ok(Packet::Wrq(
                "abc".to_string(),
                Mode::Octet,
                Opts {
                    block_size: Some(123),
                    timeout: Some(3),
                    transfer_size: Some(5556)
                }
            ))
        );
        assert_eq!(
            packet.unwrap().to_bytes(),
            Ok(b"\x00\x02abc\0octet\0blksize\0123\0timeout\03\0tsize\05556\0".to_vec())
        );

        let packet = Packet::from_bytes(b"\x00\x02abc\0netascii\0blksizeX\0123\0");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

    #[test]
    fn check_data() {
        let packet = Packet::from_bytes(b"\x00\x03\x00\x09abcde");
        assert_eq!(packet, Ok(Packet::Data(9, b"abcde".to_vec())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x03\x00\x09abcde".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x03\x00\x09");
        assert_eq!(packet, Ok(Packet::Data(9, b"".to_vec())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x03\x00\x09".to_vec()));
    }

    #[test]
    fn check_ack() {
        let packet = Packet::from_bytes(b"\x00\x04\x00\x09");
        assert_eq!(packet, Ok(Packet::Ack(9)));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x04\x00\x09".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x04\x00");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x04\x00\x09a");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

    #[test]
    fn check_error() {
        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg\0");
        assert_eq!(packet, Ok(Packet::Error(8, "msg".to_string())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x05\x00\x08msg\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg\0more");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

    #[test]
    fn check_oack() {
        let packet = Packet::from_bytes(b"\x00\x06");
        assert_eq!(
            packet,
            Ok(Packet::OAck(Opts {
                block_size: None,
                timeout: None,
                transfer_size: None
            }))
        );

        let packet = Packet::from_bytes(b"\x00\x06blksize\0123\0");
        assert_eq!(
            packet,
            Ok(Packet::OAck(Opts {
                block_size: Some(123),
                timeout: None,
                transfer_size: None
            }))
        );

        let packet = Packet::from_bytes(b"\x00\x06timeout\03\0");
        assert_eq!(
            packet,
            Ok(Packet::OAck(Opts {
                block_size: None,
                timeout: Some(3),
                transfer_size: None
            }))
        );

        let packet = Packet::from_bytes(b"\x00\x06tsize\05556\0");
        assert_eq!(
            packet,
            Ok(Packet::OAck(Opts {
                block_size: None,
                timeout: None,
                transfer_size: Some(5556),
            }))
        );

        let packet = Packet::from_bytes(b"\x00\x06tsize\05556\0blksize\0123\0timeout\03\0");
        assert_eq!(
            packet,
            Ok(Packet::OAck(Opts {
                block_size: Some(123),
                timeout: Some(3),
                transfer_size: Some(5556),
            }))
        );
    }

    #[test]
    fn check_packet() {
        let packet = Packet::from_bytes(b"\x00\x07");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x05\x00");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

}
