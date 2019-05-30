#![feature(async_await)]
// false positive: https://github.com/rust-lang/rust-clippy/issues/3988
#![allow(clippy::needless_lifetimes)]

mod async_server;
mod error;
mod packet;
mod read_req;
mod utils;

pub use crate::async_server::*;
pub use crate::error::*;
