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
    aio_context: AioContext,
    reade_bytes: Arc<RwLock<u64>>,
    write_bytes: Arc<RwLock<u64>>,
}

impl TunTap {
    pub fn new(device_name: &str, t: Type) -> Self {
        info!("crate new device: {:?}, type: {:?}", device_name, t);
        let c_device_name = CString::new(device_name.clone()).unwrap();

        let fd = unsafe {
            let fd = match t {
                Type::Tun => libc::open(TUN_PATH.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK),
                Type::Tap => libc::open(
                    TAP_PATH.as_ptr(),
                    libc::O_RDWR | libc::O_NONBLOCK | libc::O_DIRECT,
                ),
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

        let aio_context =
            AioContext::new(&DefaultExecutor::current(), 2).expect("aio context crate error");

        TunTap {
            fd,
            aio_context,
            reade_bytes: Arc::new(0.into()),
            write_bytes: Arc::new(0.into()),
        }
    }
}

impl TunTap {
    pub fn read(&mut self, buf: Vec<u8>) -> impl Future<Item = Vec<u8>, Error = ()> {
        let n = self.write_bytes.clone();

        self.aio_context
            .read(self.fd, *self.write_bytes.read().unwrap(), buf)
            .map_err(move |e| panic!("{:?}", e))
            .and_then(move |s| {
                n.write().unwrap().add_assign(s.len() as u64);
                Ok(s)
            })
    }

    pub fn write(&mut self, buf: Vec<u8>) -> impl Future<Item = Vec<u8>, Error = ()> {
        let n = self.reade_bytes.clone();

        self.aio_context
            .write(self.fd, *self.reade_bytes.read().unwrap(), buf)
            .map_err(move |e| panic!("{:?}", e))
            .and_then(move |s| {
                n.write().unwrap().add_assign(s.len() as u64);
                Ok(s)
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
