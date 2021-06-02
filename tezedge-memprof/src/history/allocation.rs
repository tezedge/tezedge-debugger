use std::ops::Range;
use bpf_memprof::Hex32;
use thiserror::Error;
use serde::Serialize;

#[derive(Serialize)]
pub struct TimeRange(Range<u64>);

impl TimeRange {
    pub fn open_end(&self) -> bool {
        self.0.end == u64::MAX
    }

    fn now() -> u64 {
        use std::time::{SystemTime, Duration};

        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::default())
            .as_millis() as u64
    }

    pub fn set_end(&mut self) {
        self.0.end = Self::now();
    }

    pub fn begin_here() -> Self {
        TimeRange(Self::now()..u64::MAX)
    }

    pub fn end_here() -> Self {
        TimeRange(0..Self::now())
    }
}

#[derive(Serialize)]
pub struct Event {
    time_range: TimeRange,
    flags: Hex32,
}

impl Event {
    const EVENT_BASED_CACHE_FLAG: u32 = 1 << 31;

    #[allow(dead_code)]
    const GFP_WRITE: u32 = 0x1000;

    pub fn page_cache(&self) -> bool {
        (self.flags.0 & Self::EVENT_BASED_CACHE_FLAG) != 0
    }

    pub fn mark_page_cache(&mut self, b: bool) {
        if b {
            self.flags.0 |= Self::EVENT_BASED_CACHE_FLAG;
        } else {
            self.flags.0 &= !Self::EVENT_BASED_CACHE_FLAG;
        }
    }
}

#[derive(Debug, Error)]
#[error("double alloc")]
pub struct AllocError;

#[derive(Debug, Error)]
pub enum FreeError {
    #[error("double free")]
    DoubleFree,
    #[error("free without alloc")]
    WithoutAlloc,
}

pub trait PageHistory {
    fn track_alloc(&mut self, flags: Hex32) -> Result<(), AllocError>;
    fn track_free(&mut self) -> Result<(), FreeError>;
    fn is_allocated(&self, time: Option<u64>) -> bool;

    fn mark_page_cache(&mut self, b: bool);
    fn page_cache(&self) -> bool;

    fn is_empty(&self) -> bool;
}

#[derive(Default, Serialize)]
pub struct EventLast(Option<Event>);

impl PageHistory for EventLast {
    fn track_alloc(&mut self, flags: Hex32) -> Result<(), AllocError> {
        // if have some event in history and time range is open, track double allocation
        // if there is nothing in history or some old event, track a new allocation
        match self.0.as_mut() {
            Some(event) if event.time_range.open_end() => {
                Err(AllocError)
            },
            _ => {
                self.0 = Some(Event {
                    time_range: TimeRange::begin_here(),
                    flags,
                });
                Ok(())
            },
        }
    }

    fn track_free(&mut self) -> Result<(), FreeError> {
        match self.0.as_mut() {
            // have some allocation event, but end is open, set the end now
            Some(event) if event.time_range.open_end() => {
                event.time_range.set_end();
                Ok(())
            },
            // have some allocation event, already end, so it is double free
            Some(event) => {
                event.time_range.set_end();
                Err(FreeError::DoubleFree)
            },
            // have nothing, it is free without alloc
            None => {
                self.0 = Some(Event {
                    time_range: TimeRange::end_here(),
                    flags: Hex32(0),
                });
                Err(FreeError::WithoutAlloc)
            },
        }
    }
    
    fn is_allocated(&self, time: Option<u64>) -> bool {
        match (time, &self.0) {
            (_, &None) => false,
            (None, &Some(Event { ref time_range, .. })) => time_range.open_end(),
            (Some(time), &Some(Event { ref time_range, .. })) => time_range.0.contains(&time),
        }
    }

    fn mark_page_cache(&mut self, b: bool) {
        if let &mut Some(ref mut event) = &mut self.0 {
            event.mark_page_cache(b);
        }
    }

    fn page_cache(&self) -> bool {
        if let &Some(ref event) = &self.0 {
            event.page_cache()
        } else {
            false
        }
    }

    fn is_empty(&self) -> bool {
        !self.is_allocated(None)
    }
}
