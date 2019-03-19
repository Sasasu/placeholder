use crate::internal::package::Buffer;
use log::*;
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::io::Error as IoError;
use std::ops::AddAssign;
use std::os::raw::c_char;
use std::os::unix::io::RawFd;
use std::sync::{Arc, RwLock};
use tokio::executor::DefaultExecutor;
use tokio::prelude::Future;
use tokio_linux_aio::AioContext;

lazy_static! {
    static ref TUN_PATH: CString = CString::new("/dev/net/tun").unwrap();
    static ref TAP_PATH: CString = CString::new("/dev/tap0").unwrap();
}

#[derive(Deserialize, Serialize, Debug, Eq, PartialEq, Copy, Clone)]
pub enum Type {
    /// Tun device read and write IP package
    #[serde(rename = "tun")]
    Tun,
    /// Not unimplemented!
    ///
    /// Tap device read and write ethernet frame
    #[serde(rename = "tap")]
    Tap,
}

pub struct TunTap {
    fd: RawFd,
    reade_nbytes: Arc<RwLock<u64>>,
    write_nbytes: Arc<RwLock<u64>>,
}

impl TunTap {
    pub fn new(device_name: &str, t: Type) -> Self {
        info!("crate new device: {:?}, type: {:?}", device_name, t);
        let c_device_name = CString::new(device_name.clone()).unwrap();

        let fd = unsafe {
            let fd = match t {
                // ??????
                // open with libc::O_DIRECT get EINVAL 22, maybe because /dev/tun/tun is a
                // char device not block device, but open witch libc::O_NONBLOCK get os error -11 (unknown)
                // open with open libc::O_RDWR works fine
                Type::Tun => libc::open(TUN_PATH.as_ptr(), libc::O_RDWR),
                Type::Tap => libc::open(TAP_PATH.as_ptr(), libc::O_RDWR),
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

        TunTap {
            fd,
            reade_nbytes: Arc::new(0.into()),
            write_nbytes: Arc::new(0.into()),
        }
    }
}

impl TunTap {
    pub fn read(&mut self, buf: Vec<u8>) -> impl Future<Item = Vec<u8>, Error = ()> {
        info!("READ");
        let n = self.write_nbytes.clone();

        AioContext::new(&DefaultExecutor::current(), 2)
            .expect("aio context crate error")
            .read(self.fd, *self.write_nbytes.read().unwrap(), buf)
            .map_err(move |e| panic!("read {:?}", e))
            .and_then(move |(mut buf, nbytes)| {
                Buffer::set_len(&mut buf, nbytes as usize);
                n.write().unwrap().add_assign(nbytes);
                Ok(buf)
            })
    }

    pub fn write(&mut self, buf: Vec<u8>) -> impl Future<Item = Vec<u8>, Error = ()> {
        info!("WRITE");
        let n = self.reade_nbytes.clone();

        AioContext::new(&DefaultExecutor::current(), 2)
            .expect("aio context crate error")
            .write(self.fd, 0 /*self.reade_nbytes.read().unwrap()*/, buf)
            .map_err(move |e| panic!("write {:?}", e))
            .and_then(move |(mut buf, nbytes)| {
                Buffer::set_len(&mut buf, nbytes as usize);
                n.write().unwrap().add_assign(nbytes);
                Ok(buf)
            })
    }
}

#[link(name = "libtuntap", kind = "static")]
extern "C" {
    /// set up a tun device.
    ///
    /// return 0 if success.
    /// return other if failure, the value definition see man errno.
    fn setup_tun_device(fd: i32, ifname: *const c_char) -> i32;
}
