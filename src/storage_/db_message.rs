pub trait DbMessage {
    fn set_id(&mut self, id: u64);
    fn set_ordinal_id(&mut self, id: u64);
}

pub trait Access<T> {
    fn accessor(&self) -> T;
}
