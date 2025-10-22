#[derive(Debug)]
pub struct WatcherErr(String);

impl WatcherErr {
    pub fn new<T: AsRef<str>>(detail: T) -> Self {
        Self(detail.as_ref().to_string())
    }
}

impl std::fmt::Display for WatcherErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for WatcherErr {}
