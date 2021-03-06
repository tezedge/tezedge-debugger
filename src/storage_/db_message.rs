pub trait Access<T> {
    fn accessor(&self) -> T;
}
