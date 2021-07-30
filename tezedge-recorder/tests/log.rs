// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use tezedge_recorder::tables::node_log;

pub async fn get_log(params: &str) -> Result<Vec<node_log::ItemWithId>, serde_json::error::Error> {
    let debugger = env::var("DEBUGGER_URL")
        .unwrap();
    let res = reqwest::get(&format!("{}/v2/log?node_name=initiator&{}", debugger, params))
        .await.unwrap()
        .text()
        .await.unwrap();
    serde_json::from_str(&res)
}

fn start_time() -> i64 {
    env::var("START_TIME")
        .map(|s| s.parse::<i64>().unwrap_or(0))
        .unwrap_or(0)
}

#[tokio::test]
async fn level() {
    use tezedge_recorder::tables::node_log::LogLevel;

    struct TestCase {
        query: &'static str,
        predicate: fn(LogLevel) -> bool,
    }

    impl TestCase {
        async fn run(&self, forward: bool) {
            let direction = if forward { "direction=forward&cursor=0" } else { "direction=backward" };
            let params = format!("limit=1000&log_level={}&{}", self.query, direction);
            let items = get_log(&params).await.unwrap();
            assert!(!items.is_empty());
            for item in items {
                assert!((self.predicate)(item.level));
            }
        }
    }

    let cases = [
        TestCase { query: "info", predicate: |l| matches!(l, LogLevel::Info) },
        TestCase { query: "warn", predicate: |l| matches!(l, LogLevel::Warning) },
        TestCase { query: "error", predicate: |l| matches!(l, LogLevel::Error) },

        TestCase { query: "warn,error", predicate: |l| matches!(l, LogLevel::Warning | LogLevel::Error) },
        TestCase { query: "error,info", predicate: |l| matches!(l, LogLevel::Error | LogLevel::Info) },
        TestCase { query: "info,warn", predicate: |l| matches!(l, LogLevel::Info | LogLevel::Warning) },
    ];

    for case in &cases {
        case.run(false).await;
        case.run(true).await;
    }
}

#[tokio::test]
async fn pagination() {
    async fn request_cursor(cursor: usize, result: usize) {
        let params = format!("cursor={}&limit=1000", cursor);
        let items = get_log(&params).await.unwrap();
        assert_eq!(items.len(), 1000);
        for (n, item) in items.into_iter().enumerate() {
            assert_eq!((item.id as usize) + n, result);
        }
    }

    let last = get_log("limit=1").await.unwrap();
    let mut last_id = last.last().unwrap().id as usize;

    // deliberate out of range, should give last
    request_cursor(last_id * 2, last_id).await;

    loop {
        request_cursor(last_id, last_id).await;
        if last_id >= 1000 {
            last_id -= 1000;
        } else {
            break;
        }
    }

    request_cursor(1234, 1234).await;
}

#[tokio::test]
async fn timestamp() {
    struct TestCase {
        shift: u64,
        expected: usize,
        forward: bool,
    }

    impl TestCase {
        async fn run(self) {
            let time = (start_time() as u64 + self.shift) * 1000;
            let direction = if self.forward { "forward" } else { "backward" };
            let params = format!("timestamp={}&limit=500&direction={}", time, direction);
            let items = get_log(&params).await.unwrap();
            assert_eq!(items.len(), self.expected);
            let mut time = time;
            for item in items {
                let this = (item.timestamp / 1_000_000) as u64;
                if self.forward {
                    assert!(this > time);
                } else {
                    assert!(this <= time);
                }
                time = this;
            }
        }
    }

    let test_cases = vec![
        TestCase { shift: 321, expected: 322, forward: false },
        TestCase { shift: 321, expected: 500, forward: true },
        TestCase { shift: 6789, expected: 500, forward: false },
        TestCase { shift: 6789, expected: 500, forward: true },
        TestCase { shift: 9876, expected: 500, forward: false },
        TestCase { shift: 9876, expected: 9999 - 9876, forward: true },
    ];

    for test_case in test_cases {
        test_case.run().await;
    }
}

#[tokio::test]
async fn timestamp_and_level() {
    let time = (start_time() as u64 + 3_000) * 1000;
    let params = format!("timestamp={}&limit=500&direction=backward&log_level=warn", time);
    let items = get_log(&params).await.unwrap();
    assert!(!items.is_empty());
    let params = format!("timestamp={}&limit=500&direction=forward&log_level=warn", time);
    let items = get_log(&params).await.unwrap();
    assert!(!items.is_empty());
}

#[tokio::test]
async fn full_text_search() {
    #[derive(Debug)]
    struct TestCase {
        query: &'static str,
        limit: usize,
        has: &'static [&'static str],
        not: &'static [&'static str],
        must: &'static [&'static str],
    }

    impl TestCase {
        async fn run(&self) {
            let items = get_log(&format!("limit={}&query={}&query_no_quotes=true", self.limit, self.query)).await.unwrap();
            assert!(!items.is_empty(), "{:?}", self);
            for item in items {
                if !self.has.is_empty() {
                    assert!(self.has.iter().any(|&has| item.message.contains(has)), "{:?}", self);
                }
                for &not in self.not {
                    assert!(!item.message.contains(not), "{:?}", self);
                }
                for &must in self.must {
                    assert!(item.message.contains(must), "{:?} in {:?}", self, item.message);
                }
            }
        }
    }

    let cases = [
        TestCase { query: "peer", limit: 500, has: &["peer"], not: &[], must: &[] },
        TestCase { query: "peer -branch", limit: 500, has: &["peer"], not: &["branch"], must: &[] },
        TestCase { query: "peer chain -branch", limit: 500, has: &["peer", "chain"], not: &["branch"], must: &[] },
        TestCase { query: "peer -branch -head", limit: 500, has: &["peer"], not: &["branch", "head"], must: &[] },
        TestCase { query: "ip address -peer", limit: 500, has: &["ip", "address"], not: &["peer"], must: &[] },
        // limit is lower here, because no many records meet the condition
        TestCase { query: "+ip +address", limit: 16, has: &[], not: &[], must: &["ip", "address"] },
        TestCase { query: "+head +chain +branch -peer -connection", limit: 4, has: &[], not: &["peer", "connection"], must: &["head", "chain", "branch"] },
    ];

    for case in &cases {
        case.run().await;
    }
}
