#![cfg(feature = "external-client-tests")]
#![cfg(any(target_os = "linux", target_os = "windows"))]
#![allow(unused_imports)]

use std::env;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
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

    // Redirect output to /dev/null
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

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
        .status()
        .expect("atftp is not installed");

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

    // Redirect output to null
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

    if let Some(block_size) = block_size {
        cmd.arg(format!("-b{}", block_size));
    }

    cmd.arg("-i")
        .arg(format!("-p{}", server.port()))
        .arg(server.ip().to_string())
        .arg("get")
        .arg(filename)
        .arg(&path)
        .status()
        .expect("tftp is not installed");

    let data = fs::read(path)?;
    Ok(md5::compute(data))
}
