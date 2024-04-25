use std::convert::TryInto;
use std::str::{self, FromStr};

use crate::error::{Error, Result};
use crate::packet::{
    Error as PacketError, Mode, Opts, Packet, PacketType, RwReq,
};

pub(crate) fn parse_packet(input: &[u8]) -> Result<Packet> {
    parse_packet_type(input)
        .and_then(|(packet_type, data)| match packet_type {
            PacketType::Rrq => parse_rrq(data),
            PacketType::Wrq => parse_wrq(data),
            PacketType::Data => parse_data(data),
            PacketType::Ack => parse_ack(data),
            PacketType::Error => parse_error(data),
            PacketType::OAck => parse_oack(data),
        })
        .ok_or(Error::InvalidPacket)
}

fn parse_nul_str(input: &[u8]) -> Option<(&str, &[u8])> {
    let pos = input.iter().position(|c| *c == b'\0')?;
    let s = str::from_utf8(&input[..pos]).ok()?;
    Some((s, &input[pos + 1..]))
}

fn parse_u16_be(input: &[u8]) -> Option<(u16, &[u8])> {
    let bytes = input.get(..2)?;
    let num = u16::from_be_bytes(bytes.try_into().ok()?);
    Some((num, &input[2..]))
}

fn parse_packet_type(input: &[u8]) -> Option<(PacketType, &[u8])> {
    let (num, rest) = parse_u16_be(input)?;
    let val = PacketType::from_u16(num)?;
    Some((val, rest))
}

fn parse_mode(input: &[u8]) -> Option<(Mode, &[u8])> {
    let (s, rest) = parse_nul_str(input)?;

    let mode = if s.eq_ignore_ascii_case("netascii") {
        Mode::Netascii
    } else if s.eq_ignore_ascii_case("octet") {
        Mode::Octet
    } else if s.eq_ignore_ascii_case("mail") {
        Mode::Mail
    } else {
        return None;
    };

    Some((mode, rest))
}

pub(crate) fn parse_opts(mut input: &[u8]) -> Option<Opts> {
    let mut opts = Opts::default();

    while !input.is_empty() {
        let (name, rest) = parse_nul_str(input)?;
        let (val, rest) = parse_nul_str(rest)?;

        if name.eq_ignore_ascii_case("blksize") {
            if let Ok(val) = u16::from_str(val) {
                if val >= 8 && val <= 65464 {
                    opts.block_size = Some(val);
                }
            }
        } else if name.eq_ignore_ascii_case("timeout") {
            if let Ok(val) = u8::from_str(val) {
                if val >= 1 {
                    opts.timeout = Some(val);
                }
            }
        } else if name.eq_ignore_ascii_case("tsize") {
            if let Ok(val) = u64::from_str(val) {
                opts.transfer_size = Some(val);
            }
        }

        input = rest;
    }

    Some(opts)
}

fn parse_rrq(input: &[u8]) -> Option<Packet> {
    let (filename, rest) = parse_nul_str(input)?;
    let (mode, rest) = parse_mode(rest)?;
    let opts = parse_opts(rest)?;

    Some(Packet::Rrq(RwReq {
        filename: filename.to_owned(),
        mode,
        opts,
    }))
}

fn parse_wrq(input: &[u8]) -> Option<Packet> {
    let (filename, rest) = parse_nul_str(input)?;
    let (mode, rest) = parse_mode(rest)?;
    let opts = parse_opts(rest)?;

    Some(Packet::Wrq(RwReq {
        filename: filename.to_owned(),
        mode,
        opts,
    }))
}

fn parse_data(input: &[u8]) -> Option<Packet> {
    let (block_nr, rest) = parse_u16_be(input)?;
    Some(Packet::Data(block_nr, rest))
}

fn parse_ack(input: &[u8]) -> Option<Packet> {
    let (block_nr, rest) = parse_u16_be(input)?;

    if !rest.is_empty() {
        return None;
    }

    Some(Packet::Ack(block_nr))
}

fn parse_error(input: &[u8]) -> Option<Packet> {
    let (code, rest) = parse_u16_be(input)?;
    let (msg, rest) = parse_nul_str(rest)?;

    if !rest.is_empty() {
        return None;
    }

    Some(Packet::Error(PacketError::from_code(code, Some(msg))))
}

fn parse_oack(input: &[u8]) -> Option<Packet> {
    let opts = parse_opts(input)?;
    Some(Packet::OAck(opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nul_str() {
        let (s, rest) = parse_nul_str(b"123\0").unwrap();
        assert_eq!(s, "123");
        assert!(rest.is_empty());

        let (s, rest) = parse_nul_str(b"123\0\0").unwrap();
        assert_eq!(s, "123");
        assert_eq!(rest, b"\0");

        let (s1, rest) = parse_nul_str(b"123\0abc\0\xff\xff").unwrap();
        let (s2, rest) = parse_nul_str(rest).unwrap();
        assert_eq!(s1, "123");
        assert_eq!(s2, "abc");
        assert_eq!(rest, b"\xff\xff");

        let (s1, rest) = parse_nul_str(b"\0\0").unwrap();
        let (s2, rest) = parse_nul_str(rest).unwrap();
        assert_eq!(s1, "");
        assert_eq!(s2, "");
        assert!(rest.is_empty());

        assert!(parse_nul_str(b"").is_none());
        assert!(parse_nul_str(b"123").is_none());
        assert!(parse_nul_str(b"123\xff\xff\0").is_none());
    }

    #[test]
    fn u16_be() {
        let (n, rest) = parse_u16_be(b"\x11\x22").unwrap();
        assert_eq!(n, 0x1122);
        assert!(rest.is_empty());

        let (n, rest) = parse_u16_be(b"\x11\x22\x33").unwrap();
        assert_eq!(n, 0x1122);
        assert_eq!(rest, b"\x33");

        let (n1, rest) = parse_u16_be(b"\x11\x22\x33\x44\x55").unwrap();
        let (n2, rest) = parse_u16_be(rest).unwrap();
        assert_eq!(n1, 0x1122);
        assert_eq!(n2, 0x3344);
        assert_eq!(rest, b"\x55");

        assert!(parse_u16_be(b"").is_none());
        assert!(parse_u16_be(b"\x11").is_none());
    }
}
