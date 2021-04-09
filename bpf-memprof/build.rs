// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

fn main() {
    #[cfg(feature = "facade")]
    self::facade::main()
}

#[cfg(feature = "facade")]
mod facade {
    use std::{
        env,
        path::{Path, PathBuf},
    };
    use cargo_bpf_lib as cargo_bpf;

    pub fn main() {
        let cargo = PathBuf::from(env::var("CARGO").unwrap());
        let target = PathBuf::from(env::var("OUT_DIR").unwrap());
        let module = Path::new(".");

        let kernel_source_dir = env::var("KERNEL_SOURCE")
            .expect("`KERNEL_SOURCE` env var");

        let k = Some(kernel_source_dir.as_ref());
        let p = vec!["kprobe".to_string()];
        cargo_bpf::build_ext(&cargo, &module, &target.join("target"), p, k)
            .expect("couldn't compile module");

        cargo_bpf::probe_files(&module)
            .expect("couldn't list module files")
            .iter()
            .for_each(|file| {
                println!("cargo:rerun-if-changed={}", file);
            });
    }
}
