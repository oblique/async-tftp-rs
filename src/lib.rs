//! This library provides TFTP async implementation.
//!
//! Currently it implements only server side which can serve read
//! requests, which is the most prominent scenario used.
//!
//! The following RFCs are implemented:
//!
//! * [RFC 1350] - The TFTP Protocol (Revision 2).
//! * [RFC 2347] - TFTP Option Extension.
//! * [RFC 2348] - TFTP Blocksize Option.
//! * [RFC 2349] - TFTP Timeout Interval and Transfer Size Options.
//!
//! Features:
//!
//! * Async implementation.
//! * Serve read requests.
//! * Unlimited transfer file size (block number roll-over).
//! * You can set non-standard reply [`timeout`]. This is useful for faster
//!   file transfer in unstable environments.
//! * You can set [block size limit]. This is useful if you are accessing
//!   client through a VPN.
//! * You can implement your own [`Handler`] for more advance cases than
//!   just serving a directory.
//!
//! ### Example
//!
//! ```ignore
//! use async_tftp::server::TftpServerBuilder;
//! use async_tftp::Result;
//!
//! fn main() -> Result<()> {
//!    async_std::task::block_on(async {
//!        let tftpd = TftpServerBuilder::with_dir_ro(".")?.build().await?;
//!        tftpd.serve().await?;
//!        Ok(())
//!    })
//! }
//! ```
//!
//! Add in `Cargo.toml`:
//!
//! ```toml
//! async-tftp = "0.2"
//! ```
//!
//! The above will use [async-std] by default, if you prefer [tokio] use:
//!
//! ```toml
//! async-tftp = { version = "0.2", default-features = false, features = ["use-tokio"] }
//! ```
//!
//!
//! [async-std]: https://docs.rs/async-std
//! [tokio]: https://docs.rs/tokio
//!
//! [`timeout`]: server/struct.TftpServerBuilder.html#method.timeout
//! [block size limit]: server/struct.TftpServerBuilder.html#method.block_size_limit
//! [`Handler`]: server/trait.Handler.html
//!
//! [RFC 1350]: https://tools.ietf.org/html/rfc1350
//! [RFC 2347]: https://tools.ietf.org/html/rfc2347
//! [RFC 2348]: https://tools.ietf.org/html/rfc2348
//! [RFC 2349]: https://tools.ietf.org/html/rfc2349

#[macro_use]
pub mod log;
pub mod server;

/// Packet definitions that are needed in public API.
pub mod packet;

mod error;
mod parse;
mod runtime;
mod tests;

pub use crate::error::*;

/// Re-export of `async_trait:async_trait`.
pub use async_trait::async_trait;
