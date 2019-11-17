# async-tftp

[![license][license badge]][license]
[![crates.io][crate badge]][crate]
[![docs][docs badge]][docs]

This library provides TFTP async implementation. Currently it implements only
server side which can serve read requests. This is the most used scenario in
our modern days.

## Example

```rust
use async_tftp::server::TftpServerBuilder;
use async_tftp::Result;

fn main() -> Result<()> {
   async_std::task::block_on(async {
       let tftpd = TftpServerBuilder::with_dir_ro(".")?.build().await?;
       tftpd.serve().await?;
       Ok(())
   })
}
```

## Features

* Async implementation.
* Serve read requests.
* Unlimited transfer file size (block number roll-over).
* You can set non-standard reply [`timeout`]. This is useful for faster
  file transfer in unstable environments.
* You can set [block size limit]. This is useful if you are accessing
  client through a VPN.
* You can implement your own [`Handler`] for more advance cases than
  just serving a directory.

## Implemented RFCs

* [RFC 1350] - The TFTP Protocol (Revision 2).
* [RFC 2347] - TFTP Option Extension.
* [RFC 2348] - TFTP Blocksize Option.
* [RFC 2349] - TFTP Timeout Interval and Transfer Size Options.


[license]: LICENSE
[license badge]: https://img.shields.io/github/license/oblique/async-tftp-rs
[crate]: https://crates.io/crates/async-tftp
[crate badge]: https://img.shields.io/crates/v/async-tftp
[docs]: https://docs.rs/async-tftp
[docs badge]: https://docs.rs/async-tftp/badge.svg

[`timeout`]: https://docs.rs/async-tftp/latest/async_tftp/server/struct.TftpServerBuilder.html#method.timeout
[block size limit]: https://docs.rs/async-tftp/latest/async_tftp/server/struct.TftpServerBuilder.html#method.block_size_limit
[`Handler`]: https://docs.rs/async-tftp/latest/async_tftp/server/trait.Handler.html

[RFC 1350]: https://tools.ietf.org/html/rfc1350
[RFC 2347]: https://tools.ietf.org/html/rfc2347
[RFC 2348]: https://tools.ietf.org/html/rfc2348
[RFC 2349]: https://tools.ietf.org/html/rfc2349
