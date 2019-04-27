// clippy finds redundant closures in nom macros.
// allow this until it is fixed in nom.
#![allow(clippy::redundant_closure)]

use bytes::BufMut;
use nom::{be_u16, rest};
use std::str::{self, FromStr};

use crate::error::*;

/// Packet types
const RRQ: u16 = 1;
const WRQ: u16 = 2;
const DATA: u16 = 3;
const ACK: u16 = 4;
const ERROR: u16 = 5;

/// A struct which enforces our constraints of a max 512 byte size so
/// that we don't have to worry about validation after this gets created.
#[derive(PartialEq, Debug)]
pub struct DataBlock {
    content: Vec<u8>,
}

impl DataBlock {
    fn new(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 512 {
            Err(ErrorKind::PacketTooLarge.into())
        } else {
            Ok(Self {
                content: Vec::from(bytes),
            })
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Packet {
    Rrq(String, Mode),
    Wrq(String, Mode),
    Data(u16, DataBlock),
    Ack(u16),
    Error(u16, String),
}

#[derive(Debug, PartialEq)]
pub enum Mode {
    Netascii,
    Octet,
    Mail,
}


named!(data_block<DataBlock>,
       do_parse!(
           data: alt_complete!(
               take!(512) |
               call!(rest)
           ) >>
               ({ DataBlock::new(data).unwrap() })
       )
);

named!(nul_terminated_string<&[u8], &str>,
    do_parse!(
        s: map_res!(take_till!(|ch| ch == b'\0'), str::from_utf8) >>
            take!(1) >>

        (s)
    )
);

named_args!(
    mode<'a>(name: &'a str)<Mode>,
    map_res!(map_res!(
        tag_no_case!(name),
        str::from_utf8), Mode::from_str)
);

named!(filename_mode<&[u8], (&str, Mode)>,
     do_parse!(
        filename: nul_terminated_string >>
        mode: alt!(
            call!(mode, "netascii") |
            call!(mode, "octet") |
            call!(mode, "mail")
        ) >>
             tag!("\0") >>

        ({
            (filename, mode)
        })
    )
);

named!(rrq<&[u8], Packet>,
    do_parse!(
        fm: filename_mode >>

        ({
            let (filename, mode) = fm;
            Packet::Rrq(filename.to_owned(), mode)
        })
    )
);

named!(wrq<&[u8], Packet>,
    do_parse!(
        fm: filename_mode >>

        ({
            let (filename, mode) = fm;
            Packet::Wrq(filename.to_owned(), mode)
        })
    )
);

named!(data<&[u8], Packet>,
    do_parse!(
        block: be_u16 >>
        data: data_block >>

        (Packet::Data(block, data))
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
        msg: nul_terminated_string >>

        (Packet::Error(code, msg.to_owned()))
    )
);

named!(packet<&[u8], Packet>,
    do_parse!(
        packet: switch!(be_u16,
            RRQ => call!(rrq) |
            WRQ => call!(wrq) |
            DATA => call!(data) |
            ACK => call!(ack) |
            ERROR => call!(error)
        ) >>

        (packet)
    )
);

impl Packet {
    pub fn from_bytes(data: &[u8]) -> Result<Packet> {
        let (rest, p) = packet(data)
            .map_err(|_| Error::from(ErrorKind::InvalidPacket))?;

        if rest.is_empty() {
            Ok(p)
        } else {
            Err(ErrorKind::PacketTooLarge.into())
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        match self {
            Packet::Rrq(filename, mode) => {
                buf.put_u16_be(RRQ);
                buf.put(filename);
                buf.put_u8(0);
                buf.put(mode.to_str());
                buf.put_u8(0);
            }
            Packet::Wrq(filename, mode) => {
                buf.put_u16_be(WRQ);
                buf.put(filename);
                buf.put_u8(0);
                buf.put(mode.to_str());
                buf.put_u8(0);
            }
            Packet::Data(block, data) => {
                buf.put_u16_be(DATA);
                buf.put_u16_be(*block);
                buf.put(&data.content);
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
        }

        Ok(buf)
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
    use std::iter::repeat;

    #[test]
    fn check_rrq() {
        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii\0");
        assert_eq!(packet, Ok(Packet::Rrq("abc".to_string(), Mode::Netascii)));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x01abc\0netascii\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascII\0");
        assert_eq!(packet, Ok(Packet::Rrq("abc".to_string(), Mode::Netascii)));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x01abc\0netascii\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii\0more");
        assert_eq!(packet, Err(ErrorKind::PacketTooLarge.into()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascii");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x01abc\0netascXX\0");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

    #[test]
    fn check_wrq() {
        let packet = Packet::from_bytes(b"\x00\x02abc\0octet\0");
        assert_eq!(packet, Ok(Packet::Wrq("abc".to_string(), Mode::Octet)));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x02abc\0octet\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0OCTet\0");
        assert_eq!(packet, Ok(Packet::Wrq("abc".to_string(), Mode::Octet)));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x02abc\0octet\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octet\0more");
        assert_eq!(packet, Err(ErrorKind::PacketTooLarge.into()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octet");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x02abc\0octex\0");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

    #[test]
    fn check_data() {
        let packet = Packet::from_bytes(b"\x00\x03\x00\x09abcde");
        assert_eq!(
            packet, Ok(Packet::Data(9, DataBlock::new(b"abcde").unwrap())));
        assert_eq!(
            packet.unwrap().to_bytes(), Ok(b"\x00\x03\x00\x09abcde".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x03\x00\x09");
        assert_eq!(packet, Ok(Packet::Data(9, DataBlock::new(b"").unwrap())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x03\x00\x09".to_vec()));

        let data: Vec<_> = repeat(b'a').take(512).collect();
        let mut packet_vec = b"\x00\x03\x00\x09".to_vec();
        packet_vec.extend(data.iter());

        let packet = Packet::from_bytes(&packet_vec);
        assert_eq!(packet, Ok(Packet::Data(9, DataBlock::new(&data).unwrap())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(packet_vec));

        let data: Vec<_> = repeat(b'a').take(513).collect();
        let mut packet_vec = b"\x00\x03\x00\x09".to_vec();
        packet_vec.extend(data.iter());

        let packet = Packet::from_bytes(&packet_vec);
        assert_eq!(packet, Err(ErrorKind::PacketTooLarge.into()));
    }

    #[test]
    fn check_ack() {
        let packet = Packet::from_bytes(b"\x00\x04\x00\x09");
        assert_eq!(packet, Ok(Packet::Ack(9)));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x04\x00\x09".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x04\x00");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x04\x00\x09a");
        assert_eq!(packet, Err(ErrorKind::PacketTooLarge.into()));
    }

    #[test]
    fn check_error() {
        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg\0");
        assert_eq!(packet, Ok(Packet::Error(8, "msg".to_string())));
        assert_eq!(packet.unwrap().to_bytes(), Ok(b"\x00\x05\x00\x08msg\0".to_vec()));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg\0more");
        assert_eq!(packet, Err(ErrorKind::PacketTooLarge.into()));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08msg");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x05\x00\x08");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }

    #[test]
    fn check_packet() {
        let packet = Packet::from_bytes(b"\x00\x06");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));

        let packet = Packet::from_bytes(b"\x00\x05\x00");
        assert_eq!(packet, Err(ErrorKind::InvalidPacket.into()));
    }
}
