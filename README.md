# async-tftp

[![license][license badge]][license]
[![crates.io][crate badge]][crate]
[![docs][docs badge]][docs]

Executor agnostic async TFTP implementation, written with [smol]
building blocks. Currently it implements only server side.

The following RFCs are implemented:

* [RFC 1350] - The TFTP Protocol (Revision 2).
* [RFC 2347] - TFTP Option Extension.
* [RFC 2348] - TFTP Blocksize Option.
* [RFC 2349] - TFTP Timeout Interval and Transfer Size Options.
* [RFC 7440] - TFTP Windowsize Option.

Features:

* Async implementation.
* Works with any runtime/executor.
* Serve read (RRQ) and write (WRQ) requests.
* Unlimited transfer file size (block number roll-over).
* You can set non-standard reply [`timeout`]. This is useful for faster
  file transfer in unstable environments.
* You can set [block size limit]. This is useful if you are accessing
  client through a VPN.
* You can implement your own [`Handler`] for more advance cases than
  just serving a directory. Check [`tftpd-targz.rs`] for an example.

# Example

```rust
use async_tftp::server::TftpServerBuilder;
use async_tftp::Result;

#[tokio::main] // or any other runtime/executor
async fn main() -> Result<()> {
    let tftpd = TftpServerBuilder::with_dir_ro(".")?.build().await?;
    tftpd.serve().await?;
    Ok(())
}
```

Add in `Cargo.toml`:

```toml
[dependencies]
async-tftp = "0.4"
# or any other runtime/executor
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Running examples with cargo

There are some examples included with this crate.
You can run them from a source checkout with cargo:

```bash
$ cargo run --example tftpd-dir
TFTP directory: ...
Listening on: 0.0.0.0:6969
^C

$ cargo run --example tftpd-targz <archive-path>
Listening on: 0.0.0.0:6969
^C
```

# License

[MIT][license]

[smol]: https://crates.io/crates/smol

[license]: LICENSE
[license badge]: https://img.shields.io/github/license/oblique/async-tftp-rs
[crate]: https://crates.io/crates/async-tftp
[crate badge]: https://img.shields.io/crates/v/async-tftp
[docs]: https://docs.rs/async-tftp
[docs badge]: https://docs.rs/async-tftp/badge.svg

[`timeout`]: https://docs.rs/async-tftp/latest/async_tftp/server/struct.TftpServerBuilder.html#method.timeout
[block size limit]: https://docs.rs/async-tftp/latest/async_tftp/server/struct.TftpServerBuilder.html#method.block_size_limit
[`Handler`]: https://docs.rs/async-tftp/latest/async_tftp/server/trait.Handler.html
[`tftpd-targz.rs`]: https://github.com/oblique/async-tftp-rs/blob/master/examples/tftpd-targz.rs

[RFC 1350]: https://tools.ietf.org/html/rfc1350
[RFC 2347]: https://tools.ietf.org/html/rfc2347
[RFC 2348]: https://tools.ietf.org/html/rfc2348
[RFC 2349]: https://tools.ietf.org/html/rfc2349
[RFC 7440]: https://tools.ietf.org/html/rfc7440
