pub mod tuntap;

use crate::config::Config;
use crate::interface::tuntap::TunTap;
use crate::internal::error::Error;
use crate::internal::package::{Buffer, Package};
use crate::utils::*;
use log::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::prelude::stream::Stream;
use tokio::prelude::{task, Async, Future};
use tokio::sync::mpsc;

pub struct Device {
    interface: TunTap,
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
            is_reading: Arc::new(false.into()),
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
                    tokio::spawn(self.interface.write(p.raw_package).and_then(|v| {
                        trace!("interface write {} bytes", v.len());
                        Ok(())
                    }));
                }
                Async::Ready(None) => panic!(),
                Async::NotReady => break,
            }
        }

        if !self.is_reading.load(Ordering::SeqCst) {
            self.is_reading.store(true, Ordering::SeqCst);

            let mut sender = self.sender_net.clone();
            let is_reading = self.is_reading.clone();
            let task = task::current();

            let buffer = Buffer::get();

            tokio::spawn(self.interface.read(buffer).and_then(move |s| {
                info!("read {} bytes from tuntap", s.len());
                let package = Package::from_buffer(s);

                sender.try_send(package).unwrap();
                is_reading.store(false, Ordering::SeqCst);
                task.notify();

                Ok(())
            }));
        }

        Ok(Async::NotReady)
    }
}
