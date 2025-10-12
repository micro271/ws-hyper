pub trait FromDirEntyAsync<T>
where
    Self: Sized + Sync + Send,
{
    fn from_entry(value: T) -> impl Future<Output = Self>;
}
