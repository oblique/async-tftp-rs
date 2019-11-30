use bytes::{Bytes, BytesMut};
use matches::{assert_matches, matches};

use crate::error::Error;
use crate::packet::{self, Mode, Opts, Packet, RwReq};
use crate::parse::parse_opts;

fn packet_to_bytes(packet: &Packet) -> Bytes {
    let mut buf = BytesMut::with_capacity(0);
    packet.encode(&mut buf);
    buf.freeze()
}

#[test]
fn check_rrq() {
    let packet = Packet::decode(b"\x00\x01abc\0netascii\0");

    assert_matches!(packet, Ok(Packet::Rrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Netascii,
                        opts: Opts::default()
                    }
    );

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x01abc\0netascii\0"[..]
    );

    let packet = Packet::decode(b"\x00\x01abc\0netascII\0");

    assert_matches!(packet, Ok(Packet::Rrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Netascii,
                        opts: Opts::default()
                    }
    );

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x01abc\0netascii\0"[..]
    );

    let packet = Packet::decode(b"\x00\x01abc\0netascii\0more");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x01abc\0netascii");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x01abc\0netascXX\0");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(
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
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\05556\0"[..]
    );

    let packet = Packet::decode(
        b"\x00\x01abc\0netascii\0blksize\0123\0timeout\03\0tsize\0",
    );
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x01abc\0netascii\0blksizeX\0123\0");
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
    let packet = Packet::decode(b"\x00\x02abc\0octet\0");

    assert_matches!(packet, Ok(Packet::Wrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Octet,
                        opts: Opts::default()
                    }
    );

    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x02abc\0octet\0"[..]);

    let packet = Packet::decode(b"\x00\x02abc\0OCTet\0");

    assert_matches!(packet, Ok(Packet::Wrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Octet,
                        opts: Opts::default()
                    }
    );

    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x02abc\0octet\0"[..]);

    let packet = Packet::decode(b"\x00\x02abc\0octet\0more");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x02abc\0octet");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x02abc\0octex\0");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(
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
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x02abc\0octet\0blksize\0123\0timeout\03\0tsize\05556\0"[..]
    );

    let packet = Packet::decode(b"\x00\x02abc\0octet\0blksizeX\0123\0");
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
    let packet = Packet::decode(b"\x00\x03\x00\x09abcde");
    assert_matches!(packet, Ok(Packet::Data(9, ref data)) if &data[..] == b"abcde");

    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x03\x00\x09abcde"[..]);

    let packet = Packet::decode(b"\x00\x03\x00\x09");
    assert_matches!(packet, Ok(Packet::Data(9, ref data)) if data.is_empty());
    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x03\x00\x09"[..]);
}

#[test]
fn check_ack() {
    let packet = Packet::decode(b"\x00\x04\x00\x09");
    assert_matches!(packet, Ok(Packet::Ack(9)));
    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x04\x00\x09"[..]);

    let packet = Packet::decode(b"\x00\x04\x00");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x04\x00\x09a");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));
}

#[test]
fn check_error() {
    let packet = Packet::decode(b"\x00\x05\x00\x01msg\0");
    assert_matches!(packet, Ok(Packet::Error(packet::Error::FileNotFound)));
    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x05\x00\x01File not found\0"[..]
    );

    // 0x10 is unknown error code an will be treated as 0
    let packet = Packet::decode(b"\x00\x05\x00\x10msg\0");
    assert_matches!(packet, Ok(Packet::Error(packet::Error::Msg(ref errmsg)))
                        if errmsg == "msg");
    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x05\x00\x00msg\0"[..]);

    let packet = Packet::decode(b"\x00\x05\x00\x00msg\0");
    assert_matches!(packet, Ok(Packet::Error(packet::Error::Msg(ref errmsg)))
                        if errmsg == "msg");
    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x05\x00\x00msg\0"[..]);

    let packet = Packet::decode(b"\x00\x05\x00\x00msg\0more");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x05\x00\x00msg");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x05\x00\x00");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));
}

#[test]
fn check_oack() {
    let packet = Packet::decode(b"\x00\x06");
    assert_matches!(packet, Ok(Packet::OAck(ref opts)) if opts == &Opts::default());

    let packet = Packet::decode(b"\x00\x06blksize\0123\0");
    assert_matches!(packet, Ok(Packet::OAck(ref opts))
                    if opts == &Opts {
                        block_size: Some(123),
                        timeout: None,
                        transfer_size: None
                    }
    );

    let packet = Packet::decode(b"\x00\x06timeout\03\0");
    assert_matches!(packet, Ok(Packet::OAck(ref opts))
                    if opts == &Opts {
                        block_size: None,
                        timeout: Some(3),
                        transfer_size: None
                    }
    );

    let packet = Packet::decode(b"\x00\x06tsize\05556\0");
    assert_matches!(packet, Ok(Packet::OAck(ref opts))
                    if opts == &Opts {
                        block_size: None,
                        timeout: None,
                        transfer_size: Some(5556),
                    }
    );

    let packet =
        Packet::decode(b"\x00\x06tsize\05556\0blksize\0123\0timeout\03\0");
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
    let packet = Packet::decode(b"\x00\x07");
    assert_matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket));

    let packet = Packet::decode(b"\x00\x05\x00");
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
