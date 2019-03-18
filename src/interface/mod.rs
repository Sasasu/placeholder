pub mod tuntap;

pub use crate::interface::tuntap::Type;

use crate::config::Config;
use crate::interface::tuntap::TunTap;
use crate::internal::error::Error;
use crate::internal::package::{Buffer, Package};
use crate::utils::*;
use log::*;
use std::collections::linked_list::LinkedList;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::prelude::stream::Stream;
use tokio::prelude::{Async, Future};
use tokio::sync::mpsc;

pub struct Device {
    interface: TunTap,
    buffer: LinkedList<Package>,
    receiver_net: mpsc::UnboundedReceiver<Package>,
    sender_net: mpsc::UnboundedSender<Package>,
    is_reading: Arc<AtomicBool>,
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
            is_reading: Arc::new(false.into()),
        }
    }
}

impl Future for Device {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        // recv from net
        loop {
            match self.receiver_net.poll()? {
                Async::Ready(Some(p)) => {
                    trace!("interface async write {} bytes", p.len());
                    self.buffer.push_back(p);
                }
                Async::Ready(None) => panic!(),
                Async::NotReady => break,
            }
        }

        loop {
            match self.buffer.pop_front() {
                None => break,
                Some(package) => {
                    tokio::spawn(
                        self.interface
                            .write(package.raw_package)
                            .and_then(|_| Ok(())),
                    );
                }
            }
        }

        if false == self.is_reading.load(Ordering::SeqCst) {
            use tokio::prelude::task;
            info!("read start");
            self.is_reading.store(true, Ordering::SeqCst);

            let is_reading = self.is_reading.clone();
            let mut sender = self.sender_net.clone();
            let task = task::current();

            let buffer = Buffer::get();
            let s = self
                .interface
                .read(buffer)
                .then(move |s| {
                    info!("read task started");
                    s
                })
                .and_then(move |s| {
                    info!("read {} bytes from tuntap", s.len());
                    let package = Package::from_buffer(s);
                    sender.try_send(package).unwrap();

                    is_reading.store(false, Ordering::SeqCst);
                    task.notify();

                    Ok(())
                });

            tokio::spawn(s);
        }

        Ok(Async::NotReady)
    }
}
