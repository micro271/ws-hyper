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
    workdir: String,
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
                        Operation::Delete => delete(&format!("{}/{}", self.workdir, payload.bucket), false).await,
                        Operation::New => new(&format!("{}/{}", self.workdir, payload.bucket)).await,
                        Operation::Rename => {
                            if payload.old_bucket.is_none() {
                                return;
                            }
                            let new = format!("{}/{}", self.workdir, payload.bucket);
                            let old = format!("{}/{}", self.workdir, payload.old_bucket.unwrap());
                            rename(&new, &old).await;
                        },
                    }
                }
                change = rx.recv() => {
                    if let Some(some) = change {
                        change_(some).await
                    } else {
                        break;
                    }
                }
            };
        }
    }
}

pub async fn delete(bk: &str, force: bool) {
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

pub async fn new(name: &str) {
    if let Err(er) = tokio::fs::create_dir(name).await {
        tracing::error!("Create error: {er}");
    }
}

pub async fn rename(new: &str, old: &str) {
    if let Err(er) = tokio::fs::rename(old, new).await {
        tracing::error!("Create error: {er}");
    }
}

pub async fn change_(change: Change) {
    match change {
        Change::NewObject {
            bucket,
            key,
            object,
        } => todo!(),
        Change::NewKey { bucket, key } => todo!(),
        Change::NewBucket { bucket } => todo!(),
        Change::NameObject {
            bucket,
            key,
            from,
            to,
        } => todo!(),
        Change::NameBucket { from, to } => todo!(),
        Change::NameKey { bucket, from, to } => todo!(),
        Change::DeleteObject {
            bucket,
            key,
            object,
        } => todo!(),
        Change::DeleteKey { bucket, key } => todo!(),
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
