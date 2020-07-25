//! Async TFTP implementation, written with [smol] building blocks. Currently
//! it implements only server side.
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
//!     smol::run(async {
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
//! smol = "0.3"
//! async-tftp = "0.3"
//! ```
//!
//! For [tokio] you need to enable `tokio02` feature of [smol]:
//!
//! ```toml
//! smol = { version = "0.3", features = ["tokio02"] }
//! async-tftp = "0.3"
//! ```
//!
//! If you need to use it in other runtimes or if you need more control
//! then check the next section.
//!
//! # Advance way of using it with other async runtimes
//!
//! Rule of thumb: If you are using a runtime that does not use
//! [async-executor] crate for an executor, then you need start your
//! own [`async_executor::Executor`] and provide the spawner with
//! [`async_tftp::set_spawner`].
//!
//! **[async-std] example:**
//!
//! ```ignore
//! use async_tftp::server::TftpServerBuilder;
//! use async_tftp::Result;
//!
//! use futures_lite::future;
//! use std::thread;
//!
//! #[async_std::main]
//! async fn main() -> Result<()> {
//!     // Set explicit async-executor spawner
//!     let ex = Executor::new();
//!     async_tftp::set_spawner(ex.spawner());
//!
//!     // Start new thread that can handle both, async-executor tasks
//!     // and async-std tasks.
//!     thread::spawn(move || ex.run(future::pending::<()>()));
//!
//!     // Start tftp server
//!     let tftpd = TftpServerBuilder::with_dir_ro(".")?.build().await?;
//!     tftpd.serve().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! **[tokio] example:**
//!
//! For tokio there is one more requirement: You need to enter in tokio's
//! runtime context.
//!
//! ```ignore
//! use async_tftp::server::TftpServerBuilder;
//! use async_tftp::Result;
//!
//! use futures_lite::future;
//! use std::thread;
//! use tokio::runtime;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Set explicit async-executor spawner
//!     let ex = Executor::new();
//!     async_tftp::set_spawner(ex.spawner());
//!
//!     // Start new thread that can handle both, async-executor tasks
//!     // and tokio tasks.
//!     let handle = runtime::Handle::current();
//!     thread::spawn(move || handle.enter(|| ex.run(future::pending::<()>())));
//!
//!     // Start tftp server
//!     let tftpd = TftpServerBuilder::with_dir_ro(".")?.build().await?;
//!     tftpd.serve().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! [async-executor]: https://crates.io/crates/async-executor
//! [smol]: https://docs.rs/smol
//! [async-std]: https://docs.rs/async-std
//! [tokio]: https://docs.rs/tokio
//!
//! [`async_tftp::set_spawner`]: fn.set_spanwer.html
//! [`timeout`]: server/struct.TftpServerBuilder.html#method.timeout
//! [block size limit]: server/struct.TftpServerBuilder.html#method.block_size_limit
//! [`Handler`]: server/trait.Handler.html
//! [`async_executor::Executor`]: https://docs.rs/async-executor/0.1/async_executor/struct.Executor.html
//! [`tftpd-targz.rs`]: https://github.com/oblique/async-tftp-rs/blob/master/examples/tftpd-targz.rs
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
mod task;
mod tests;
mod utils;

pub use crate::error::*;
pub use crate::task::*;

/// Re-export of `async_trait:async_trait`.
pub use async_trait::async_trait;
