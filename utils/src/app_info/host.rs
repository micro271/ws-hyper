use std::{net::{IpAddr, Ipv4Addr, Ipv6Addr}, ops::Not};

use regex::Regex;

use crate::app_info::Proto;

#[derive(Debug, Clone)]
pub enum HostType {
    Domain {
        proto: Proto,
        domain: String,
    },
    Ipv4 {
        proto: Proto,
        ip: Ipv4Addr,
    },
    Ipv6 {
        proto: Proto,
        ip: Ipv6Addr,
    }
}

impl HostType {
    pub fn proto(&self) -> Proto {
        match self {
            HostType::Domain { proto, .. } => *proto,
            HostType::Ipv4 { proto, .. } => *proto,
            HostType::Ipv6 { proto, .. } => *proto,
        }
    }

    pub fn domain(&self) -> String {
        match self {
            HostType::Domain { domain, .. } => domain.to_string(),
            HostType::Ipv4 { ip, .. } => ip.to_string(),
            HostType::Ipv6 { ip, .. } => ip.to_string(),
        }
    }
}


#[derive(Debug, Clone)]
pub struct Host {
    host: HostType,
}

impl Host {
    pub fn proto(&self) -> Proto {
        self.host.proto()
    }

    pub fn domain(&self) -> String {
        self.host.domain()
    }

    pub fn host(&self) -> String {
        format!("{}://{}/", self.proto(), self.domain())
    }

    pub fn new_ip<T: Into<Proto>, K: Into<IpAddr>>(proto: T, ip: K) -> Self {
        let ip = ip.into();
        Self {
            host: {
                match ip {
                    IpAddr::V4(ip) => {
                        HostType::Ipv4 { proto: proto.into(), ip  }
                    },
                    IpAddr::V6(ip) => {
                        HostType::Ipv6 { proto: proto.into(), ip  }
                    },
                }
            }
        }
    }

    pub fn new_domain<T: Into<Proto>, K: Into<String>>(proto: T, domain: K) -> Self {
        
        Self {
            host: {
                HostType::Domain { proto: proto.into(), domain: domain.into() }
            }
        }
    }

    pub fn from_url(url: &str) -> Result<Self, ()> {
        let reg = Regex::new(r"^(?P<proto>https?)://(?:(?P<ip>\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}|\[[0-9:]*\])|(?P<domain>[^-].*))(?P<port>:\d)?/$").unwrap();
        if let Some(cap) = reg.captures(url) {
            let proto = &cap["proto"];
            tracing::info!("{proto}");
            let resp = if let Some(ip) = cap.name("ip").and_then(|x| x.as_str().parse::<IpAddr>().ok()) {
                tracing::info!("{ip}");
                if cap.name("port").and_then(|x| x.as_str().parse::<usize>().ok()).is_some_and(|x| (1..65535).contains(&x).not())  {
                    tracing::error!("Host invalid port");
                    return Err(());
                }
                Self::new_ip(proto, ip)
            } else if let Some(domain) = cap.name("domain").map(|x| x.as_str()) {
                tracing::info!("{domain}");
                Self::new_domain(proto, domain)
            } else {
                return Err(());
            };

            Ok(resp)
        } else {
            Err(())
        }

    }
}

