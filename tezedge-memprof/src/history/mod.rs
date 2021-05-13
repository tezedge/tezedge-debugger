use super::stack::StackResolver;

mod page;
mod allocation;
mod error;
mod history;
mod report;

pub use self::{
    page::Page,
    allocation::{PageHistory, EventLast},
    history::History,
    report::FrameReport,
};
