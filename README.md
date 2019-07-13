# placeholder

a mesh VPN writen in Rust and tokio.

Feature:

1. Fast, 2 core VM can routing 600k package pre second (Good)
1. But still alloc memory at runtime (Bad)
1. But still use `recvfrom` and `sendto` (Bad)
1. No encryption (Bad)
1. Full mesh, auto routing and HA (Good)
1. Status detect timer is not implemented (Bad)

lisense:

MIT
