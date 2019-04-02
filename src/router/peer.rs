use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::net::SocketAddr;

#[derive(Eq, Debug, Clone, PartialEq)]
pub enum Host {
    Localhost,
    Unreachable,
    Socket(SocketAddr),
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub name: String,
    host: BinaryHeap<PeerInternal>,
}

impl Peer {
    pub fn new(name: String) -> Self {
        Peer {
            name,
            host: BinaryHeap::new(),
        }
    }

    pub fn get_host(&self) -> Host {
        match self.host.peek() {
            None => Host::Unreachable,
            Some(t) => match t {
                PeerInternal::Localhost => Host::Localhost,
                PeerInternal::Unreachable => Host::Unreachable,
                PeerInternal::Socket(addr, _) => Host::Socket(addr.clone()),
            },
        }
    }

    pub fn add_host(&mut self, host: Host) -> Result<(), ()> {
        match host {
            Host::Localhost => {
                for i in self.host.iter() {
                    if let PeerInternal::Localhost = i {
                        return Err(());
                    }
                }
                self.host.push(PeerInternal::Localhost)
            }
            Host::Unreachable => {
                for i in self.host.iter() {
                    if let PeerInternal::Unreachable = i {
                        return Err(());
                    }
                }
                self.host.push(PeerInternal::Unreachable)
            }
            Host::Socket(addr) => {
                for i in self.host.iter() {
                    if let PeerInternal::Socket(a, mut rank) = i {
                        if a == &addr {
                            use std::ops::AddAssign;
                            rank.add_assign(1);
                            return Err(());
                        }
                    }
                }
                self.host.push(PeerInternal::Socket(addr, 1))
            }
        }
        Ok(())
    }
}

#[derive(Eq, Debug, Clone)]
enum PeerInternal {
    Localhost,
    Unreachable,
    Socket(SocketAddr, u32),
}

impl PartialEq for PeerInternal {
    fn eq(&self, other: &Self) -> bool {
        if let (PeerInternal::Localhost, PeerInternal::Localhost) = (other, self) {
            return true;
        } else if let (PeerInternal::Unreachable, PeerInternal::Unreachable) = (other, self) {
            return true;
        } else if let (PeerInternal::Socket(s1, r1), PeerInternal::Socket(s2, r2)) = (self, other) {
            return s1 == s2 && r1 == r2;
        }
        false
    }
}

impl PartialOrd for PeerInternal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.cmp(other).into()
    }
}

impl Ord for PeerInternal {
    fn cmp(&self, other: &Self) -> Ordering {
        if let (PeerInternal::Localhost, PeerInternal::Localhost) = (self, other) {
            return Ordering::Equal;
        } else if let (PeerInternal::Unreachable, PeerInternal::Unreachable) = (self, other) {
            return Ordering::Equal;
        } else if let PeerInternal::Localhost = self {
            return Ordering::Greater;
        } else if let PeerInternal::Localhost = other {
            return Ordering::Less;
        } else if let PeerInternal::Unreachable = self {
            return Ordering::Less;
        } else if let PeerInternal::Unreachable = other {
            return Ordering::Greater;
        } else if let (PeerInternal::Socket(_, r1), PeerInternal::Socket(_, r2)) = (other, self) {
            if r1 > r2 {
                return Ordering::Greater;
            } else if r1 == r2 {
                return Ordering::Equal;
            } else {
                return Ordering::Less;
            }
        }
        unreachable!("no more ord")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn it_works() {
        let mut p = Peer::new("test".to_string());
        assert_eq!(p.add_host(Host::Localhost).is_ok(), true);
        assert_eq!(p.add_host(Host::Localhost).is_err(), true);

        assert_eq!(
            p.add_host(Host::Socket("128.66.1.0:1234".parse().unwrap()))
                .is_ok(),
            true
        );
        assert_eq!(p.get_host(), Host::Localhost);
    }

    #[test]
    pub fn add_two_host() {
        let mut p = Peer::new("test".to_string());
        assert_eq!(p.add_host(Host::Localhost).is_ok(), true);
        assert_eq!(p.add_host(Host::Unreachable).is_ok(), true);

        assert_eq!(p.get_host(), Host::Localhost);
    }

    #[test]
    pub fn test_rank() {
        let mut p = Peer::new("test".to_string());
        assert_eq!(
            p.add_host(Host::Socket("128.66.1.0:1234".parse().unwrap()))
                .is_ok(),
            true
        );
        assert_eq!(
            p.add_host(Host::Socket("128.66.1.0:1234".parse().unwrap()))
                .is_err(),
            true
        );
        assert_eq!(
            p.add_host(Host::Socket("128.66.1.1:1234".parse().unwrap()))
                .is_ok(),
            true
        );

        assert_eq!(
            p.get_host(),
            Host::Socket("128.66.1.0:1234".parse().unwrap())
        );
    }
}
