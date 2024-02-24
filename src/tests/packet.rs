use bytes::{Bytes, BytesMut};

use crate::error::Error;
use crate::packet::{self, Mode, Opts, Packet, RwReq};
use crate::parse::parse_opts;

pub(crate) fn packet_to_bytes(packet: &Packet) -> Bytes {
    let mut buf = BytesMut::with_capacity(0);
    packet.encode(&mut buf);
    buf.freeze()
}

#[test]
fn check_rrq() {
    let packet = Packet::decode(b"\x00\x01abc\x00netascii\x00");

    assert!(matches!(packet, Ok(Packet::Rrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Netascii,
                        opts: Opts::default()
                    }
    ));

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x01abc\x00netascii\x00"[..]
    );

    let packet = Packet::decode(b"\x00\x01abc\x00netascII\x00");

    assert!(matches!(packet, Ok(Packet::Rrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Netascii,
                        opts: Opts::default()
                    }
    ));

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x01abc\x00netascii\x00"[..]
    );

    let packet = Packet::decode(b"\x00\x01abc\x00netascii\x00more");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x01abc\x00netascii");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x01abc\x00netascXX\x00");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(
        b"\x00\x01abc\x00netascii\x00blksize\x00123\x00timeout\x003\x00tsize\x005556\x00",
    );

    assert!(matches!(packet, Ok(Packet::Rrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Netascii,
                        opts: Opts {
                            block_size: Some(123),
                            timeout: Some(3),
                            transfer_size: Some(5556),
                            window_size: None,
                        }
                    }
    ));

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x01abc\x00netascii\x00blksize\x00123\x00timeout\x003\x00tsize\x005556\x00"[..]
    );

    let packet = Packet::decode(
        b"\x00\x01abc\x00netascii\x00blksize\x00123\x00timeout\x003\x00tsize\x00",
    );
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet =
        Packet::decode(b"\x00\x01abc\x00netascii\x00blksizeX\x00123\x00");
    assert!(matches!(packet, Ok(Packet::Rrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Netascii,
                        opts: Opts::default()
                    }
    ));
}

#[test]
fn check_wrq() {
    let packet = Packet::decode(b"\x00\x02abc\x00octet\x00");

    assert!(matches!(packet, Ok(Packet::Wrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Octet,
                        opts: Opts::default()
                    }
    ));

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x02abc\x00octet\x00"[..]
    );

    let packet = Packet::decode(b"\x00\x02abc\x00OCTet\x00");

    assert!(matches!(packet, Ok(Packet::Wrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Octet,
                        opts: Opts::default()
                    }
    ));

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x02abc\x00octet\x00"[..]
    );

    let packet = Packet::decode(b"\x00\x02abc\x00octet\x00more");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x02abc\x00octet");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x02abc\x00octex\x00");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(
        b"\x00\x02abc\x00octet\x00blksize\x00123\x00timeout\x003\x00tsize\x005556\x00windowsize\x004\x00",
    );

    assert!(matches!(packet, Ok(Packet::Wrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Octet,
                        opts: Opts {
                            block_size: Some(123),
                            timeout: Some(3),
                            transfer_size: Some(5556),
                            window_size: Some(4)
                        }
                    }
    ));

    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x02abc\x00octet\x00blksize\x00123\x00timeout\x003\x00tsize\x005556\x00windowsize\x004\x00"[..]
    );

    let packet = Packet::decode(b"\x00\x02abc\x00octet\x00blksizeX\x00123\x00");
    assert!(matches!(packet, Ok(Packet::Wrq(ref req))
                    if req == &RwReq {
                        filename: "abc".to_string(),
                        mode: Mode::Octet,
                        opts: Opts::default()
                    }
    ));
}

#[test]
fn check_data() {
    let packet = Packet::decode(b"\x00\x03\x00\x09abcde");
    assert!(
        matches!(packet, Ok(Packet::Data(9, ref data)) if &data[..] == b"abcde")
    );

    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x03\x00\x09abcde"[..]);

    let packet = Packet::decode(b"\x00\x03\x00\x09");
    assert!(matches!(packet, Ok(Packet::Data(9, ref data)) if data.is_empty()));
    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x03\x00\x09"[..]);
}

#[test]
fn check_ack() {
    let packet = Packet::decode(b"\x00\x04\x00\x09");
    assert!(matches!(packet, Ok(Packet::Ack(9))));
    assert_eq!(packet_to_bytes(&packet.unwrap()), b"\x00\x04\x00\x09"[..]);

    let packet = Packet::decode(b"\x00\x04\x00");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x04\x00\x09a");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));
}

#[test]
fn check_error() {
    let packet = Packet::decode(b"\x00\x05\x00\x01msg\x00");
    assert!(matches!(packet, Ok(Packet::Error(packet::Error::FileNotFound))));
    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x05\x00\x01File not found\x00"[..]
    );

    // 0x10 is unknown error code an will be treated as 0
    let packet = Packet::decode(b"\x00\x05\x00\x10msg\x00");
    assert!(matches!(packet, Ok(Packet::Error(packet::Error::Msg(ref errmsg)))
                        if errmsg == "msg"));
    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x05\x00\x00msg\x00"[..]
    );

    let packet = Packet::decode(b"\x00\x05\x00\x00msg\x00");
    assert!(matches!(packet, Ok(Packet::Error(packet::Error::Msg(ref errmsg)))
                        if errmsg == "msg"));
    assert_eq!(
        packet_to_bytes(&packet.unwrap()),
        b"\x00\x05\x00\x00msg\x00"[..]
    );

    let packet = Packet::decode(b"\x00\x05\x00\x00msg\x00more");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x05\x00\x00msg");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x05\x00\x00");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));
}

#[test]
fn check_oack() {
    let packet = Packet::decode(b"\x00\x06");
    assert!(
        matches!(packet, Ok(Packet::OAck(ref opts)) if opts == &Opts::default())
    );

    let packet = Packet::decode(b"\x00\x06blksize\x00123\x00");
    assert!(matches!(packet, Ok(Packet::OAck(ref opts))
                    if opts == &Opts {
                        block_size: Some(123),
                        ..Default::default()
                    }
    ));

    let packet = Packet::decode(b"\x00\x06timeout\x003\x00");
    assert!(matches!(packet, Ok(Packet::OAck(ref opts))
                    if opts == &Opts {
                        timeout: Some(3),
                        ..Default::default()
                    }
    ));

    let packet = Packet::decode(b"\x00\x06tsize\x005556\x00");
    assert!(matches!(packet, Ok(Packet::OAck(ref opts))
                    if opts == &Opts {
                        transfer_size: Some(5556),
                        ..Default::default()
                    }
    ));

    let packet = Packet::decode(
        b"\x00\x06tsize\x005556\x00blksize\x00123\x00timeout\x003\x00",
    );
    assert!(matches!(packet, Ok(Packet::OAck(ref opts))
                    if opts == &Opts {
                        block_size: Some(123),
                        timeout: Some(3),
                        transfer_size: Some(5556),
            ..Default::default()
                    }
    ));
}

#[test]
fn check_packet() {
    let packet = Packet::decode(b"\x00\x07");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));

    let packet = Packet::decode(b"\x00\x05\x00");
    assert!(matches!(packet, Err(ref e) if matches!(e, Error::InvalidPacket)));
}

#[test]
fn check_blksize_boundaries() {
    let opts = parse_opts(b"blksize\x007\x00").unwrap();
    assert_eq!(
        opts,
        Opts {
            block_size: None,
            ..Opts::default()
        }
    );

    let opts = parse_opts(b"blksize\x008\x00").unwrap();
    assert_eq!(
        opts,
        Opts {
            block_size: Some(8),
            ..Opts::default()
        }
    );

    let opts = parse_opts(b"blksize\x0065464\x00").unwrap();
    assert_eq!(
        opts,
        Opts {
            block_size: Some(65464),
            ..Opts::default()
        }
    );

    let opts = parse_opts(b"blksize\x0065465\x00").unwrap();
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
    let opts = parse_opts(b"timeout\x000\x00").unwrap();
    assert_eq!(
        opts,
        Opts {
            timeout: None,
            ..Opts::default()
        }
    );

    let opts = parse_opts(b"timeout\x001\x00").unwrap();
    assert_eq!(
        opts,
        Opts {
            timeout: Some(1),
            ..Opts::default()
        }
    );

    let opts = parse_opts(b"timeout\x00255\x00").unwrap();
    assert_eq!(
        opts,
        Opts {
            timeout: Some(255),
            ..Opts::default()
        }
    );

    let opts = parse_opts(b"timeout\x00256\x00").unwrap();
    assert_eq!(
        opts,
        Opts {
            timeout: None,
            ..Opts::default()
        }
    );
}
