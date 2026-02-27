mod builder;
mod types;

use std::marker::PhantomData;
use std::sync::Arc;

pub use self::builder::*;
pub use self::types::*;
use super::{super::Cowed, Bucket, Key, LocalStorage, LsError, Object};
use nanoid::nanoid;
use regex::Regex;

const COLISION_DEFAULT_PREFIX_LENGTH: usize = 8;
const MAX_RENAME_ATTEMPTS: usize = 5;

pub struct NewObjNameHandler<'a> {
    object: &'a mut Object,
    key: Key<'a>,
    bucket: Bucket<'a>,
}

pub struct RenameObjHandler<'a> {
    bucket: Bucket<'a>,
    key: Key<'a>,
    from: &'a mut String,
    to: &'a mut String,
}

impl<'a> NewObjNameHandler<'a> {
    pub async fn run(&mut self, ls: Arc<LocalStorage>) {
        let mut count = 0;
        tracing::debug!("[NewObjNameHandler]");
        loop {
            if count == MAX_RENAME_ATTEMPTS {
                tracing::error!(
                    "[ RenameObjHandler] Mas attempts reached - object.name: {}",
                    self.object.name
                );
                break;
            }
            match ls
                .new_object(self.bucket.borrow(), self.key.borrow(), self.object)
                .await
            {
                Ok(e) => {
                    tracing::trace!("[ NewObjNameHandler ] {{ Object Inserted }} result: {e:?}");
                    break;
                }
                Err(LsError::DuplicateKey) => {
                    tracing::error!(
                        "[ NewObjectHandler ] {{ Duplicate key }} bucket: {}, key: {}, object.name: {} ",
                        self.bucket,
                        self.key,
                        self.object.name
                    );

                    name_generator(&mut self.object.name);
                    tracing::info!(
                        "[ ManagerRunning ] {{ Duplicate key }} object new name {}",
                        self.object.name
                    );
                }
                Err(e) => {
                    tracing::error!("[ NewObjHandler ] {{ Rename Fail }} {e}");
                    break;
                }
            }
            count += 1;
        }
    }
}

impl<'a> RenameObjHandler<'a> {
    pub async fn run(&mut self, ls: Arc<LocalStorage>) {
        let mut count = 0;

        loop {
            if count == MAX_RENAME_ATTEMPTS {
                tracing::error!(
                    "[ RenameObjHandler] Mas attempts reached - from: {}",
                    self.from
                );
                break;
            }

            match ls
                .set_name(self.bucket.borrow(), self.key.borrow(), self.from, self.to)
                .await
            {
                Err(LsError::DuplicateKey) => {
                    tracing::warn!(
                        "[ RenameObjectHandler ] {{ Duplicate key }} bucket: {}, key: {}, object.name: {} ",
                        self.bucket,
                        self.key,
                        self.to
                    );
                    name_generator(self.to);
                    tracing::info!(
                        "[ ManagerRunning ] {{ Duplicate key }} object new name {}",
                        self.to
                    );
                }
                Err(er) => {
                    tracing::error!("{er}");
                    break;
                }
                Ok(er) => {
                    tracing::info!("[ RenameObjHandler ] Rename in DB {er:?}");
                    break;
                }
            }

            count += 1;
        }
    }
}

fn name_generator(name: &mut String) {
    let regex_replace_prefix = Regex::new(r"(^(~.*\$).*)").unwrap();
    if regex_replace_prefix.is_match(name) {
        *name = regex_replace_prefix
            .replace(
                name,
                &format!("~{}$", nanoid!(COLISION_DEFAULT_PREFIX_LENGTH)),
            )
            .into_owned();
    } else {
        name.insert_str(0, &format!("~{}$", nanoid!(COLISION_DEFAULT_PREFIX_LENGTH)));
    }
}
