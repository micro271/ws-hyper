use clap::{Parser, ValueEnum, command};
use std::net::Ipv4Addr;
use tracing::Level;

#[derive(Parser)]
#[command(version, about)]
pub struct Args {
    #[arg(long, value_enum, default_value = "event")]
    pub watcher: TypeWatcher,

    #[arg(long = "watcher-path", env = "ROOT_PATH")]
    pub watcher_path: String,

    #[arg(long, short, env = "IP_ADDRESS", default_value = "0.0.0.0")]
    pub listen: Ipv4Addr,

    #[arg(long, short, env = "PORT", default_value = "3500")]
    pub port: u16,

    #[arg(long = "log-level", env = "LOG_LEVEL", default_value = "info")]
    pub log_level: LogLebel,

    #[arg(
        long = "prefix-root",
        env = "PREFIX_ROOT",
        default_value = ".",
        help = "Prefix of the root directory"
    )]
    pub prefix_root: String,
}

#[derive(Clone, ValueEnum)]
pub enum TypeWatcher {
    Poll,
    Event,
}

#[derive(Clone, ValueEnum)]
pub enum LogLebel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLebel> for Level {
    fn from(value: LogLebel) -> Self {
        match value {
            LogLebel::Trace => Self::TRACE,
            LogLebel::Debug => Self::DEBUG,
            LogLebel::Info => Self::INFO,
            LogLebel::Warn => Self::WARN,
            LogLebel::Error => Self::ERROR,
        }
    }
}
