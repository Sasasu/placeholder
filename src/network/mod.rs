use crate::config::Config;
use crate::generated::transport as proto;
use crate::internal::message::Message;
use crate::internal::package::{Buffer, Package};
use crate::router::{Peer, Router};
use log::*;
use protobuf::Message as PbMessage;
use std::collections::LinkedList;
use std::convert::From;
use std::io;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::prelude::Stream;
use tokio::prelude::{Async, Future};
use tokio::sync::mpsc;

pub struct Network {
    socket: UdpSocket,
    rx: mpsc::UnboundedReceiver<Package>,
    tx: mpsc::UnboundedSender<(Option<SocketAddr>, Message)>,
    send_buffer: LinkedList<(SocketAddr, Vec<u8>)>,
    router_buffer: LinkedList<(Option<SocketAddr>, Message)>,
}

impl Network {
    pub fn new(
        rx: mpsc::UnboundedReceiver<Package>,
        tx: mpsc::UnboundedSender<(Option<SocketAddr>, Message)>,
    ) -> Self {
        let c = Config::get();
        info!("binding socket: {}:{}", "0.0.0.0", c.port);
        let socket = UdpSocket::bind(&SocketAddr::new("0.0.0.0".parse().unwrap(), c.port))
            .expect("bind address failure");

        // add my self to router table
        Router::get().add_peer(&c.name, Peer::LocaleHost);
        Router::get().insert_to_table(c.get_v4().into(), c.get_v4_mask(), c.name.clone());

        // prepare hello message to other node
        let router_buffer = {
            let mut router_buffer = LinkedList::new();
            // myself
            let nodes = vec![{
                let mut myself = proto::AddNode::new();
                myself.set_sub_net_v4(c.get_v4().octets().to_vec());
                myself.set_net_mask_v4(c.get_v4_mask());
                myself.set_name(c.name.clone());
                myself.set_jump(1);
                myself
            }];

            // clone for all servers
            for host in &c.servers {
                info!("connecting {:?}", host);
                let addr = SocketAddr::new(host.address.parse().unwrap(), host.port);
                socket.connect(&addr).unwrap();
                Router::get().add_peer(&host.name, Peer::Addr(addr));
                router_buffer.push_back((Some(addr), Message::AddNodeWrite(nodes.clone())));
            }
            router_buffer
        };

        Network {
            rx,
            tx,
            socket,
            router_buffer,
            send_buffer: LinkedList::new(),
        }
    }

    pub fn add_to_send_list(&mut self, addr: SocketAddr, buffer: Vec<u8>) {
        self.send_buffer.push_back((addr, buffer));
    }
}

impl Future for Network {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        loop {
            let mut buffer = Buffer::get();
            match self.socket.poll_recv_from(buffer.as_mut_slice())? {
                Async::Ready((read_size, addr)) => {
                    Buffer::set_len(buffer.as_mut(), read_size);
                    info!("receive {} bytes from {}", read_size, addr);
                    let message_to_router = Message::from_protobuf(Some(addr), buffer);
                    self.router_buffer.push_back(message_to_router);
                }
                Async::NotReady => {
                    Buffer::put_back(buffer);
                    break;
                }
            };
        }

        while let Some((addr, message)) = self.router_buffer.pop_front() {
            match message {
                Message::DoNoting => {}
                Message::InterfaceWrite(package) => {
                    self.tx
                        .try_send((None, Message::InterfaceWrite(package)))
                        .unwrap();
                }
                Message::PackageShareWrite(package, ttl) => self.add_to_send_list(
                    addr.unwrap(),
                    Message::PackageShareWrite(package, ttl)
                        .into_protobuf()
                        .write_to_bytes()
                        .unwrap(),
                ),
                Message::ShareNodeWrite(nodes) => self.add_to_send_list(
                    addr.unwrap(),
                    Message::ShareNodeWrite(nodes)
                        .into_protobuf()
                        .write_to_bytes()
                        .unwrap(),
                ),
                Message::WhoHasNodeWrite(nodes) => self.add_to_send_list(
                    addr.unwrap(),
                    Message::WhoHasNodeWrite(nodes)
                        .into_protobuf()
                        .write_to_bytes()
                        .unwrap(),
                ),
                Message::AddNodeWrite(nodes) => self.add_to_send_list(
                    addr.unwrap(),
                    Message::AddNodeWrite(nodes)
                        .into_protobuf()
                        .write_to_bytes()
                        .unwrap(),
                ),
                other => {
                    let ans = Router::get().router_message((addr, other));
                    self.router_buffer.push_back(ans);
                }
            }
        }

        // net send
        while let Some((addr, buffer)) = self.send_buffer.pop_front() {
            let send_res = match self.socket.poll_send_to(&buffer, &addr) {
                Ok(t) => t,
                Err(e) => {
                    warn!("send package to {} error: {}, try remove peer", addr, e);
                    Async::NotReady
                }
            };

            match send_res {
                Async::Ready(send_size) => {
                    info!("send {} bytes to {}", send_size, addr);
                }
                Async::NotReady => {
                    self.add_to_send_list(addr, buffer);
                    break;
                }
            }
        }

        loop {
            match self.rx.poll()? {
                Async::Ready(Some(package)) => {
                    let message =
                        Router::get().router_message((None, Message::InterfaceRead(package)));
                    self.router_buffer.push_back(message);
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
