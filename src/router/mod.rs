pub mod peer;
pub mod table;

pub use self::peer::{Host, Peer};
pub use self::table::{LikeRouter, Table};
use crate::config::Config;
use crate::internal::message::Message;
use crate::internal::package::Package;
use crate::network::SELF_SHARE;
use log::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::RwLock;
use tokio::prelude::stream::Stream;
use tokio::prelude::{Async, Future};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct Router {
    ipv4_table: RwLock<Table>,
    ipv6_table: RwLock<Table>,
    tx: mpsc::UnboundedSender<Message>,
    rx: mpsc::UnboundedReceiver<Message>,
}

impl Router {
    pub fn new(tx: mpsc::UnboundedSender<Message>, rx: mpsc::UnboundedReceiver<Message>) -> Self {
        Router {
            tx,
            rx,
            ipv6_table: Table::new().into(),
            ipv4_table: Table::new().into(),
        }
    }
}

impl Future for Router {
    type Item = ();
    type Error = mpsc::error::UnboundedRecvError;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        loop {
            match self.rx.poll()? {
                Async::Ready(Some(m)) => {
                    self.router_message(m);
                }
                Async::Ready(None) => panic!(),
                Async::NotReady => break,
            };
        }
        Ok(Async::NotReady)
    }
}

impl Router {
    pub fn router_message(&mut self, m: Message) {
        match m {
            Message::PackageShareRead(package, ttl) => {
                trace!("router get PackageShareRead read");
                match self.find_in_table(&package) {
                    Some(peer) => match peer.get_host() {
                        Host::Socket(addr) => {
                            info!(
                                "{} -> {} route to real address {}",
                                package.source_address(),
                                package.destination_address(),
                                addr
                            );
                            self.tx
                                .try_send(Message::PackageShareWrite(addr, package, ttl))
                                .unwrap();
                        }
                        Host::Localhost => {
                            info!(
                                "{} -> {} route to Self",
                                package.source_address(),
                                package.destination_address()
                            );
                            self.tx.try_send(Message::InterfaceWrite(package)).unwrap();
                        }
                        Host::Unreachable => {
                            info!(
                                "{} -> {} can'find edge to reach, drop package",
                                package.source_address(),
                                package.destination_address()
                            );
                            self.tx.try_send(Message::DoNoting).unwrap();
                        }
                    },
                    None => {
                        info!(
                            "{} -> {} not find in router table, drop package",
                            package.source_address(),
                            package.destination_address()
                        );
                        self.tx.try_send(Message::DoNoting).unwrap();
                    }
                }
            }
            Message::AddNodeRead(addr, mut node) => {
                if node.name == Config::get().name {
                    info!("get myself");
                    return;
                }

                let source = {
                    if node.jump == 0 {
                        addr
                    } else {
                        let ip = read_ip(&node.real_ip);
                        SocketAddr::new(ip, node.port as u16)
                    }
                };

                let v = read_ip(&node.sub_net);
                if let Ok(()) = self.insert_to_table(
                    v,
                    node.net_mask as u16,
                    node.name.clone(),
                    Host::Socket(source),
                ) {
                    let jump = node.get_jump() + 1;
                    node.set_jump(jump);
                    node.set_real_ip(parse_ip(source.ip()));
                    node.set_port(source.port() as i32);
                    info!("broadcast to all {:?}", node);
                    for node_addr in self.get_all_node() {
                        self.tx
                            .try_send(Message::AddNodeWrite(node_addr, node.clone()))
                            .unwrap();
                    }
                    self.tx
                        .try_send(Message::AddNodeWrite(addr, SELF_SHARE.clone()))
                        .unwrap();
                }
            }
            Message::DelNodeRead(addr, _) => {
                trace!("router get DelNode read from {}", addr);
                self.tx.try_send(Message::DoNoting).unwrap();
            }
            Message::InterfaceRead(package) => {
                trace!("router get interface read");
                self.router_message(Message::PackageShareRead(package, 127));
            }
            Message::DoNoting => {
                self.tx.try_send(Message::DoNoting).unwrap();
            }
            Message::PingPongRead(addr, _name) => {
                info!("get PingPongRead from {}", addr);
                self.tx
                    .try_send(Message::PingPongWrite(addr, Config::get().name.clone()))
                    .unwrap();
            }
            Message::InterfaceWrite(_) => panic!("InterfaceWrite can not route"),
            Message::PingPongWrite(_, _) => panic!("PingPongWrite can not route"),
            Message::AddNodeWrite(_, _) => panic!("AddNodeWrite can not route"),
            Message::PackageShareWrite(_, _, _) => panic!("PackageShareWrite can not route"),
            Message::DelNodeWrite(_, _) => panic!("DelNodeWrite can not route"),
        }
    }
}

impl Router {
    pub fn get_all_node(&self) -> Vec<SocketAddr> {
        let v = self.ipv4_table.read().unwrap().get_all_peer();
        let o = &mut self.ipv6_table.read().unwrap().get_all_peer();

        v.iter()
            .chain(o.iter())
            .map(|p| match p.get_host() {
                Host::Socket(addr) => Some(addr),
                Host::Unreachable | Host::Localhost => None,
            })
            .filter(|x| !x.is_none())
            .map(|x| x.unwrap())
            .collect()
    }

    pub fn insert_to_table(
        &self,
        dest: IpAddr,
        mask: u16,
        name: String,
        host: Host,
    ) -> Result<(), ()> {
        info!(
            "add {}/{} -> {}:\"{:?}\" to router table",
            dest, mask, name, host
        );

        match dest {
            IpAddr::V4(v4_addr) => {
                self.ipv4_table
                    .write()
                    .unwrap()
                    .insert(v4_addr.into(), mask, name, host)
            }
            IpAddr::V6(v6_addr) => {
                self.ipv6_table
                    .write()
                    .unwrap()
                    .insert(v6_addr.into(), mask, name, host)
            }
        }
    }

    pub fn find_in_table(&self, package: &Package) -> Option<Peer> {
        let dest = package.destination_address();
        match dest {
            IpAddr::V4(v4) => match self.ipv4_table.read().unwrap().find(v4.into()) {
                Some(t) => Some(t.clone()),
                None => None,
            },
            IpAddr::V6(v6) => match self.ipv6_table.read().unwrap().find(v6.into()) {
                Some(t) => Some(t.clone()),
                None => None,
            },
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<Peer> {
        if let Some(peer) = self.ipv4_table.read().unwrap().get_by_peer_name(name) {
            return Some(peer);
        }
        if let Some(peer) = self.ipv6_table.read().unwrap().get_by_peer_name(name) {
            return Some(peer);
        }
        None
    }
}

fn read_ip(v: &[u8]) -> IpAddr {
    match v.len() {
        4 => Ipv4Addr::from([v[0], v[1], v[2], v[3]]).into(),
        16 => Ipv6Addr::from([
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13],
            v[14], v[15],
        ])
        .into(),
        _ => {
            error!("try read with length {}", v.len());
            unreachable!()
        }
    }
}

fn parse_ip(v: IpAddr) -> Vec<u8> {
    match v {
        IpAddr::V4(v4) => v4.octets().to_vec(),
        IpAddr::V6(v6) => v6.octets().to_vec(),
    }
}
