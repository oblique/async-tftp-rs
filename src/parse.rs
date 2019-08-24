use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_till};
use nom::combinator::{map, map_opt, map_res, rest};
use nom::multi::many0;
use nom::number::complete::be_u16;
use nom::sequence::tuple;
use nom::IResult;
use num_traits::FromPrimitive;
use std::str::{self, FromStr};

use crate::error::*;
use crate::packet::*;

#[derive(Debug)]
enum Opt<'a> {
    BlkSize(u16),
    Timeout(u8),
    Tsize(u64),
    Invalid(&'a str, &'a str),
}

pub(crate) fn parse_packet(input: &[u8]) -> Result<Packet> {
    let (rest, packet) = match parse_packet_type(input)? {
        (data, PacketType::Rrq) => parse_rrq(data)?,
        (data, PacketType::Wrq) => parse_wrq(data)?,
        (data, PacketType::Data) => parse_data(data)?,
        (data, PacketType::Ack) => parse_ack(data)?,
        (data, PacketType::Error) => parse_error(data)?,
        (data, PacketType::OAck) => parse_oack(data)?,
    };

    if rest.is_empty() {
        Ok(packet)
    } else {
        Err(Error::InvalidPacket)
    }
}

fn nul_str(input: &[u8]) -> IResult<&[u8], &str> {
    map_res(
        tuple((take_till(|c| c == b'\0'), tag(b"\0"))),
        |(s, _): (&[u8], _)| str::from_utf8(s),
    )(input)
}

fn parse_packet_type(input: &[u8]) -> IResult<&[u8], PacketType> {
    map_opt(be_u16, PacketType::from_u16)(input)
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
    tuple((be_u16, nul_str))(input).map(|(i, (code, msg))| {
        (i, TftpError::from_code(code, Some(msg)).into())
    })
}

fn parse_oack(input: &[u8]) -> IResult<&[u8], Packet> {
    parse_opts(input).map(|(i, opts)| (i, Packet::OAck(opts)))
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
