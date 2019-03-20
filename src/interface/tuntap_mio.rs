use libc::{close, read, write};
use log::*;
use mio::unix::EventedFd;
use mio::{Evented, Poll, PollOpt, Ready, Token};
use std::ffi::c_void;
use std::ffi::CString;
use std::io::Error as IoError;
use std::io::{Error, Read, Write};
use std::os::raw::c_char;
use std::os::unix::io::RawFd;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::{io, mem};

use super::Type;
use super::TAP_PATH;
use super::TUN_PATH;

#[link(name = "libtuntap", kind = "static")]
extern "C" {
    /// set up a tun device.
    ///
    /// return 0 if success.
    /// return other if failure, the value definition see man errno.
    fn setup_tun_device(fd: i32, ifname: *const c_char) -> i32;
}

#[derive(Debug)]
pub struct TunTap {
    fd: RawFd,
}

impl TunTap {
    pub fn new(device_name: &str, t: Type) -> Self {
        info!("crate new device: {:?}, type: {:?}", device_name, t);
        let c_device_name = CString::new(device_name).unwrap();

        let fd = unsafe {
            let fd = match t {
                Type::Tun => libc::open(TUN_PATH.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK),
                Type::Tap => libc::open(TAP_PATH.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK),
            };

            if fd < 0 {
                panic!("open file error: {}", IoError::last_os_error());
            }

            if setup_tun_device(fd, c_device_name.as_ptr()) < 0 {
                panic!(
                    "use ioctl to set tun name IO error: {}",
                    IoError::last_os_error()
                );
            }

            fd
        };

        TunTap { fd }
    }
}

impl Evented for TunTap {
    fn register(&self, poll: &Poll, token: Token, events: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, events, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        events: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, events, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

impl Read for TunTap {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = unsafe { read(self.fd, buf.as_mut_ptr() as *mut c_void, buf.len()) };
        if size == -1 {
            return Err(Error::last_os_error());
        }
        Ok(size as usize)
    }
}

impl Write for TunTap {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }

        let size = unsafe { write(self.fd, buf.as_ptr() as *mut c_void, buf.len()) };
        if size < 0 {
            return Err(Error::last_os_error());
        }
        Ok(size as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRawFd for TunTap {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl FromRawFd for TunTap {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { fd }
    }
}

impl IntoRawFd for TunTap {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.fd;
        mem::forget(self);
        fd
    }
}

impl Drop for TunTap {
    fn drop(&mut self) {
        unsafe {
            let _ = close(self.fd);
        }
    }
}
