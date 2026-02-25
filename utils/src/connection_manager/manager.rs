use std::{collections::VecDeque, time::Duration};

use tokio::{
    select,
    sync::{
        Semaphore,
        mpsc::{Receiver, Sender, channel},
    },
};

use crate::connection_manager::{Konnection, Ping};

pub struct Connection<Msg> {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl<Msg> Connection<Msg> {
    pub async fn send(&mut self, msg: Msg) -> Msg {
        _ = self.tx.send(msg);
        self.rx.recv().await.unwrap()
    }
}

#[derive(Debug)]
pub struct ConectionManager<C> {
    connection: C,
    status: ConnectionStatus,
    retry: u64,
}

impl<C> ConectionManager<C>
where
    C: Konnection + Send + 'static,
{
    fn new(conn: C, retry_ms: u64) -> Self {
        Self {
            connection: conn,
            status: ConnectionStatus::Pending,
            retry: retry_ms,
        }
    }

    fn connect(mut self) -> Connection<C::Message> {
        let (tx, mut rx) = channel::<C::Message>(128);
        let (tx_2, rx_2) = channel::<C::Message>(128);

        tokio::spawn(async move {
            loop {
                select! {
                    _ = tokio::time::sleep(Duration::from_millis(self.retry)) => {
                        if self.connection.ping().await == Ping::Loss {
                            tracing::error!("Connection Loss");
                            if self.status == ConnectionStatus::Ok {
                                self.status = ConnectionStatus::Loss;
                            }
                        } else if self.status == ConnectionStatus::Loss {
                            tracing::info!("Connection reestablished");
                            self.status = ConnectionStatus::Ok;
                        }
                    }
                    msg = rx.recv() => {
                        match msg {
                            Some(msg) => {
                                match self.connection.handler(msg.clone()).await {
                                    Ok(msg) => {
                                        if self.status == ConnectionStatus::Loss {
                                            self.status = ConnectionStatus::Ok;
                                        }
                                        _= tx_2.send(msg).await;
                                    },
                                    Err(super::error::Error::ConnectionLoss) => {
                                        self.status = ConnectionStatus::Loss;
                                    }
                                }

                            },
                            None => todo!(),
                        }

                    }
                }
            }
        });

        Connection { tx, rx: rx_2 }
    }
}

#[derive(Debug, PartialEq)]
pub enum ConnectionStatus {
    Ok,
    Loss,
    Pending,
}
