use nanoid::nanoid;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::bucket::utils::rename_handlers::{RenamedToNoTo, RenamedToWithTo};
use crate::bucket::{DEFAULT_LENGTH_NANOID, utils::Renamed};
use crate::manager::utils::Task;
use crate::manager::utils::{OBJECT_NAME_REPEATED, SplitTask};
use tokio::sync::oneshot::{Receiver, Sender, channel};

#[derive(Debug)]
pub struct RenamedTo<'a, T> {
    from: &'a Path,
    to: T,
    tx: Sender<String>,
    rx: Option<Receiver<String>>,
}

pub struct RenamedToTask {
    from: PathBuf,
    to: PathBuf,
    tx: Sender<String>,
}

impl<'a> RenamedTo<'a, RenamedToWithTo> {
    pub fn file_name(&self) -> &str {
        self.to.file_name()
    }
    pub fn get_to(&self) -> &Path {
        self.to.path()
    }
}

impl<'a> RenamedTo<'a, RenamedToNoTo> {
    pub fn get_from(&self) -> &Path {
        self.from
    }

    pub fn new(from: &'a Path) -> Self {
        let (tx, rx) = channel();
        Self {
            from,
            to: RenamedToNoTo,
            tx,
            rx: Some(rx),
        }
    }

    pub fn to<T: Into<PathBuf>>(self, path: T) -> RenamedTo<'a, RenamedToWithTo> {
        RenamedTo {
            from: self.from,
            to: RenamedToWithTo(path.into()),
            rx: self.rx,
            tx: self.tx,
        }
    }
}

impl<'a> SplitTask for RenamedTo<'a, RenamedToWithTo> {
    type Output = Receiver<String>;

    fn split(mut self) -> (<Self as SplitTask>::Output, impl crate::manager::utils::Run) {
        let RenamedToWithTo(to) = self.to;
        (
            self.rx.take().unwrap(),
            RenamedToTask {
                from: self.from.to_path_buf(),
                to,
                tx: self.tx,
            },
        )
    }
}

impl Task for RenamedToTask {
    async fn task(self)
    where
        Self: Sized,
    {
        if let Err(er) = tokio::fs::rename(&self.from, &self.to).await {
            tracing::error!(
                "[ RenamedToTask ] - Error to rename file from: {:?} - to: {:?}",
                self.from,
                self.to
            );
            tracing::error!("IoError: {er}");
        }
        tracing::warn!(
            "[ RenamedToTask ] {{ Rename file }} from: {:?} to: {:?}",
            self.from,
            self.to
        );

        if let Err(er) = self.tx.send(
            self.to
                .file_name()
                .and_then(|x| x.to_str().map(ToString::to_string))
                .unwrap(),
        ) {
            tracing::error!("[ RenamedToTask ] Error to send {er:?}");
        }
    }
}

#[derive(Debug, Default)]
pub struct NormalizePathUtf8 {
    new_path: bool,
}

impl NormalizePathUtf8 {
    pub fn is_new(mut self) -> Self {
        self.new_path = true;
        self
    }

    pub async fn run(self, to: &Path) -> Renamed<'_> {
        if let Some(str) = to.file_name().map(|x| x.to_string_lossy()) {
            let new_name = str.trim_start_matches("-").replace("\u{FFFD}", "_");

            if !new_name.is_empty() {
                if str != new_name {
                    tracing::trace!(
                        "[ NormalizePathUtf8 ] We going to rename the directory from {str} to {new_name}"
                    );
                    let mut to_ = PathBuf::from(to);
                    to_.pop();
                    to_.push(new_name);
                    return Renamed::Yes(RenamedTo::new(to).to(to_));
                } else {
                    return Renamed::Not(new_name);
                }
            }
        }

        if self.new_path {
            let mut to_ = to.to_path_buf();
            to_.pop();
            to_.push(format!("INVALID_NAME-{}", nanoid!(DEFAULT_LENGTH_NANOID)));
            Renamed::Yes(RenamedTo::new(to).to(to_))
        } else {
            Renamed::NeedRestore(RenamedTo::new(to))
        }
    }
}

#[derive(Debug)]
pub struct NormalizeFileUtf8;

impl NormalizeFileUtf8 {
    pub async fn run(path: &Path) -> Renamed<'_> {
        if let Some(str) = path.file_name().and_then(|x| x.to_str()) {
            Renamed::Not(str.to_string())
        } else {
            let mut to = PathBuf::from(path);
            let file_name = to
                .file_name()
                .map(|x| x.to_string_lossy().replace("\u{FFFD}", ""))
                .filter(|x| !x.is_empty())
                .unwrap_or(nanoid!(24).to_string());

            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("__unknown");

            let new_name = format!("{file_name}.{ext}",);
            to.push(&new_name);

            if to.exists() {
                if OBJECT_NAME_REPEATED.is_match(&new_name) {
                    todo!()
                }
            }

            tracing::warn!("[NormalizeFileUtf] {{ Rename file }} from: {path:?} to: {to:?}");
            Renamed::Yes(RenamedTo::new(path).to(to))
        }
    }
}
