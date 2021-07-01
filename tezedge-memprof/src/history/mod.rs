use super::stack;

mod abstract_tracker;
mod page;
mod page_history;
mod error;
mod allocation;
mod history;
mod report;

pub use self::abstract_tracker::{Tracker, Reporter};
pub use self::allocation::AllocationState;
pub use self::{
    page::Page,
    page_history::{PageHistory, EventLast},
    history::History,
    report::FrameReport,
};

#[cfg(test)]
mod tests;
