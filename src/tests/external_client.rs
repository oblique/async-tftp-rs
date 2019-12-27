#![cfg(feature = "external-client-tests")]
#![allow(unused_imports)]

use std::env;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

#[cfg(target_os = "linux")]
pub fn external_tftp_recv(
    filename: &str,
    server: SocketAddr,
    block_size: Option<u16>,
) -> io::Result<md5::Digest> {
    let tmp = tempdir()?;
    let path = tmp.path().join("data");

    // Expects `atftp` to be installed
    let mut cmd = Command::new("atftp");

    if let Some(block_size) = block_size {
        cmd.arg("--option").arg(format!("blksize {}", block_size));
    }

    cmd.arg("-g")
        .arg("-l")
        .arg(&path)
        .arg("-r")
        .arg(filename)
        .arg(server.ip().to_string())
        .arg(server.port().to_string())
        .status()?;

    let data = fs::read(path)?;
    Ok(md5::compute(data))
}

#[cfg(target_os = "windows")]
pub fn external_tftp_recv(
    filename: &str,
    server: SocketAddr,
    block_size: Option<u16>,
) -> io::Result<md5::Digest> {
    let tmp = tempdir()?;
    let path = tmp.path().join("data");

    // Expects `https://www.winagents.com/downloads/tftp.exe` is in `PATH`
    let mut cmd = Command::new("tftp");

    if let Some(block_size) = block_size {
        cmd.arg(format!("-b{}", block_size));
    }

    cmd.arg("-i")
        .arg(format!("-p{}", server.port()))
        .arg(server.ip().to_string())
        .arg("get")
        .arg(filename)
        .arg(&path)
        .status()?;

    let data = fs::read(path)?;
    Ok(md5::compute(data))
}
