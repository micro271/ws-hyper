use sqlx::{
    Connection, PgConnection,
    postgres::{PgConnectOptions, PgListener},
};

use crate::state::pg_listen::ListenBucket;

#[derive(Debug, Default)]
pub struct ListenBucketBuilder {
    username: Option<String>,
    password: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    database: Option<String>,
    channel: Option<String>,
    workdir: Option<String>,
}

impl ListenBucketBuilder {
    pub fn username(mut self, username: String) -> Self {
        self.username = Some(username);

        self
    }

    pub fn workdir(mut self, workdir: String) -> Self {
        self.username = Some(workdir);

        self
    }

    pub fn password(mut self, password: String) -> Self {
        self.password = Some(password);

        self
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = Some(host);

        self
    }

    pub fn channel(mut self, channel: String) -> Self {
        self.channel = Some(channel);

        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);

        self
    }

    pub fn database(mut self, db: String) -> Self {
        self.database = Some(db);

        self
    }

    pub async fn build(self) -> ListenBucket {
        let Self {
            username,
            password,
            host,
            port,
            database,
            channel,
            workdir,
        } = self;
        let username = username.expect("Username not defined");
        let password = password.expect("Password not defined");
        let host = host.expect("Host not defined");
        let port = port.expect("Port not defined");
        let database = database.expect("Database not defined");
        let workdir = workdir.expect("Database not defined");

        let url = format!(
            "postgres://{}:{}@{}:{}/{}",
            username, password, host, port, database
        );
        let opts = PgConnectOptions::new()
            .database(&database)
            .username(&username)
            .password(&password)
            .host(&host)
            .port(port);
        let conn = PgConnection::connect_with(&opts).await.unwrap();
        let lst = PgListener::connect(&url).await.unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        ListenBucket {
            conn,
            lst,
            channel: channel.expect("Channel not defined"),
            tx: tx.into(),
            rx: rx.into(),
            workdir,
        }
    }
}
