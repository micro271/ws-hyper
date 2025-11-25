pub mod builder;

use std::path::PathBuf;

use serde::Deserialize;
use sqlx::{PgConnection, postgres::PgListener};
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
};

use crate::manager::{
    Change,
    utils::{SplitTask, Task},
};

#[derive(Debug)]
pub struct ListenBucketChSender(Sender<Change>);

impl ListenBucketChSender {
    pub fn new(sender: Sender<Change>) -> Self {
        Self(sender)
    }
}

impl std::ops::Deref for ListenBucketChSender {
    type Target = Sender<Change>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct ListenBucket {
    lst: PgListener,
    conn: PgConnection,
    channel: String,
    tx: Option<Sender<Change>>,
    rx: Option<Receiver<Change>>,
    workdir: PathBuf,
}

impl Task for ListenBucket {
    async fn task(mut self)
    where
        Self: Sized,
    {
        self.lst.listen(&self.channel).await.unwrap();

        let mut rx = self.rx.take().unwrap();

        loop {
            select! {
                msg = self.lst.recv() => {
                    let payload = serde_json::from_str::<Payload>(msg.unwrap().payload()).unwrap();
                    match payload.operation {
                        Operation::Delete => {
                            let mut path = self.workdir.clone();
                            path.push(payload.bucket);
                            delete(path, false).await
                        },
                        Operation::New => {
                            let mut path = self.workdir.clone();
                            path.push(payload.bucket);
                            new(path).await
                        },
                        Operation::Rename => {
                            let Some(old_bucket) = payload.old_bucket else {
                                return;
                            };
                            let mut from = self.workdir.clone();
                            let mut to = self.workdir.clone();
                            from.push(old_bucket);
                            to.push(payload.bucket);
                            rename(from, to).await;
                        },
                    }
                }
                change = rx.recv() => {
                    if let Some(some) = change {
                        change_(some, self.workdir.clone()).await
                    } else {
                        break;
                    }
                }
            };
        }
    }
}

pub async fn delete(bk: PathBuf, force: bool) {
    let mut path = PathBuf::from(bk);
    if !path.exists() {
        tracing::warn!("dir: {path:?} not found");
        return;
    }

    if !path.is_dir() {
        tracing::warn!("dir: {path:?} doesn't dir");
        return;
    }

    if force && let Err(err) = tokio::fs::remove_dir_all(&path).await {
        tracing::error!("{err}");
        return;
    } else if tokio::fs::remove_dir(&path)
        .await
        .is_err_and(|x| x.kind() == tokio::io::ErrorKind::DirectoryNotEmpty)
    {
        let date = time::OffsetDateTime::now_local().unwrap();
        let id = nanoid::nanoid!(4);
        let date = date
            .format(
                &time::format_description::parse(
                    "[day]-[month]-[year repr:last_two]_[hour]-[minute]",
                )
                .unwrap(),
            )
            .unwrap();
        let file_name = path.file_name().unwrap().to_string_lossy();
        let new_name = format!("DELETED-{id}-{date}_{file_name}");
        let from = path.clone();
        path.pop();
        path.push(new_name);
        if let Err(er) = tokio::fs::rename(from, path).await {
            tracing::error!("{er}");
        }
        return;
    };

    tracing::warn!("{path:?} deleted");
}

pub async fn new(name: PathBuf) {
    if let Err(er) = tokio::fs::create_dir(name).await {
        tracing::error!("Create error: {er}");
    }
}

pub async fn rename(new: PathBuf, old: PathBuf) {
    if let Err(er) = tokio::fs::rename(old, new).await {
        tracing::error!("Create error: {er}");
    }
}

pub async fn change_(change: Change, mut workdir: PathBuf) {
    match change {
        Change::NewBucket { bucket } => {
            workdir.push(bucket.as_ref());
            new(workdir).await;
        }
        Change::NameBucket { from, to } => {
            let mut from_ = workdir.clone();
            from_.push(from.as_ref());

            workdir.push(to.as_ref());
            rename(from_, workdir).await;
        }
        Change::DeleteBucket { bucket } => {
            workdir.push(bucket.as_ref());
            delete(workdir, false).await;
        }
        _ => {}
    }
}

impl SplitTask for ListenBucket {
    type Output = ListenBucketChSender;

    fn split(mut self) -> (<Self as SplitTask>::Output, impl crate::manager::utils::Run) {
        (ListenBucketChSender::new(self.tx.take().unwrap()), self)
    }
}

#[derive(Debug, Deserialize)]
pub struct Payload {
    operation: Operation,
    bucket: String,
    old_bucket: Option<String>,
}

#[derive(Debug, Deserialize)]
enum Operation {
    Delete,
    New,
    Rename,
}
