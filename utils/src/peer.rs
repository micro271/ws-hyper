use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, Copy)]
pub struct Peer(Option<SocketAddr>);

impl Peer {
    pub fn new(socket: Option<SocketAddr>) -> Self {
        Self(socket)
    }

    pub fn get_socket_or_unknown(&self) -> String {
        self.0.map_or("Unknown".to_string(), |x| x.to_string())
    }

    pub fn get_ip(&self) -> Option<IpAddr> {
        self.0.map(|x| x.ip())
    }
}