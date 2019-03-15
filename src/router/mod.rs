use crate::generated::transport as proto;
use crate::internal::message::Message;
use crate::internal::package::Package;
use log::*;
use radix_trie::Trie;
use radix_trie::TrieKey;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::RwLock;

pub mod peer;
pub mod table;

lazy_static! {
    static ref ROUTE_TABLE: Router = Router::new();
}

#[derive(Debug, Clone)]
pub enum Peer {
    Addr(SocketAddr),
    LocaleHost,
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct V4Addr(Ipv4Addr);

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct V6Addr(Ipv6Addr);

impl TrieKey for V4Addr {
    fn encode_bytes(&self) -> Vec<u8> {
        let bits = self.0.octets();
        vec![bits[0], bits[1], bits[2], bits[3]]
    }
}

impl From<Ipv4Addr> for V4Addr {
    fn from(a: Ipv4Addr) -> Self {
        V4Addr { 0: a }
    }
}

impl TrieKey for V6Addr {
    fn encode_bytes(&self) -> Vec<u8> {
        let bits = self.0.octets();
        vec![
            bits[0], bits[1], bits[2], bits[3], bits[4], bits[5], bits[6], bits[7], bits[8],
            bits[9], bits[10], bits[11], bits[12], bits[13], bits[14], bits[15],
        ]
    }
}

impl From<Ipv6Addr> for V6Addr {
    fn from(a: Ipv6Addr) -> Self {
        V6Addr { 0: a }
    }
}

#[derive(Debug, Default)]
pub struct Router {
    ipv4_table: RwLock<Trie<V4Addr, String>>,
    ipv6_table: RwLock<Trie<V6Addr, String>>,
    peer_list: RwLock<HashMap<String, Vec<Peer>>>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            ipv6_table: Trie::new().into(),
            ipv4_table: Trie::new().into(),
            peer_list: HashMap::new().into(),
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
                Ok(Some(peer)) => match peer {
                    Peer::Addr(addr) => {
                        info!(
                            "{} -> {} route to real address {}",
                            package.source_address(),
                            package.destination_address(),
                            addr
                        );
                        (Some(addr), Message::PackageShareWrite(package, ttl))
                    }
                    Peer::LocaleHost => {
                        info!(
                            "{} -> {} route to Self",
                            package.source_address(),
                            package.destination_address()
                        );
                        (None, Message::InterfaceWrite(package))
                    }
                },
                Ok(None) => {
                    info!(
                        "cannot find {} in route table, drop package",
                        package.destination_address()
                    );
                    (None, Message::DoNoting)
                }
                Err(name) => {
                    info!(
                        "find {} in route table, but not known where to go, asking now",
                        &name
                    );
                    let random_node = self.peer_list.read().unwrap();
                    for (k, v) in random_node.iter() {
                        for i in v {
                            match i {
                                Peer::LocaleHost => continue,
                                Peer::Addr(addr) => {
                                    info!("ask {} for {}", &k, &name);
                                    return (
                                        Some(*addr),
                                        Message::WhoHasNodeWrite(vec![k.clone()]),
                                    );
                                }
                            }
                        }
                    }
                    info!("no one have {}, drop package", &name);
                    (None, Message::DoNoting)
                }
            },
            Message::ShareNodeRead(nodes) => {
                for node in nodes {
                    let node_addr = SocketAddr::new(node.host.parse().unwrap(), node.port as u16);
                    info!("add {}:{} to router table", node.name, node_addr);
                    self.peer_list
                        .write()
                        .unwrap()
                        .entry(node.name)
                        .or_insert_with(Vec::new)
                        .push(Peer::Addr(node_addr));
                }
                (None, Message::DoNoting)
            }
            Message::AddNodeRead(nodes) => {
                let mut find = vec![];
                for node in nodes {
                    // 1. insert peer
                    self.add_peer(&node.name, Peer::Addr(m.0.expect("node addr is None")));

                    // 2. insert subnet
                    if !node.sub_net_v4.is_empty() {
                        let v4 = node.sub_net_v4;
                        let v4 = Ipv4Addr::new(v4[0], v4[1], v4[2], v4[3]);
                        self.insert_to_table(v4.into(), node.net_mask_v4, node.name.clone());
                    }
                    if !node.sub_net_v6.is_empty() {
                        let v6 = node.sub_net_v6;
                        let v6 = Ipv6Addr::from([
                            v6[0], v6[1], v6[2], v6[3], v6[4], v6[5], v6[6], v6[7], v6[8], v6[9],
                            v6[10], v6[11], v6[12], v6[13], v6[14], v6[15],
                        ]);
                        self.insert_to_table(v6.into(), node.net_mask_v6, node.name.clone());
                    }

                    // 3. make a diff.
                    find.push(node.name.clone());
                }
                if find.len() == self.peer_list.read().unwrap().len() {
                    (None, Message::DoNoting)
                } else {
                    (m.0, Message::AddNodeWrite(unreachable!()))
                }
            }
            Message::WhoHasNodeRead(names) => {
                let mut node_address = vec![];
                for name in names {
                    let peer = Router::get().peer_list.read().unwrap().get(&name).cloned();
                    if peer.is_none() {
                        return (None, Message::DoNoting);
                    }
                    for p in peer.unwrap() {
                        match p {
                            Peer::LocaleHost => continue,
                            Peer::Addr(addr) => {
                                let mut address = proto::NodeAddress::new();
                                address.set_name(name.clone());
                                address.set_host(addr.ip().to_string());
                                address.set_port(addr.port().into());
                                node_address.push(address);
                            }
                        }
                    }
                }
                (m.0, Message::ShareNodeWrite(node_address))
            }
            Message::DelNodeRead(nodes) => {
                info!("del node {:?}", nodes);
                (None, Message::DoNoting)
            }
            Message::InterfaceRead(package) => {
                self.router_message((None, Message::PackageShareRead(package, 127)))
            }
            Message::DoNoting => (None, Message::DoNoting),
            Message::PingPongRead(_name) => {
                return (
                    m.0,
                    Message::PingPongWrite("name-from-ping-pong".to_string()),
                );
            }
            Message::InterfaceWrite(_)
            | Message::PingPongWrite(_)
            | Message::AddNodeWrite(_)
            | Message::PackageShareWrite(_, _)
            | Message::DelNodeWrite(_)
            | Message::ShareNodeWrite(_)
            | Message::WhoHasNodeWrite(_) => {
                info!("wrong message type send to router");
                (None, Message::DoNoting)
            }
        }
    }
}

impl Router {
    pub fn get_all_peer_for_send(&self) -> Vec<proto::AddNode> {
        let v = vec![];
        for (name, peer) in self.peer_list.read().unwrap().iter() {
            let mut node = proto::AddNode::new();
            node.set_name(name.clone());
            // TODO
            node.set_jump(1);
        }
        v
    }
    pub fn add_peer(&self, name: &str, peer: Peer) {
        info!("add {} -> {:?} to peer list", name, peer);
        self.peer_list
            .write()
            .unwrap()
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(peer);
    }
    pub fn insert_to_table(&self, dest: IpAddr, _net_mask: u32, name: String) {
        info!("add {} -> {:?} to router table", dest, name);
        self.peer_list
            .write()
            .unwrap()
            .entry(name.clone())
            .or_insert_with(Vec::new);
        match dest {
            IpAddr::V4(v4_addr) => self
                .ipv4_table
                .write()
                .unwrap()
                .insert(v4_addr.into(), name),
            IpAddr::V6(v6_addr) => self
                .ipv6_table
                .write()
                .unwrap()
                .insert(v6_addr.into(), name),
        };
    }

    pub fn find_in_table(&self, package: &Package) -> Result<Option<Peer>, String> {
        let dest = package.destination_address();
        match dest {
            IpAddr::V4(v4) => {
                if let Some(t) = self.ipv4_table.read().unwrap().get(&V4Addr { 0: v4 }) {
                    match self.get_by_name(&t) {
                        Some(peer) => return Ok(Some(peer)),
                        None => return Err(t.clone()),
                    }
                } else {
                    return Ok(None);
                }
            }
            IpAddr::V6(v6) => {
                if let Some(t) = self.ipv6_table.read().unwrap().get(&V6Addr { 0: v6 }) {
                    match self.get_by_name(&t) {
                        Some(peer) => return Ok(Some(peer)),
                        None => return Err(t.clone()),
                    }
                } else {
                    return Ok(None);
                }
            }
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<Peer> {
        match self.peer_list.read().unwrap().get(name) {
            None => None,
            Some(v) => {
                if v.is_empty() {
                    None
                } else {
                    Some(v[0].clone())
                }
            }
        }
    }
}
