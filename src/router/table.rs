use log::*;
use radix_trie::Trie;
use radix_trie::TrieCommon;
use std::net::{IpAddr, SocketAddr};

use super::peer::Host;
use super::peer::Peer;

#[derive(Debug)]
pub struct Table {
    table: Trie<Vec<u8>, Peer>,
}

pub trait LikeRouter {
    fn find(&self, addr: IpAddr) -> Option<&Peer>;
    fn insert(
        &mut self,
        addr: IpAddr,
        mask: u16,
        peer_name: String,
        peer_host: Host,
    ) -> Result<(), ()>;
    fn delete(&mut self, addr: IpAddr, mask: u16) -> Result<(), ()>;
}

pub enum Error {
    NotFind,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            table: Trie::default(),
        }
    }
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Table {
    pub fn get_all_nodes(&self) -> Vec<SocketAddr> {
        let mut v = vec![];
        for (_, node) in self.table.iter() {
            match node.get_host() {
                Host::Localhost | Host::Unreachable => {}
                Host::Socket(addr) => v.push(addr),
            }
        }
        v
    }

    pub fn get_by_peer_name(&self, name: &str) -> Option<Peer> {
        for (_, node) in self.table.iter() {
            if node.name == name {
                return Some(node.clone());
            }
        }
        None
    }

    pub fn get_all_peer(&self) -> Vec<Peer> {
        let mut v = vec![];
        for (_, node) in self.table.iter() {
            info!("dump {:?}", node);
            v.push(node.clone());
        }
        v
    }
}

impl LikeRouter for Table {
    fn find(&self, addr: IpAddr) -> Option<&Peer> {
        let addr = encode_bytes(addr);
        self.table.get_ancestor_value(&addr)
    }

    fn insert(
        &mut self,
        addr: IpAddr,
        mask: u16,
        peer_name: String,
        peer_host: Host,
    ) -> Result<(), ()> {
        let mut addr = encode_bytes(addr);
        unsafe { addr.set_len(mask.into()) };

        match self.table.get_mut(&addr) {
            None => {
                let mut peer = Peer::new(peer_name);
                peer.add_host(peer_host).unwrap();
                self.table.insert(addr, peer);
                Ok(())
            }
            Some(p) => p.add_host(peer_host),
        }
    }

    fn delete(&mut self, addr: IpAddr, mask: u16) -> Result<(), ()> {
        let mut addr = encode_bytes(addr);
        unsafe { addr.set_len(mask.into()) };
        match self.table.remove(&addr) {
            None => Err(()),
            Some(_) => Ok(()),
        }
    }
}

fn encode_bytes(ip: IpAddr) -> Vec<u8> {
    match ip {
        IpAddr::V4(ip) => {
            let mut v = Vec::with_capacity(4 * 8);
            for b in ip.octets().iter() {
                split_u8(*b, &mut v);
            }
            v
        }
        IpAddr::V6(ip) => {
            let mut v = Vec::with_capacity(16 * 8);
            for b in ip.octets().iter() {
                split_u8(*b, &mut v);
            }
            v
        }
    }
}

fn split_u8(u: u8, v: &mut Vec<u8>) {
    v.push((u & 0b1000_0000) >> 7);
    v.push((u & 0b0100_0000) >> 6);
    v.push((u & 0b0010_0000) >> 5);
    v.push((u & 0b0001_0000) >> 4);
    v.push((u & 0b0000_1000) >> 3);
    v.push((u & 0b0000_0100) >> 2);
    v.push((u & 0b0000_0010) >> 1);
    v.push(u & 0b0000_0001);
}

#[cfg(test)]
mod test {
    use crate::router::peer::Host;
    use crate::router::table::{LikeRouter, Table};

    #[test]
    pub fn crate_table() {
        let mut table = Table::new();
        table
            .insert(
                "128.66.1.0".parse().unwrap(),
                24,
                "test".to_string(),
                Host::Localhost,
            )
            .unwrap();
    }

    #[test]
    pub fn insert_table() {
        let mut table = Table::new();
        table
            .insert(
                "128.66.1.0".parse().unwrap(),
                24,
                "test1".to_string(),
                Host::Localhost,
            )
            .unwrap();
        table
            .insert(
                "128.66.2.0".parse().unwrap(),
                24,
                "test2".to_string(),
                Host::Localhost,
            )
            .unwrap();

        let ans1 = table.find("128.66.1.0".parse().unwrap()).unwrap();
        assert_eq!(ans1.name, "test1");
        let ans2 = table.find("128.66.2.0".parse().unwrap()).unwrap();
        assert_eq!(ans2.name, "test2");
    }

    #[test]
    pub fn get_from_table() {
        let mut table = Table::new();
        table
            .insert(
                "128.66.1.0".parse().unwrap(),
                24,
                "test1".to_string(),
                Host::Localhost,
            )
            .unwrap();
        table
            .insert(
                "128.66.2.0".parse().unwrap(),
                24,
                "test2".to_string(),
                Host::Localhost,
            )
            .unwrap();

        let ans1 = table.find("128.66.1.1".parse().unwrap()).unwrap();
        assert_eq!(ans1.name, "test1");

        let ans1 = table.find("128.66.1.2".parse().unwrap()).unwrap();
        assert_eq!(ans1.name, "test1");

        let ans2 = table.find("128.66.2.1".parse().unwrap()).unwrap();
        assert_eq!(ans2.name, "test2");
        let ans2 = table.find("128.66.2.255".parse().unwrap()).unwrap();
        assert_eq!(ans2.name, "test2");

        let ans3 = table.find("128.66.3.0".parse().unwrap());
        assert!(ans3.is_none());
    }

    #[test]
    pub fn delete_from_table() {
        let mut table = Table::new();

        table
            .insert(
                "128.66.1.0".parse().unwrap(),
                24,
                "test1".to_string(),
                Host::Localhost,
            )
            .unwrap();

        let ans1 = table.find("128.66.1.1".parse().unwrap()).unwrap().clone();
        assert_eq!(ans1.name, "test1");

        let ans2 = table.find("128.66.2.1".parse().unwrap());
        assert!(ans2.is_none());

        table.delete("128.66.1.0".parse().unwrap(), 24).unwrap();
        assert!(table.find("128.66.1.1".parse().unwrap()).is_none());
        assert!(table.find("128.66.1.0".parse().unwrap()).is_none());
    }

    #[test]
    pub fn get_by_name() {
        let mut table = Table::new();
        table
            .insert(
                "128.66.1.0".parse().unwrap(),
                24,
                "test1".to_string(),
                Host::Localhost,
            )
            .unwrap();
        table
            .insert(
                "128.66.2.0".parse().unwrap(),
                24,
                "test2".to_string(),
                Host::Localhost,
            )
            .unwrap();

        assert_eq!(table.get_by_peer_name("test1").unwrap().name, "test1");
        assert_eq!(table.get_by_peer_name("test2").unwrap().name, "test2");
        assert!(table.get_by_peer_name("test3").is_none());
    }

    #[test]
    pub fn get_all() {
        let mut table = Table::new();
        table
            .insert(
                "128.66.1.0".parse().unwrap(),
                24,
                "test1".to_string(),
                Host::Localhost,
            )
            .unwrap();
        table
            .insert(
                "128.66.2.0".parse().unwrap(),
                24,
                "test2".to_string(),
                Host::Localhost,
            )
            .unwrap();
        assert_eq!(2, table.get_all_peer().len());
    }
}
