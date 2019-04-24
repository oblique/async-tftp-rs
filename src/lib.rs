mod error;
mod packet;

pub use crate::error::*;
pub use crate::packet::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
