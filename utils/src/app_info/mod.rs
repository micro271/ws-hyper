pub mod host;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Proto {
    Http,
    Https,
    #[default]
    Unknown,
    Ws,
    Wss,
    Ftp,
}

impl std::fmt::Display for Proto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Proto::Http => write!(f, "http"),
            Proto::Https => write!(f, "https"),
            Proto::Unknown => write!(f, "unknown"),
            Proto::Ws => write!(f, "ws"),
            Proto::Ftp => write!(f, "ftp"),
            Proto::Wss => write!(f, "wss"),
        }
    }
}

impl From<&str> for Proto {
    fn from(value: &str) -> Self {
        match value {
            "http" => Self::Http,
            "https" => Self::Https,
            "ws" => Self::Ws,
            "wss" => Self::Wss,
            "ftp" => Self::Ftp,
            _ => Self::Unknown,
        }
    }
}
