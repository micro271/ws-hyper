pub mod file;
use std::fs::DirEntry;
use std::borrow::Cow;


#[derive(Debug)]
pub struct Directory<'a> {
    path: Cow<'a, str>,
    len: u64,
}

impl<'a> TryFrom<DirEntry> for Directory<'a> {
    type Error = FromEntryToDirErr;
    fn try_from(value: DirEntry) -> Result<Self, Self::Error> {
        if !value.file_type().unwrap().is_dir() {
            return Err(FromEntryToDirErr);
        }

        Ok(Self {
            path: Cow::Owned(value.path().to_str().unwrap().to_string()),
            len: value.metadata().unwrap().len(),
        })
    }
}

#[derive(Debug)]
pub struct FromEntryToDirErr;

impl std::fmt::Display for FromEntryToDirErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "This is not a directory")
    }
}

impl std::error::Error for FromEntryToDirErr {}

impl<'a> AsRef<str> for Directory<'a> {
    fn as_ref(&self) -> &str {
        &self.path
    }
}

impl<'a, T> std::cmp::PartialEq<T> for Directory<'a> 
where 
    T: AsRef<str>
{
    fn eq(&self, other: &T) -> bool {
        self.path.eq(other.as_ref())
    }
}

impl<'a> std::cmp::Eq for Directory<'a> { }

impl<'a, T> std::cmp::PartialOrd<T> for Directory<'a> 
    where 
        T: AsRef<str>,
{
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }
}

impl<'a> std::cmp::Ord for Directory<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}