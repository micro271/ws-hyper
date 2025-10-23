use std::path::PathBuf;


pub trait ParserFrom<T: Send>: Send + Sync + Sized {
    fn parser(&self, from: PathBuf) -> T;
}

pub struct ParserError;