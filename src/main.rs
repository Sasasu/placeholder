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

use crate::config::ARG;
use crate::interface::Device;
use crate::network::Network;
use env_logger::Builder;
use env_logger::Target;
use log::LevelFilter;
use tokio::prelude::future::{lazy, Future};
use tokio::sync::mpsc;

fn main() {
    Builder::from_default_env()
        .filter_level({
            let verbose = ARG.occurrences_of("verbosity") as usize;

            // verbose = 0 and -q is not set means there is no parameter pass in
            // set verbose to max
            if ARG.is_present("quiet") {
                LevelFilter::Off
            } else {
                match verbose {
                    1 => LevelFilter::Error,
                    2 => LevelFilter::Warn,
                    3 => LevelFilter::Info,
                    4 => LevelFilter::Debug,
                    _ => LevelFilter::Trace,
                }
            }
        })
        .target(Target::Stderr)
        .init();

    let server = lazy(|| {
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
