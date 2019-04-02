use super::package::Package;
use crate::generated::transport::{Node, PackageShard, Payload, PingPong};
use log::*;
use std::net::SocketAddr;

// the message bus
#[derive(Debug)]
pub enum Message {
    InterfaceRead(Package),
    InterfaceWrite(Package),

    PackageShareRead(Package, u32),
    PackageShareWrite(SocketAddr, Package, u32),

    AddNodeRead(SocketAddr, Node),
    AddNodeWrite(SocketAddr, Node),

    DelNodeRead(SocketAddr, Node),
    DelNodeWrite(SocketAddr, Node),

    PingPongRead(SocketAddr, String),
    PingPongWrite(SocketAddr, String),

    DoNoting,
}

impl Message {
    /// message to protobuf message
    ///
    /// only support network and router
    pub fn write_bytes(self) -> (SocketAddr, Vec<u8>) {
        let mut payload = Payload::new();
        let addr;
        match self {
            Message::PackageShareWrite(a, package, ttl) => {
                let mut package_shard = PackageShard::new();
                package_shard.set_package(package.raw_package);
                package_shard.set_ttl(ttl);
                payload.set_package(package_shard);
                addr = a;
            }
            Message::AddNodeWrite(a, node) => {
                payload.set_add_node(node);
                addr = a;
            }
            Message::DelNodeWrite(a, node) => {
                payload.set_del_node(node);
                addr = a;
            }
            Message::PingPongWrite(a, name) => {
                let mut wrapper = PingPong::new();
                wrapper.set_name(name);
                payload.set_ping(wrapper);
                addr = a;
            }
            Message::InterfaceRead(_) => panic!("can not write InterfaceRead to bytes"),
            Message::InterfaceWrite(_) => panic!("can not write InterfaceWrite to bytes"),
            Message::DoNoting => panic!("can not write DoNoting to bytes"),
            Message::PingPongRead(_, _) => panic!("can not write PingPingRead to bytes"),
            Message::AddNodeRead(_, _) => panic!("can not write AddNodeRead to bytes"),
            Message::PackageShareRead(_, _) => panic!("can not write PackageShareRead to bytes"),
            Message::DelNodeRead(_, _) => panic!("can not write DelNodeRead to bytes"),
        };
        let bytes = (Box::new(payload) as Box<protobuf::Message>)
            .write_to_bytes()
            .unwrap();
        (addr, bytes)
    }

    /// protobuf message to message
    pub fn from_protobuf(addr: SocketAddr, buffer: Vec<u8>) -> Self {
        use crate::generated::transport::Payload_oneof_payload as PayloadOneof;
        use protobuf::Message as ProtoMessage;

        let mut payload = Payload::new();
        if let Err(e) = payload.merge_from_bytes(&buffer) {
            warn!("error to decode protobuf, drop package {}", e);
            return Message::DoNoting;
        }
        match payload.payload {
            None => {
                warn!("no payload, drop package");
                (Message::DoNoting)
            }
            Some(PayloadOneof::ping(value)) => Message::PingPongRead(addr, value.name),
            Some(PayloadOneof::package(package)) => {
                let p = Package::from_buffer(package.package);
                Message::PackageShareRead(p, package.ttl)
            }
            Some(PayloadOneof::add_node(node)) => Message::AddNodeRead(addr, node),
            Some(PayloadOneof::del_node(node)) => Message::DelNodeRead(addr, node),
        }
    }
}
