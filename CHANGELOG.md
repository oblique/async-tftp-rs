# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [unreleased]

## [0.4.2] - 2025-12-02

### Added

- write_req: Close file after writing [#21](https://github.com/oblique/async-tftp-rs/pull/21)

## [0.4.1] - 2025-10-25

### Changed

- Add support for windowsize option (RFC 7440) ([#17](https://github.com/oblique/async-tftp-rs/pull/17))
- Use RTITIT instead of async-trait
- Implement parsing without nom
- Remove `num-traits` dependency
- Update all dependencies
- Use `tokio` in examples

## [0.3.6] - 2022-12-16

### Changed

- Send responses and errors from the bind address [#14](https://github.com/oblique/async-tftp-rs/pull/14)
- Upgrade dependencies

## [0.3.5] - 2021-01-28

### Changed

- Upgrade to `bytes` 1.0
- Migrate from `async-mutex` to `async-lock`
- Upgdate other dependencies

## [0.3.4] - 2020-12-13

### Changed

- Upgrade to `bytes` 0.6.0
- Upgrade to `nom` 6.0.1
- Upgrade other dependencies
- Use async-executor instead of FuturesUnordered

## [0.3.3] - 2020-09-14

### Changed

- Upgrade to the v1 of [smol] building blocks.

## [0.3.2] - 2020-08-31

### Changed

- Remove `once_cell` from dependencies.
- Upgrade to new smol building blocks.

## [0.3.1] - 2020-08-22

### Improve

- Rewrite `tftpd-targz.rs` example with `async-tar` and `async-compression`
  crates.
- Use only `alloc` feature flag for `futures-util`.

## [0.3.0] - 2020-08-17

### Added

- async-tftp is now runtime/executor agnostic thanks to [smol] building
  blocks. You can even run it with a simple `block_on`.
- Added an example on how you can serve files from a tar.gz.
- Added `TftpServerBuilder::std_socket`.

### Changed

- Because `use-tokio` feature flag is removed, `Handler` now only accepts
  `futures_io::AsyncRead` and `futures_io::AsyncWrite`.
- `TftpServerBuilder::socket` now accepts `async_io::Async<std::net::UdpSocket>`.

### Removed

- Removed `use-async-std` feature flag.
- Removed `use-tokio` feature flag.
- Removed `async_tftp::log::set_log_level`.

## [0.2.0] - 2020-02-08

### Added

- Handle write requests.
- Added `TftpServerBuilder::with_dir_wo` that handles only write
  requests.
- Added `TftpServerBuilder::with_dir_rw` that handles read and write
  requests.
- Added `use-async-std` feature flag, to enable async-std 1.0 integration (default).
- Added `use-tokio` feature flag, to enable Tokio 0.2 integration.

### Changed

- `Handler` trait needs a `Writer` associated type.
- `DirRoHandler` is renamed to `DirHandler`.
- `DirHandler::new` now requires initialization flags.

## [0.1.3] - 2019-11-20

### Added

- Minor improvements for read request.
- Added tests for non-default block size.

## [0.1.2] - 2019-11-20

### Added

- You can now set the maximum send retries of a data block via
  `TftpServerBuilder::max_send_retries`. Default is 100 retries.
- You can now produce a serve request failure on the first `read`

## [0.1.1] - 2019-11-17

### Fixed

- Improve test cases.

## [0.1.0] - 2019-11-17

[First release](https://docs.rs/async-tftp/0.1.0)


[unreleased]: https://github.com/oblique/async-tftp-rs/compare/0.4.2...HEAD
[0.4.2]: https://github.com/oblique/async-tftp-rs/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/oblique/async-tftp-rs/compare/0.3.6...0.4.1
[0.3.6]: https://github.com/oblique/async-tftp-rs/compare/0.3.5...0.3.6
[0.3.5]: https://github.com/oblique/async-tftp-rs/compare/0.3.4...0.3.5
[0.3.4]: https://github.com/oblique/async-tftp-rs/compare/0.3.3...0.3.4
[0.3.3]: https://github.com/oblique/async-tftp-rs/compare/0.3.2...0.3.3
[0.3.2]: https://github.com/oblique/async-tftp-rs/compare/0.3.1...0.3.2
[0.3.1]: https://github.com/oblique/async-tftp-rs/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/oblique/async-tftp-rs/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/oblique/async-tftp-rs/compare/0.1.3...0.2.0
[0.1.3]: https://github.com/oblique/async-tftp-rs/compare/0.1.2...0.1.3
[0.1.2]: https://github.com/oblique/async-tftp-rs/compare/0.1.1...0.1.2
[0.1.1]: https://github.com/oblique/async-tftp-rs/compare/0.1.0...0.1.1
[0.1.0]: https://github.com/oblique/async-tftp-rs/releases/tag/0.1.0

[smol]: https://github.com/stjepang/smol
