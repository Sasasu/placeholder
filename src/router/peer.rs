use std::net::SocketAddr;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Host {
    Localhost,
    Socket(SocketAddr),
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Peer {
    pub name: String,
    pub host: Vec<Host>,
}

impl Peer {
    pub fn merge(&mut self, other: &Peer) {}
    pub fn get_host(&self) -> Option<&Host> {
        if self.host.is_empty() {
            None
        } else {
            Some(&self.host[0])
        }
    }
}
