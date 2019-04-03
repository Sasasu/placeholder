local get_peer(servers) = [
    {
        address: server.address,
        port: server.port,
        name: server.name
    } for server in servers
];

local get_config(servers, server) = {
        device_name: "ph0",
        device_type: "tun",
        name: server.name,
        subnet: server.subnet,
        port: server.port,
        servers: get_peer(servers),
        ifup: |||
            ip link set $INTERFACE up
            ip address add %s dev $INTERFACE
            ip link set dev $INTERFACE mtu 1400
        ||| % server.net ,
        ifdown: |||
            ip address del $IP_ADDR_MASK dev $INTERFACE
            ip link set $INTERFACE down
        |||
};

local servers = [
    {
        address: "192.168.56.2",
        port: 5432,
        name: "node0",
        net:    "10.1.0.1/16",
        subnet: "10.1.0.1/24"
    },
    {
        address: "192.168.56.14",
        port: 5432,
        name: "node1",
        net:    "10.1.1.1/16",
        subnet: "10.1.1.1/24"
    }
];

{
    "node0.yaml": get_config(servers, servers[0]),
    "node1.yaml": get_config(servers, servers[1]),
}


