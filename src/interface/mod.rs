use crate::config::Config;
use crate::internal::package::{Buffer, Package};
use crate::utils::*;
use libc;
use log::*;
use serde::{Deserialize, Serialize};
use std::collections::linked_list::LinkedList;
use std::convert::From;
use std::ffi::CString;
use std::fs::File as StdFile;
use std::io::Error as IoError;
use std::os::raw::c_char;
use std::os::unix::io::FromRawFd;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use tokio::prelude::stream::Stream;
use tokio::prelude::{future, Async, Future};
use tokio::sync::mpsc;

#[link(name = "libtuntap", kind = "static")]
extern "C" {
    /// set up a tun device.
    ///
    /// return 0 if success.
    /// return other if failure, the value definition see man errno.
    fn setup_tun_device(fd: i32, ifname: *const c_char) -> i32;
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

#[derive(Debug)]
pub struct Device {
    interface: Arc<Mutex<device_interface::DeviceInterface>>,
    rx: mpsc::UnboundedReceiver<Package>,
    tx: Option<mpsc::UnboundedSender<Package>>,
    _write_buffer: LinkedList<Package>,
}

lazy_static! {
    static ref TUN_PATH: CString = CString::new("/dev/net/tun").unwrap();
    static ref TAP_PATH: CString = CString::new("/dev/tap0").unwrap();
}

impl Device {
    pub fn new(rx: mpsc::UnboundedReceiver<Package>, tx: mpsc::UnboundedSender<Package>) -> Self {
        let c = Config::get();
        info!(
            "crate new device: {:?}, type: {:?}",
            c.device_name, c.device_type
        );
        let c_ifname = CString::new(c.device_name.clone()).unwrap();

        let interface = unsafe {
            let fd = match c.device_type {
                // NOTE: maybe tokio-fs will support FIFO, pipe, unix socks or string device
                // in the future, the open sys-call can use O_NONBLOCK
                // for now, use O_NONBLOCK will case panic in tokio-fs
                Type::Tun => libc::open(TUN_PATH.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK),
                Type::Tap => libc::open(TAP_PATH.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK),
            };
            if fd < 0 {
                panic!("open file error: {}", IoError::last_os_error());
            };

            if setup_tun_device(fd, c_ifname.as_ptr()) < 0 {
                panic!(
                    "use ioctl to set tun name IO error: {}",
                    IoError::last_os_error()
                );
            }
            StdFile::from_raw_fd(fd)
        };

        run_command(&c.get_env(), &c.ifup);

        Device {
            rx,
            tx: tx.into(),
            interface: Mutex::new(device_interface::DeviceInterface::new(interface)).into(),
            _write_buffer: LinkedList::new(),
        }
    }
}

impl Future for Device {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        // recv from net
        loop {
            match self.rx.poll()? {
                Async::Ready(Some(p)) => {
                    trace!("interface async send {} bytes", p.len());
                    self._write_buffer.push_back(p);
                }
                Async::Ready(None) => panic!("DW: get None"),
                Async::NotReady => break,
            }
        }

        while let Some(package) = self._write_buffer.pop_front() {
            trace!("interface get {} bytes to write, start task", package.len());
            let interface = self.interface.clone();
            let (tx, mut rx) = tokio::sync::oneshot::channel();

            tx.send(package).unwrap();

            tokio::spawn(future::lazy(move || {
                let package = rx.try_recv().unwrap();
                let size = {
                    trace!("write lock");
                    let t = interface
                        .lock()
                        .unwrap()
                        .block_write(package.into())
                        .unwrap();
                    trace!("write unlock");
                    t
                };
                trace!("interface write {} bytes successfully", size);
                Ok(())
            }));
        }

        match self.tx {
            None => { /* pass */ }
            Some(ref tx) => {
                use std::mem;
                info!("starting interface read task");

                let mut tx = tx.clone();
                mem::replace(&mut self.tx, None);

                let interface = self.interface.clone();
                let mut buffer = Buffer::get();

                tokio::spawn(future::lazy(move || loop {
                    sleep(Duration::from_nanos(50000));

                    match interface.lock().unwrap().block_read(&mut buffer) {
                        Some(size) => {
                            Buffer::set_len(&mut buffer, size);
                            let _b = mem::replace(&mut buffer, Buffer::get());
                            let package = Package::from_buffer(_b);
                            trace!("interface send {} bytes to net", package.len());
                            tx.try_send(package).unwrap();
                        }
                        None => {
                            continue;
                        }
                    }

                    if false {
                        // hit compiler, this is a `FnOnce<Ok(())>`
                        return Ok(());
                    }
                }));
            }
        }

        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    RecvError(mpsc::error::UnboundedRecvError),
}

impl From<mpsc::error::UnboundedRecvError> for Error {
    fn from(e: mpsc::error::UnboundedRecvError) -> Self {
        Error::RecvError(e)
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::IoError(e)
    }
}

pub mod device_interface {
    use log::*;
    use std::fs::File;
    use std::io::ErrorKind;
    use std::io::{Read, Write};

    #[derive(Debug)]
    pub struct DeviceInterface {
        pub interface: File,
    }

    impl DeviceInterface {
        pub fn new(interface: File) -> Self {
            DeviceInterface { interface }
        }

        pub fn block_read(&mut self, buffer: &mut Vec<u8>) -> Option<usize> {
            match self.interface.read(buffer.as_mut_slice()) {
                Ok(size) => Some(size),
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        None
                    } else {
                        panic!("read error: {}", e);
                    }
                }
            }
        }

        pub fn block_write(&mut self, buffer: Vec<u8>) -> Option<usize> {
            info!("interface start write {} bytes", buffer.len());
            let size = self
                .interface
                .write(buffer.as_ref())
                .expect("tun interface write error");
            info!("interface finish write {} bytes", size);
            Some(size)
        }
    }
}
