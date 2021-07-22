use std::sync::Arc;
use bpf_memprof_common::{Hex64, Stack};

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct FuncPath(Arc<Vec<Hex64>>);

impl FuncPath {
    pub fn new(stack: &Stack) -> Self {
        FuncPath(Arc::new(stack.ips().to_vec()))
    }
}

impl AsRef<[Hex64]> for FuncPath {
    fn as_ref(&self) -> &[Hex64] {
        self.0.as_ref()
    }
}
