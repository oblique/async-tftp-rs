use bytes::{Buf, BufMut, BytesMut, IntoBuf};

pub trait BytesMutExt {
    fn extend_u8(&mut self, n: u8);
    fn extend_u16_be(&mut self, n: u16);
    fn extend_buf<T: IntoBuf>(&mut self, src: T)
    where
        Self: Sized;
}

impl BytesMutExt for BytesMut {
    fn extend_u8(&mut self, n: u8) {
        self.reserve(1);
        self.put_u8(n);
    }

    fn extend_u16_be(&mut self, n: u16) {
        self.reserve(3);
        self.put_u16_be(n);
    }

    fn extend_buf<T: IntoBuf>(&mut self, src: T)
    where
        Self: Sized,
    {
        let buf = src.into_buf();
        self.reserve(buf.bytes().len());
        self.put(buf);
    }
}
