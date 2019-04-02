use crate::config::Config;
use crate::internal::error::Error;
use crate::internal::message::Message;
use crate::internal::package::Buffer;
use log::*;
use net2::UdpBuilder;
use std::collections::linked_list::LinkedList;
use std::net::{IpAddr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::prelude::Stream;
use tokio::prelude::{Async, Future};
use tokio::reactor::Handle;
use tokio::sync::mpsc;

pub struct Socket {
    v6: UdpSocket,
    v4: UdpSocket,
    tx: mpsc::UnboundedSender<Message>,
    rx: mpsc::UnboundedReceiver<Message>,
    buffer: LinkedList<(SocketAddr, Vec<u8>)>,
}

impl Socket {
    pub fn new(tx: mpsc::UnboundedSender<Message>, rx: mpsc::UnboundedReceiver<Message>) -> Self {
        let c = Config::get();
        let v6_addr = SocketAddr::new("::".parse().unwrap(), c.port);
        info!("bind to {}", v6_addr);
        let v6 = UdpBuilder::new_v6()
            .unwrap()
            .only_v6(true)
            .unwrap()
            .bind(v6_addr)
            .unwrap();

        let v4_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), c.port);
        info!("bind to {}", v4_addr);
        let v4 = UdpBuilder::new_v4().unwrap().bind(v4_addr).unwrap();

        let v6 = UdpSocket::from_std(v6, &Handle::default()).unwrap();
        let v4 = UdpSocket::from_std(v4, &Handle::default()).unwrap();

        let buffer = LinkedList::new();
        Self {
            v6,
            v4,
            tx,
            rx,
            buffer,
        }
    }

    pub fn connect(&self, addr: &SocketAddr) -> Result<(), std::io::Error> {
        info!("connect {}", addr);
        match addr.ip() {
            IpAddr::V4(_) => self.v4.connect(addr),
            IpAddr::V6(_) => self.v6.connect(addr),
        }
    }
}

impl Future for Socket {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        loop {
            let mut buffer = Buffer::get();
            match self.v4.poll_recv_from(buffer.as_mut_slice())? {
                Async::Ready((read_size, addr)) => {
                    Buffer::set_len(buffer.as_mut(), read_size);
                    info!("receive {} bytes from {}", read_size, addr);
                    let message_to_router = Message::from_protobuf(addr, buffer);
                    self.tx.try_send(message_to_router).unwrap();
                }
                Async::NotReady => {
                    Buffer::put_back(buffer);
                    break;
                }
            };
        }

        loop {
            let mut buffer = Buffer::get();
            match self.v6.poll_recv_from(buffer.as_mut_slice())? {
                Async::Ready((read_size, addr)) => {
                    Buffer::set_len(buffer.as_mut(), read_size);
                    info!("receive {} bytes from {}", read_size, addr);
                    let message_to_router = Message::from_protobuf(addr, buffer);
                    self.tx.try_send(message_to_router).unwrap();
                }
                Async::NotReady => {
                    Buffer::put_back(buffer);
                    break;
                }
            };
        }

        loop {
            match self.rx.poll()? {
                Async::Ready(None) => panic!(),
                Async::Ready(Some(message)) => {
                    if let Message::AddNodeWrite(addr, _) = message {
                        self.connect(&addr).unwrap();
                    }

                    self.buffer.push_back(message.write_bytes());
                }
                Async::NotReady => break,
            }
        }

        loop {
            match self.buffer.pop_front() {
                None => break,

                Some((addr, buffer)) => {
                    let socket = match addr.ip() {
                        IpAddr::V4(_) => &mut self.v4,
                        IpAddr::V6(_) => &mut self.v6,
                    };
                    match socket.poll_send_to(&buffer, &addr)? {
                        Async::Ready(size) => info!("write {} bytes to {}", size, &addr),
                        Async::NotReady => {
                            self.buffer.push_back((addr, buffer));
                            break;
                        }
                    };
                }
            }
        }

        Ok(Async::NotReady)
    }
}
