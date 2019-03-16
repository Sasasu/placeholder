use super::package::Package;
use crate::generated::transport as proto;
use log::*;
use std::net::SocketAddr;

// the message bus
#[derive(Debug)]
pub enum Message {
    InterfaceRead(Package),
    InterfaceWrite(Package),

    PackageShareRead(Package, u32),
    PackageShareWrite(Package, u32),

    AddNodeRead(Vec<proto::AddNode>),
    AddNodeWrite(Vec<proto::AddNode>),

    DelNodeRead(Vec<String>),
    DelNodeWrite(Vec<String>),

    PingPongRead(String),
    PingPongWrite(String),

    DoNoting,
}

impl Message {
    /// message to protobuf message
    ///
    /// only support network and router
    pub fn into_protobuf(self) -> impl protobuf::Message {
        let mut payload = proto::Payload::new();
        match self {
            Message::PackageShareWrite(package, ttl) => {
                let mut package_shard = proto::PackageShard::new();
                package_shard.set_package(package.raw_package);
                package_shard.set_ttl(ttl);
                payload.set_package(package_shard);
            }
            Message::AddNodeWrite(nodes) => {
                let mut wrapper = proto::AddNodeRequest::new();
                wrapper.set_nodes(nodes.into());
                payload.set_add_node(wrapper);
            }
            Message::DelNodeWrite(nodes) => {
                let mut wrapper = proto::DelNodeRequest::new();
                wrapper.set_nodes(nodes.into());
                payload.set_del_node(wrapper);
            }
            Message::PingPongWrite(name) => {
                let mut wrapper = proto::PingPong::new();
                wrapper.set_name(name);
                payload.set_ping(wrapper);
            }
            Message::InterfaceRead(_) | Message::InterfaceWrite(_) | Message::DoNoting => {
                unreachable!("can not covert to protobuf message, {:?}", self)
            }
            Message::PingPongRead(_)
            | Message::AddNodeRead(_)
            | Message::PackageShareRead(_, _)
            | Message::DelNodeRead(_) => unreachable!(
                "can not covert response message to protobuf message, {:?}",
                self
            ),
        };
        payload
    }

    /// protobuf message to message
    pub fn from_protobuf(
        source: Option<SocketAddr>,
        buffer: Vec<u8>,
    ) -> (Option<SocketAddr>, Self) {
        use crate::generated::transport::Payload_oneof_payload as PayloadOneof;
        use protobuf::Message as ProtoMessage;
        let mut payload = proto::Payload::new();
        if let Err(e) = payload.merge_from_bytes(&buffer) {
            warn!("error to decode protobuf, drop package {}", e);
            return (source, Message::DoNoting);
        }
        match payload.payload {
            None => {
                warn!("no payload, drop package");
                (source, Message::DoNoting)
            }
            Some(PayloadOneof::ping(value)) => (source, Message::PingPongRead(value.name)),
            Some(PayloadOneof::package(package)) => {
                let p = Package::from_buffer(package.package);
                (source, Message::PackageShareRead(p, package.ttl))
            }
            Some(PayloadOneof::add_node(nodes)) => {
                (source, Message::AddNodeRead(nodes.nodes.into()))
            }
            Some(PayloadOneof::del_node(nodes)) => {
                (source, Message::DelNodeRead(nodes.nodes.into()))
            }
        }
    }
}
