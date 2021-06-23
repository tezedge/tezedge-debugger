use super::stack;

mod page;
mod page_history;
mod error;
mod allocation_state;
mod history;
mod report;

pub use self::allocation_state::AllocationState;
pub use self::{
    page::Page,
    page_history::{PageHistory, EventLast},
    history::History,
    report::FrameReport,
};

#[cfg(test)]
mod tests;
