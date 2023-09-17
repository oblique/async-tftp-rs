use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_till};
use nom::combinator::{map, map_opt, map_res, rest};
use nom::multi::many0;
use nom::number::complete::be_u16;
use nom::sequence::tuple;
use nom::IResult;
use std::str::{self, FromStr};

use crate::error::Result;
use crate::packet::{self, *};

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
        Err(crate::Error::InvalidPacket)
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

pub(crate) fn parse_opts(input: &[u8]) -> IResult<&[u8], Opts> {
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
        (i, packet::Error::from_code(code, Some(msg)).into())
    })
}

fn parse_oack(input: &[u8]) -> IResult<&[u8], Packet> {
    parse_opts(input).map(|(i, opts)| (i, Packet::OAck(opts)))
}
