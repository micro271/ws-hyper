use clap::{Parser, ValueEnum, command};
use std::{net::Ipv4Addr, path::PathBuf};
use tonic::transport::Endpoint;
use tracing::Level;

#[derive(Parser)]
#[command(version, about)]
pub struct Args {
    #[arg(long, value_enum, default_value = "event")]
    pub watcher: TypeWatcher,

    #[arg(long = "watcher-path", env = "ROOT_PATH")]
    pub watcher_path: PathBuf,

    #[arg(long, short, env = "IP_ADDRESS", default_value = "0.0.0.0")]
    pub listen: Ipv4Addr,

    #[arg(long, short, env = "PORT", default_value = "3500")]
    pub port: u16,

    #[arg(long = "log-level", env = "LOG_LEVEL", default_value = "info")]
    pub log_level: LogLebel,

    #[arg(
        long = "grpc-auth-server",
        env = "GRPC_AUTH_SERVER",
        help = "Endpoint to auth server",
        default_value = "[::1]:50051"
    )]
    pub grpc_auth_server: Endpoint,

    #[arg(long = "database-name", env = "DATABASE_NAME")]
    pub database_name: String,

    #[arg(long = "database-user", env = "DATABASE_USER")]
    pub username: String,

    #[arg(long = "database-pass", env = "DATABASE_PASSWD")]
    pub password: String,

    #[arg(long = "database-listen-channel", env = "DATABASE_LISTEN_CHANNEL")]
    pub channel: String,

    #[arg(long = "database-host", env = "DATABASE_HOST")]
    pub database_host: String,

    #[arg(long = "database-port", env = "DATABASE_PORT")]
    pub database_port: u16,

    #[arg(long = "md-host", env = "MD_DATABASE_HOST")]
    pub md_host: String,

    #[arg(long = "md-port", env = "MD_DATABASE_PORT")]
    pub md_port: u16,

    #[arg(long = "md-username", env = "MD_DATABASE_USERNAME")]
    pub md_username: String,

    #[arg(long = "md-password", env = "MD_DATABASE_PASSWORD")]
    pub md_pass: String,

    #[arg(long = "md-database-name", env = "MD_DATABASE_NAME")]
    pub md_database: String,

    #[arg(long = "ignore-rename-suffix", env = "IGNORE_RENAME_SUFFIX", default_value = ".__stream-in-progress")]
    pub ignore_rename_suffix: String,
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
