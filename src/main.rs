#![feature(box_syntax)]

pub mod config;
pub mod generated;
pub mod interface;
pub mod internal;
pub mod network;
pub mod router;
pub mod utils;

#[macro_use]
extern crate lazy_static;

use crate::interface::Device;
use crate::network::Network;
use tokio::prelude::future;
use tokio::prelude::future::Future;
use tokio::sync::mpsc;

fn main() {
    let server = future::lazy(|| {
        let (interface_io_tx, interface_io_rx) = mpsc::unbounded_channel();
        let (message_bus_tx, message_bus_rx) = mpsc::unbounded_channel();

        let device = Device::new(message_bus_rx, interface_io_tx).map_err(|_| ());
        let net = Network::new(interface_io_rx, message_bus_tx).map_err(|_| ());

        tokio::spawn(device);
        tokio::spawn(net);
        Ok(())
    });

    tokio::run(server);
}
