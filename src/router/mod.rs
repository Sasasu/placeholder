pub mod peer;
pub mod table;
#[cfg(test)]
pub mod test;

pub use self::peer::{Host, Peer};
pub use self::table::{LikeRouter, Table};
use crate::config::Config;
use crate::generated::transport::Node;
use crate::internal::message::Message;
use crate::internal::package::Package;
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
    tx: mpsc::UnboundedSender<(Option<SocketAddr>, Message)>,
    rx: mpsc::UnboundedReceiver<(Option<SocketAddr>, Message)>,
}

impl Router {
    pub fn new(
        tx: mpsc::UnboundedSender<(Option<SocketAddr>, Message)>,
        rx: mpsc::UnboundedReceiver<(Option<SocketAddr>, Message)>,
    ) -> Self {
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
                    let (addr, message) = self.router_message(m);
                    self.tx.try_send((addr, message)).unwrap();
                }
                Async::Ready(None) => panic!(),
                Async::NotReady => break,
            };
        }
        Ok(Async::NotReady)
    }
}

impl Router {
    pub fn router_message(
        &self,
        m: (Option<SocketAddr>, Message),
    ) -> (Option<SocketAddr>, Message) {
        match m.1 {
            Message::PackageShareRead(package, ttl) => {
                trace!("router get PackageShareRead read");
                match self.find_in_table(&package) {
                    Some(peer) => match peer.get_host() {
                        Some(Host::Socket(addr)) => {
                            info!(
                                "{} -> {} route to real address {}",
                                package.source_address(),
                                package.destination_address(),
                                addr
                            );
                            (Some(*addr), Message::PackageShareWrite(package, ttl))
                        }
                        Some(Host::Localhost) => {
                            info!(
                                "{} -> {} route to Self",
                                package.source_address(),
                                package.destination_address()
                            );
                            (None, Message::InterfaceWrite(package))
                        }
                        None | Some(Host::Unreachable) => {
                            info!("{} -> {} find node in router but can'find edge to reach, drop package", package.source_address(), package.destination_address());
                            (None, Message::DoNoting)
                        }
                    },
                    None => {
                        info!(
                            "{} -> {} not find in router table, drop package",
                            package.source_address(),
                            package.destination_address()
                        );
                        (None, Message::DoNoting)
                    }
                }
            }
            Message::InitNode(init) => {
                trace!("router get InitNode");
                let mut add_node = Node::new();
                add_node.set_jump(1);
                add_node.set_name(init.name);
                add_node.set_sub_net(init.sub_net);
                add_node.set_net_mask(init.net_mask);

                let addr = m.0.unwrap();
                add_node.set_port(addr.port().into());
                match addr.ip() {
                    IpAddr::V6(v6) => add_node.set_real_ip(v6.octets().to_vec()),
                    IpAddr::V4(v4) => add_node.set_real_ip(v4.octets().to_vec()),
                }
                // use None to broadcast
                (None, Message::AddNodeWrite(add_node))
            }
            Message::AddNodeRead(mut node) => {
                trace!("router get AddNode read");
                if m.0.is_none() {
                    info!("can not add node for source none");
                    return (None, Message::DoNoting);
                }

                if node.name == Config::get().name {
                    info!("receive myself");
                    return (None, Message::DoNoting);
                }

                let source = {
                    if node.jump == 0 {
                        m.0.unwrap()
                    } else {
                        let ip = read_ip(&node.real_ip);
                        SocketAddr::new(ip, node.port as u16)
                    }
                };

                // 1. insert subnet
                if !node.sub_net.is_empty() {
                    let v = read_ip(&node.sub_net);
                    self.insert_to_table(
                        v,
                        node.net_mask as u16,
                        node.name.clone(),
                        Host::Socket(source),
                    );
                }

                // board cast every node
                let jump = node.get_jump() + 1;
                node.set_jump(jump);
                (Some(m.0.unwrap()), Message::AddNodeWrite(node.clone()))
            }
            Message::DelNodeRead(_) => {
                trace!("router get DelNode read");
                (None, Message::DoNoting)
            }
            Message::InterfaceRead(package) => {
                trace!("router get interface read");
                self.router_message((None, Message::PackageShareRead(package, 127)))
            }
            Message::DoNoting => (None, Message::DoNoting),
            Message::PingPongRead(_name) => {
                (m.0, Message::PingPongWrite(Config::get().name.clone()))
            }
            Message::InterfaceWrite(_)
            | Message::PingPongWrite(_)
            | Message::AddNodeWrite(_)
            | Message::PackageShareWrite(_, _)
            | Message::DelNodeWrite(_) => {
                info!("wrong message type send to router");
                (None, Message::DoNoting)
            }
        }
    }
}

impl Router {
    pub fn get_all_node(&self) -> Vec<SocketAddr> {
        info!("dump all node");
        let v = self.ipv4_table.read().unwrap().get_all_peer();
        let o = &mut self.ipv6_table.read().unwrap().get_all_peer();

        v.iter()
            .chain(o.iter())
            .map(|p| match p.get_host() {
                Some(Host::Socket(addr)) => Some(addr),
                None | Some(Host::Unreachable) | Some(Host::Localhost) => None,
            })
            .filter(|x| !x.is_none())
            .map(|x| *x.unwrap())
            .collect()
    }

    pub fn insert_to_table(&self, dest: IpAddr, mask: u16, name: String, host: Host) {
        info!(
            "add {}/{} -> {}:\"{:?}\" to router table",
            dest, mask, name, host
        );
        let peer = Peer {
            name,
            host: vec![host],
        };

        match dest {
            IpAddr::V4(v4_addr) => {
                self.ipv4_table
                    .write()
                    .unwrap()
                    .insert(v4_addr.into(), mask, peer)
            }
            IpAddr::V6(v6_addr) => {
                self.ipv6_table
                    .write()
                    .unwrap()
                    .insert(v6_addr.into(), mask, peer)
            }
        }
        .unwrap()
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
    info!("{}", v.len());
    match v.len() {
        4 => Ipv4Addr::from([v[0], v[1], v[2], v[3]]).into(),
        16 => Ipv6Addr::from([
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13],
            v[14], v[15],
        ])
        .into(),
        _ => {
            error!("{}", v.len());
            unreachable!()
        }
    }
}
