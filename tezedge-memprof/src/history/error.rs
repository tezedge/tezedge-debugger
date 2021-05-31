use serde::Serialize;
use super::page::Page;

#[derive(Default, Serialize)]
pub struct ErrorReport {
    double_free: Vec<Page>,
    without_alloc: Vec<Page>,
    double_alloc: Vec<Page>,
}

impl ErrorReport {
    pub fn double_free(&mut self, page: &Page) {
        self.double_free.push(page.clone());
    }

    pub fn without_alloc(&mut self, page: &Page) {
        self.without_alloc.push(page.clone());
    }

    pub fn double_alloc(&mut self, page: &Page) {
        self.double_alloc.push(page.clone());
    }
}
