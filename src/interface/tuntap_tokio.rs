use super::Type;
use crate::interface::tuntap_mio::TunTap as TunTapMio;
use std::io;
use std::io::{Read, Write};
use tokio::io::Error;
use tokio::prelude::Async;
use tokio::reactor::PollEvented2 as PollEvented;
use tokio_io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct TunTap {
    io: PollEvented<TunTapMio>,
}

impl TunTap {
    pub fn new(device_name: &str, t: Type) -> Self {
        let io = PollEvented::new(TunTapMio::new(device_name, t));
        TunTap { io }
    }
}

impl Read for TunTap {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.io.read(bytes)
    }
}

impl Write for TunTap {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl AsyncRead for TunTap {}

impl AsyncWrite for TunTap {
    fn shutdown(&mut self) -> Result<Async<()>, Error> {
        Ok(().into())
    }
}
