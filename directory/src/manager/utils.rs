use regex::Regex;
use std::path::PathBuf;
use tokio::fs;

pub trait TakeOwn<T: Send> {
    fn take(self) -> T;
}

pub trait FromDirEntyAsync<T>
where
    Self: Sized + Sync + Send,
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

macro_rules! match_error {
    ($e:expr) => {
        match $e {
            Ok(e) => (e),
            Err(err) => {
                tracing::error!("{err}");
                continue;
            }
        }
    };
    ($e:expr, $prefix: expr) => {
        match $e {
            Ok(e) => (e),
            Err(err) => {
                tracing::error!("{} {err}", $prefix);
                continue;
            }
        }
    };
}

pub(crate) use match_error;
