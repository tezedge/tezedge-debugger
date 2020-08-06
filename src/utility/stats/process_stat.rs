// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Serialize, Deserialize};
use std::io;
use super::ProcessStatSource;
use crate::utility::docker::Top;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStat {
    pub cmd: String,
    pub memory_usage: u64,
    pub cpu_usage: f64,
}

impl ProcessStatSource for ProcessStat {
    fn process_cmd(&self) -> &str {
        self.cmd.as_str()
    }

    fn memory_usage(&self) -> u64 {
        self.memory_usage
    }

    fn cpu_usage(&self) -> f64 {
        self.cpu_usage
    }
}

impl ProcessStat {
    pub fn parse_top(top: Top) -> Vec<Self> {
        // the top output look like:
        // {
        //     "Processes":[
        //         ["100","755828","33.1","11.7","41418780","3857544","?","Dsl","14:10","27:18","/usr/local/bin/tezos-node run --data-dir /var/run/tezos/node/data"],
        //         ["100","756042","23.0","0.7","257264","244748","?","Sl","14:10","18:58","tezos-validator"]
        //     ],
        //     "Titles":[
        //         "USER","PID","%CPU","%MEM","VSZ","RSS","TTY","STAT","START","TIME","COMMAND"
        //     ]
        // }

        let mut cpu_column_index = None;
        let mut rss_column_index = None;
        let mut command_column_index = None;

        // find the indices of cpu, rss and command columns
        for (i, title) in top.titles.iter().enumerate() {
            match title.as_str() {
                "%CPU" => cpu_column_index = Some(i),
                "RSS" => rss_column_index = Some(i),
                "COMMAND" => command_column_index = Some(i),
                _ => (),
            }
        }

        top.processes.iter().map(|string_array| {
            let cpu_usage = cpu_column_index
                .and_then(|i| {
                    if i < string_array.len() {
                        string_array[i].parse().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    tracing::warn!(warning = "docker daemon returns malformed `top`");
                    0.0
                });
            let rss = rss_column_index
                .and_then(|i| {
                    if i < string_array.len() {
                        string_array[i].parse().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    tracing::warn!(warning = "docker daemon returns malformed `top`");
                    0
                });
            let cmd = command_column_index
                .and_then(|i| {
                    if i < string_array.len() {
                        string_array.get(i).cloned()
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    tracing::warn!(warning = "docker daemon returns malformed `top`");
                    String::new()
                });
            ProcessStat {
                cmd,
                memory_usage: rss * 4096,
                cpu_usage,
            }
        }).collect()
    }

    pub fn list() -> Result<Vec<Self>, io::Error> {
        use std::fs;

        let v = fs::read_dir("/proc")?
            .filter_map(|dir_entry|
                dir_entry.ok().and_then(|e| e.file_name().to_str().unwrap().parse::<u32>().ok())
            )
            .filter_map(|pid| {
                match Self::read_from_system(pid) {
                    Ok(stat) => Some(stat),
                    Err(err) => {
                        tracing::warn!(
                            warning = tracing::field::display(&err),
                            "failed to read stat",
                        );
                        None
                    },
                }
            })
            .collect();
        Ok(v)
    }

    pub fn read_from_system(pid: u32) -> Result<Self, io::Error> {
        use std::{fs::File, io::Read};

        let read_file = |name| -> Result<String, io::Error> {
            let mut file = File::open(format!("/proc/{}/{}", pid, name))?;
            let mut data = String::new();
            file.read_to_string(&mut data)?;
            Ok(data)
        };

        let vm_rss = read_file("statm")?
            .split(' ')
            .nth(1)
            .ok_or(io::Error::new(io::ErrorKind::InvalidData, "failed to get resident set size"))?
            .parse::<u64>()
            .map_err(|parse_int_err| io::Error::new(io::ErrorKind::InvalidData, parse_int_err))?;
        const PAGE_SIZE: u64 = 4096;

        let cmd = read_file("cmdline")?;
        
        Ok(ProcessStat {
            cmd,
            memory_usage: vm_rss * PAGE_SIZE,
            cpu_usage: 0.0,
        })
    }
}
