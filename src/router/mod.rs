pub mod peer;
pub mod table;

pub use self::peer::{Host, Peer};
pub use self::table::{LikeRouter, Table};
use crate::internal::message::Message;
use crate::internal::package::Package;
use log::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::RwLock;

lazy_static! {
    static ref ROUTE_TABLE: Router = Router::new();
}

#[derive(Debug, Default)]
pub struct Router {
    ipv4_table: RwLock<Table>,
    ipv6_table: RwLock<Table>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            ipv6_table: Table::new().into(),
            ipv4_table: Table::new().into(),
        }
    }
    pub fn get() -> &'static Self {
        &ROUTE_TABLE
    }
}

impl Router {
    pub fn router_message(
        &self,
        m: (Option<SocketAddr>, Message),
    ) -> (Option<SocketAddr>, Message) {
        match m.1 {
            Message::PackageShareRead(package, ttl) => match self.find_in_table(&package) {
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
            },
            Message::AddNodeRead(node) => {
                if m.0.is_none() {
                    info!("can not add node for source none");
                    return (None, Message::DoNoting);
                }
                let source = m.0.unwrap();

                // 1. insert subnet
                if !node.sub_net_v4.is_empty() {
                    let v4 = node.sub_net_v4;
                    let v4 = Ipv4Addr::new(v4[0], v4[1], v4[2], v4[3]);
                    self.insert_to_table(
                        v4.into(),
                        node.net_mask_v4 as u16,
                        node.name.clone(),
                        Host::Socket(source),
                    );
                }
                if !node.sub_net_v6.is_empty() {
                    let v6 = node.sub_net_v6;
                    let v6 = Ipv6Addr::from([
                        v6[0], v6[1], v6[2], v6[3], v6[4], v6[5], v6[6], v6[7], v6[8], v6[9],
                        v6[10], v6[11], v6[12], v6[13], v6[14], v6[15],
                    ]);
                    self.insert_to_table(
                        v6.into(),
                        node.net_mask_v6 as u16,
                        node.name.clone(),
                        Host::Socket(source),
                    );
                }

                // 2. find out what we known but m.0 did not known
                if false {
                    (m.0, Message::AddNodeWrite(unreachable!()))
                } else {
                    (None, Message::DoNoting)
                }
            }
            Message::DelNodeRead(nodes) => {
                info!("del node {:?}", nodes);
                (None, Message::DoNoting)
            }
            Message::InterfaceRead(package) => {
                self.router_message((None, Message::PackageShareRead(package, 127)))
            }
            Message::DoNoting => (None, Message::DoNoting),
            Message::PingPongRead(_name) => (
                m.0,
                Message::PingPongWrite("name-from-ping-pong".to_string()),
            ),
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
