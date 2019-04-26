// clippy finds redundant closures in nom macros.
// allow this until it is fixed in nom.
#![allow(clippy::redundant_closure)]

use bytes::BufMut;
use nom::be_u16;
use std::str::{self, FromStr};

use crate::error::*;

const RRQ: u16 = 1;
const WRQ: u16 = 2;
const DATA: u16 = 3;
const ACK: u16 = 4;
const ERROR: u16 = 5;

#[derive(Debug, PartialEq)]
pub enum Packet {
    Rrq(String, Mode),
    Wrq(String, Mode),
    Data(u16, Vec<u8>),
    Ack(u16),
    Error(u16, String),
}

#[derive(Debug, PartialEq)]
pub enum Mode {
    Netascii,
    Octet,
    Mail,
}

fn take_512_max(i: &[u8]) -> nom::IResult<&[u8], &[u8]> {
    if i.len() <= 512 {
        Ok((&[], i))
    } else {
        Ok((&i[512..], &i[..512]))
    }
}

named!(nul_string<&[u8], &str>,
    do_parse!(
        s: map_res!(take_till!(|ch| ch == b'\0'), str::from_utf8) >>
        take!(1) >>

        (s)
    )
);

named!(filename_mode<&[u8], (&str, Mode)>,
     do_parse!(
        filename: nul_string >>
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
        data: take_512_max >>

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
        msg: nul_string >>

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
        let (rest, p) = packet(data).map_err(|_| Error::from(ErrorKind::InvalidPacket))?;

        if rest.is_empty() {
            Ok(p)
        } else {
            Err(ErrorKind::PacketTooLarge.into())
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
