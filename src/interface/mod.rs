pub mod tuntap_mio;
pub mod tuntap_tokio;

use crate::config::Config;
use crate::interface::tuntap_tokio::TunTap;
use crate::internal::error::Error;
use crate::internal::package::{Buffer, Package};
use crate::utils::*;
use log::*;
use serde::{Deserialize, Serialize};
use std::collections::linked_list::LinkedList;
use std::ffi::CString;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::prelude::stream::Stream;
use tokio::prelude::{Async, Future};
use tokio::sync::mpsc;

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

pub struct Device {
    interface: TunTap,
    receiver_net: mpsc::UnboundedReceiver<Package>,
    sender_net: mpsc::UnboundedSender<Package>,
    buffer: LinkedList<Vec<u8>>,
}

impl Device {
    pub fn new(rx: mpsc::UnboundedReceiver<Package>, tx: mpsc::UnboundedSender<Package>) -> Self {
        let c = Config::get();
        let interface = TunTap::new(&c.device_name, c.device_type);

        run_command(&c.get_env(), &c.ifup);

        Device {
            interface,
            receiver_net: rx,
            sender_net: tx,
            buffer: LinkedList::new(),
        }
    }
}

impl Future for Device {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        loop {
            match self.receiver_net.poll()? {
                Async::Ready(Some(p)) => {
                    self.buffer.push_back(p.raw_package);
                }
                Async::Ready(None) => panic!(),
                Async::NotReady => break,
            }
        }

        loop {
            let mut buffer = Buffer::get();
            match self.interface.poll_read(buffer.as_mut_slice())? {
                Async::Ready(nbytes) => {
                    Buffer::set_len(buffer.as_mut(), nbytes);
                    let package = Package::from_buffer(buffer);
                    self.sender_net.try_send(package).unwrap();
                }
                Async::NotReady => break,
            }
        }

        while let Some(buff) = self.buffer.pop_front() {
            match self.interface.poll_write(&buff)? {
                Async::Ready(nbytes) => {
                    info!("write {} bytes to interface", nbytes);
                }
                Async::NotReady => {
                    self.buffer.push_back(buff);
                    break;
                }
            }
        }

        Ok(Async::NotReady)
    }
}
