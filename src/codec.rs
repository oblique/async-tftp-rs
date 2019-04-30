use bytes::BytesMut;
use tokio::codec::{Decoder, Encoder};

use crate::error::Error;
use crate::packet::Packet;

pub struct Codec;

impl Codec {
    pub fn new() -> Self {
        Codec
    }
}

impl Decoder for Codec {
    type Item = Packet;
    type Error = Error;

    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        let packet = Packet::from_bytes(&src)?;
        src.clear();
        Ok(Some(packet))
    }
}

impl Encoder for Codec {
    type Item = Packet;
    type Error = Error;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        *dst = item.to_bytes()?.into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::*;

    #[test]
    fn decode() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&b"\x00\x01abc\0netascii\0"[..]);

        let mut codec = Codec::new();
        let packet = codec.decode(&mut buf);

        assert_eq!(
            packet,
            Ok(Some(Packet::Rrq(
                "abc".to_string(),
                Mode::Netascii,
                Opts::default()
            )))
        );
    }

    #[test]
    fn encode() {
        let mut buf = BytesMut::new();
        let packet =
            Packet::Rrq("abc".to_string(), Mode::Netascii, Opts::default());

        let mut codec = Codec::new();
        codec.encode(packet, &mut buf).expect("encode failed");

        assert_eq!(&buf[..], &b"\x00\x01abc\0netascii\0"[..]);
    }
}
