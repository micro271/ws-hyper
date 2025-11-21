use regex::Regex;
use std::path::PathBuf;
use tokio::{fs, sync::mpsc::error::SendError};

pub trait AsyncRecv: Send {
    type Item;

    fn recv(&mut self) -> impl Future<Output = Option<Self::Item>> + Send;
}

pub trait AsyncSender: Send + 'static {
    type Item;

    fn send(
        &mut self,
        item: Self::Item,
    ) -> impl Future<Output = Result<(), SendError<Self::Item>>> + Send;
}

pub trait OneshotSender: Send + 'static {
    type Item;

    fn send(&self, item: Self::Item) -> Result<(), SendError<Self::Item>>;
}

pub trait TakeOwn<T: Send + 'static> {
    fn take(self) -> T;
}

pub trait FromDirEntyAsync<T>
where
    Self: Sized + Send,
{
    fn from_entry(value: T) -> impl Future<Output = Self>;
}

#[derive(Debug)]
pub enum ValidateError {
    RegexError(Box<dyn std::error::Error>),
    PathNotExist(PathBuf),
}

pub async fn validate_name_and_replace(path: PathBuf, to: &str) -> Result<(), ValidateError> {
    let re = Regex::new(r"(^\.[^.])|(^\.\.)|(\s+)|(^$)")
        .map_err(|x| ValidateError::RegexError(Box::new(x)))?;

    if !path.exists() {
        return Err(ValidateError::PathNotExist(path));
    }

    // TODO: We have to verify if the new file name already exists or not

    if re.is_match(to) {
        tracing::info!("[Validate Task] {{ Auto rename excecuted }} invalid file name: {to:?}");
        let new_to_file_name = re
            .replace_all(to, |caps: &regex::Captures<'_>| {
                if caps.get(1).is_some() {
                    "[DOT]".to_string()
                } else if caps.get(2).is_some() {
                    "[DOT][DOT]".to_string()
                } else if caps.get(3).is_some() {
                    "_".to_string()
                } else if caps.get(4).is_some() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            })
            .to_string();

        let mut path_from = PathBuf::from(&path);
        path_from.push(to);
        let mut path_to = PathBuf::from(&path);
        path_to.push(&new_to_file_name);

        tracing::debug!("[Validate Task] Attempt to rename from: {path_from:?} - to: {path_to:?}");

        if let Err(err) = fs::rename(&path_from, &path_to).await {
            tracing::error!(
                "[Validate Task] Auto rename error from: {path_from:?} to: {path_to:?}, error: {err}"
            );
        }
        tracing::warn!("[Validate Task] Auto rename from: {path_from:?} - to: {path_to:?}");
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Pending<W: Send + 'static>(W);

impl<W: Send + 'static> Pending<W> {
    pub fn new(inner: W) -> Self {
        Self(inner)
    }
}

#[derive(Debug, Clone)]
pub struct Executing;

impl<W: Send + 'static> TakeOwn<W> for Pending<W> {
    fn take(self) -> W {
        self.0
    }
}

pub trait Task {
    type Output: Send + 'static;

    fn task(self) -> impl Future<Output = Self::Output> + Send + 'static
    where
        Self: Sized;
}

pub trait Run: Task {
    fn run(self)
    where
        Self: Sized,
    {
        tokio::spawn(self.task());
    }
}
