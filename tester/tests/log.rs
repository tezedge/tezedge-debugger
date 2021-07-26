// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use tezedge_recorder::tables::node_log;
use tester::START_TIME;

pub async fn get_log(params: &str) -> Result<Vec<node_log::ItemWithId>, serde_json::error::Error> {
    let debugger = env::var("DEBUGGER_URL")
        .unwrap();
    let res = reqwest::get(&format!("{}/v2/log?node_name=initiator&{}", debugger, params))
        .await.unwrap()
        .text()
        .await.unwrap();
    serde_json::from_str(&res)
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
            let time = (START_TIME as u64 + self.shift) * 1000;
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
    let time = (START_TIME as u64 + 3_000) * 1000;
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
        has: &'static [&'static str],
        not: &'static [&'static str],
    }

    impl TestCase {
        async fn run(&self) {
            let items = get_log(&format!("limit=500&query={}", self.query)).await.unwrap();
            assert!(!items.is_empty(), "{:?}", self);
            for item in items {
                assert!(self.has.iter().any(|&has| item.message.contains(has)), "{:?}", self);
                for &not in self.not {
                    assert!(!item.message.contains(not), "{:?}", self);
                }
            }
        }
    }

    let cases = [
        TestCase { query: "peer", has: &["peer"], not: &[] },
        TestCase { query: "peer%20-branch", has: &["peer"], not: &["branch"] },
        TestCase { query: "peer%20chain%20-branch", has: &["peer", "chain"], not: &["branch"] },
        TestCase { query: "peer%20-branch%20-head", has: &["peer"], not: &["branch", "head"] },
        TestCase { query: "ip%20address%20-peer", has: &["ip", "address"], not: &["peer"] },
    ];

    for case in &cases {
        case.run().await;
    }
}
