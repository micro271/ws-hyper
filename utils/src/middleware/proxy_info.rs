use std::net::IpAddr;

use http::Response;
use hyper::body::Body;

use crate::{Peer, middleware::IntoLayer};

pub struct ProxyInfoLayer;

#[derive(Debug, Clone)]
pub enum ProxyInfoType {
    Forwarded {
        r#for: Ip,
        by: Ip,
        proto: Proto,
    },
    XRealIp { ip: Ip },
    XForwardedFor {
        ip: Ip,
        proxys: Vec<Ip>
    }
}

impl std::default::Default for ProxyInfoType {
    fn default() -> Self {
        Self::XRealIp { ip: Ip::default() }
    }
}

#[derive(Debug, Clone)]
pub enum Ip {
    Ip(IpAddr),
    Unknown,
}

impl From<&str> for Ip {
    fn from(value: &str) -> Self {
        if let Ok(ip) = value.parse() {
            Self::Ip(ip)
        } else {
            Self::Unknown
        }
    }
}

impl From<IpAddr> for Ip {
    fn from(value: IpAddr) -> Self {
        Self::Ip(value)
    }
}

impl std::default::Default for Ip {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Proto {
    Http,
    Https,
    Unknown,
    Ws,
    Wss
}

impl std::fmt::Display for Proto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Proto::Http => write!(f, "http"),
            Proto::Https => write!(f, "https"),
            Proto::Unknown => write!(f, "unknown"),
            Proto::Ws => write!(f, "Ws"),
            Proto::Wss => write!(f, "Wss"),
        }
    }
}

impl From<&str> for Proto {
    fn from(value: &str) -> Self {
        match value {
            "http" => Self::Http,
            "https" => Self::Http,
            "ws" => Self::Ws,
            "wss" => Self::Wss,
            _ => Self::Unknown
        }
    }
}

impl<L, ReqBody, ResBody> IntoLayer<L, ReqBody, ResBody> for ProxyInfoLayer 
where 
    L: super::Layer<ReqBody, ResBody> + Clone,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    fn into_layer(self, inner: L) -> Self::Output where Self: Sized {
        ProxyInfo { inner }
    }
    
    type Output = ProxyInfo<L>;
}

#[derive(Debug, Clone)]
pub struct ProxyInfo<L> {
    inner: L
}

impl<L, ReqBody, ResBody> super::Layer<ReqBody, ResBody> for ProxyInfo<L> 
where 
    L: super::Layer<ReqBody, ResBody> + Clone,
    ReqBody: Body + Send,
    ResBody: Body + Send + Default,
{
    type Error = L::Error;

    fn call(&self, mut req: http::Request<ReqBody>) -> impl Future<Output = Result<http::Response<ResBody>, Self::Error> > {

        let mut info= None;
        
        if let Some(fw) = req.headers().get("Forwarded").and_then(|x| x.to_str().ok()) {
            let list = fw.split(";").nth(0).map(|x| x.split(",").map(str::trim_start).collect::<Vec<_>>()).unwrap();
            let mut r#for = None;
            let mut by = None;
            let mut proto = None;
            for i in list {
                let (key, value) = i.split_once("=").unwrap();
                if key == "for" {
                    r#for = Some(value);
                } else if key == "by" {
                    by = Some(value);
                } else if key == "proto" {
                    proto = Some(value);
                }
            }

            info = Some(ProxyInfoType::Forwarded { r#for: r#for.unwrap().into(), by: by.unwrap().into(), proto: proto.unwrap().into() });

        } else if let Some(fw) = req.headers().get("X-Forwarded-For").and_then(|x| x.to_str().ok()) {
            let mut list = fw.split(",").map(str::trim_start).map(|x| x.into()).collect::<Vec<_>>();
            info = Some(ProxyInfoType::XForwardedFor { ip: list.remove(0), proxys: list })
        } else if let Some(ip) = req.headers().get("X-Real-Ip").and_then(|x| x.to_str().ok().map(|x| Ip::from(x))) {
            info = Some(ProxyInfoType::XRealIp { ip });
        } else if let Some(Peer(ip)) = req.extensions_mut().remove::<Peer>() {
            let ip = ip.map(|x| x.ip()).map(Ip::from).unwrap_or_default();
            info = Some(ProxyInfoType::XRealIp { ip })
        }

        req.extensions_mut().insert(info.unwrap_or_default());

        ResponseFutureProxyInfo{
            f: self.inner.call(req)
        }
    }
}

pub struct ResponseFutureProxyInfo<Fut> {
    f: Fut
}

impl<Fut, ReqBody, E> Future for ResponseFutureProxyInfo<Fut> 
where 
    Fut: Future<Output = Result<Response<ReqBody>, E>>,
    ReqBody: Body + Send,
{
    type Output = Result<Response<ReqBody>, E>;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        unsafe { self.map_unchecked_mut(|x| &mut x.f)}.poll(cx)
    }

}