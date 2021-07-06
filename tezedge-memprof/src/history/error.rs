// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;
use super::page::Page;

#[derive(Default, Serialize)]
pub struct ErrorReport {
    enabled: bool,
    double_free: Vec<Page>,
    without_alloc: Vec<Page>,
    double_alloc: Vec<Page>,
}

impl ErrorReport {
    #[allow(dead_code)]
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn double_free(&mut self, page: &Page) {
        if self.enabled {
            self.double_free.push(page.clone());
        }
    }

    pub fn without_alloc(&mut self, page: &Page) {
        if self.enabled {
            self.without_alloc.push(page.clone());
        }
    }

    pub fn double_alloc(&mut self, page: &Page) {
        if self.enabled {
            self.double_alloc.push(page.clone());
        }
    }
}
