pub mod socket;

use crate::config::Config;
use crate::generated::transport as proto;
use crate::internal::message::Message;
use crate::internal::package::Package;
use crate::network::socket::Socket;
use crate::router::{Host, Router};
use log::*;
use std::convert::From;
use std::io;
use std::net::SocketAddr;
use tokio::prelude::future::lazy;
use tokio::prelude::Stream;
use tokio::prelude::{Async, Future};
use tokio::sync::mpsc;

lazy_static! {
    pub static ref SELF: proto::Node = {
        let c = Config::get();
        let mut myself = proto::Node::new();
        myself.set_sub_net(c.get_v4().octets().to_vec());
        myself.set_net_mask(c.get_v4_mask());
        myself.set_name(c.name.clone());
        myself.set_jump(1);
        myself
    };
}
pub struct Network {
    rx: mpsc::UnboundedReceiver<Package>,
    tx: mpsc::UnboundedSender<Package>,

    router_send: mpsc::UnboundedSender<(Option<SocketAddr>, Message)>,
    router_receiver: mpsc::UnboundedReceiver<(Option<SocketAddr>, Message)>,

    socket_send: mpsc::UnboundedSender<(SocketAddr, Message)>,
    socket_receiver: mpsc::UnboundedReceiver<(SocketAddr, Message)>,
}

impl Network {
    pub fn new(rx: mpsc::UnboundedReceiver<Package>, tx: mpsc::UnboundedSender<Package>) -> Self {
        let c = Config::get();

        let (sender_to_router, _r) = mpsc::unbounded_channel();
        let (_s, receiver_from_router) = mpsc::unbounded_channel();
        let router = Router::new(_s, _r);

        let (mut sender_to_socket, _r) = mpsc::unbounded_channel();
        let (_s, receiver_from_socket) = mpsc::unbounded_channel();
        let socket = Socket::new(_s, _r);

        // add my self to router table
        router.insert_to_table(
            c.get_v4().into(),
            c.get_v4_mask() as u16,
            c.name.clone(),
            Host::Localhost,
        );

        // prepare hello message to other node
        // clone for all servers
        for host in &c.servers {
            info!("connecting {:?}", host);
            let addr = SocketAddr::new(host.address.parse().unwrap(), host.port);
            socket.connect(&addr).unwrap();
            sender_to_socket
                .try_send((addr, Message::AddNodeWrite(SELF.clone())))
                .unwrap();
        }

        tokio::spawn(lazy(|| {
            router.map_err(|e| {
                panic!(e);
            })
        }));

        tokio::spawn(lazy(|| {
            socket.map_err(|e| {
                panic!(e);
            })
        }));

        Network {
            rx,
            tx,
            socket_send: sender_to_socket,
            socket_receiver: receiver_from_socket,
            router_send: sender_to_router,
            router_receiver: receiver_from_router,
        }
    }

    pub fn send_to_router(&mut self, addr: Option<SocketAddr>, message: Message) {
        self.router_send.try_send((addr, message)).unwrap();
    }
}

impl Future for Network {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        loop {
            match self.router_receiver.poll()? {
                Async::Ready(Some((addr, message))) => match message {
                    Message::DoNoting => {}
                    Message::InterfaceWrite(package) => {
                        self.tx.try_send(package).unwrap();
                    }
                    Message::PackageShareWrite(package, ttl) => self
                        .socket_send
                        .try_send((addr.unwrap(), Message::PackageShareWrite(package, ttl)))
                        .unwrap(),
                    Message::AddNodeWrite(node) => {
                        self.socket_send
                            .try_send((addr.unwrap(), Message::AddNodeWrite(node)))
                            .unwrap();
                    }
                    other => {
                        self.send_to_router(addr, other);
                    }
                },
                Async::Ready(None) => {
                    panic!();
                }
                Async::NotReady => break,
            }
        }

        loop {
            match self.socket_receiver.poll()? {
                Async::Ready(Some((addr, message))) => {
                    self.send_to_router(Some(addr), message);
                }
                Async::Ready(None) => panic!(),
                Async::NotReady => break,
            }
        }

        loop {
            match self.rx.poll()? {
                Async::Ready(Some(package)) => {
                    self.send_to_router(None, Message::InterfaceRead(package));
                }
                Async::Ready(None) => panic!(),
                Async::NotReady => break,
            };
        }

        Ok(Async::NotReady)
    }
}

pub enum Error {
    IoError(io::Error),
    RecvError(mpsc::error::UnboundedRecvError),
}

impl From<mpsc::error::UnboundedRecvError> for Error {
    fn from(e: mpsc::error::UnboundedRecvError) -> Self {
        Error::RecvError(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IoError(e)
    }
}
