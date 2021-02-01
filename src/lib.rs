//! Executor agnostic async TFTP implementation, written with [smol]
//! building blocks. Currently it implements only server side.
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
//! * Works with any runtime/executor.
//! * Serve read (RRQ) and write (WRQ) requests.
//! * Unlimited transfer file size (block number roll-over).
//! * You can set non-standard reply [`timeout`]. This is useful for faster
//!   file transfer in unstable environments.
//! * You can set [block size limit]. This is useful if you are accessing
//!   client through a VPN.
//! * You can implement your own [`Handler`] for more advance cases than
//!   just serving a directory. Check [`tftpd-targz.rs`] for an example.
//!
//! # Example
//!
//! ```ignore
//! use async_tftp::server::TftpServerBuilder;
//! use async_tftp::Result;
//!
//! fn main() -> Result<()> {
//!     smol::block_on(async { // or any other runtime/executor
//!         let tftpd = TftpServerBuilder::with_dir_ro(".")?.build().await?;
//!         tftpd.serve().await?;
//!         Ok(())
//!     })
//! }
//! ```
//!
//! Add in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! smol = "1" # or any other runtime/executor
//! async-tftp = "0.3"
//! ```
//!
//! [smol]: https://docs.rs/smol
//!
//! [`timeout`]: server::TftpServerBuilder::timeout
//! [block size limit]: server::TftpServerBuilder::block_size_limit
//! [`Handler`]: server::Handler
//! [`tftpd-targz.rs`]: https://github.com/oblique/async-tftp-rs/blob/master/examples/tftpd-targz.rs
//!
//! [RFC 1350]: https://tools.ietf.org/html/rfc1350
//! [RFC 2347]: https://tools.ietf.org/html/rfc2347
//! [RFC 2348]: https://tools.ietf.org/html/rfc2348
//! [RFC 2349]: https://tools.ietf.org/html/rfc2349

pub mod server;

/// Packet definitions that are needed in public API.
pub mod packet;

mod error;
mod executor;
mod parse;
mod tests;
mod utils;

pub use crate::error::*;

/// Re-export of `async_trait:async_trait`.
pub use async_trait::async_trait;
