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
        process::{Command, Stdio},
        fs,
        //io::Write,
    };
    use cargo_bpf_lib as cargo_bpf;

    fn prepare_sources(kernel_version: String, target: &Path) -> PathBuf {
        let mut i = kernel_version.split('-');
        let mut i = i.next().unwrap().split('.');
        let major = i.next().unwrap().parse::<u8>().unwrap();
        let minor = i.next().unwrap().parse::<u8>().unwrap();
        let patch = i.next().unwrap_or("0").parse::<u8>().unwrap();
        let version = format!("{}.{}.{}", major, minor, patch);

        let url = format!(
            "https://cdn.kernel.org/pub/linux/kernel/v{}.x/linux-{}.tar.xz",
            major, version,
        );
        let kernel_source_dir = target.join(format!("linux-{}", version));

        if !kernel_source_dir.exists() {
            // download the sources
            Command::new("wget")
                .current_dir(&target)
                .args(&["wget", "-cq"])
                .arg(&url)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
            // decompress
            Command::new("tar")
                .current_dir(&target)
                .arg("-xf")
                .arg(format!("linux-{}.tar.xz", version))
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
            // remove archive
            fs::remove_file(target.join(format!("linux-{}.tar.xz", version))).unwrap();
            // prepare
            Command::new("make")
                .current_dir(&kernel_source_dir)
                .arg("defconfig")
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
            Command::new("make")
                .current_dir(&kernel_source_dir)
                .arg("modules_prepare")
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
                .wait()
                .unwrap();
        }

        kernel_source_dir
    }

    pub fn main() {
        let cargo = PathBuf::from(env::var("CARGO").unwrap());
        let target = PathBuf::from(env::var("OUT_DIR").unwrap());
        let module = Path::new(".");

        let kernel_version = env::var("KERNEL_VERSION").ok();
        let kernel_source_dir = env::var("KERNEL_SOURCE")
            .ok()
            .map(PathBuf::from)
            .or_else(|| kernel_version.map(|v| prepare_sources(v, &target)));

        let k = kernel_source_dir.as_ref().map(AsRef::as_ref);
        cargo_bpf::build_ext(&cargo, &module, &target.join("target"), vec![], k)
            .expect("couldn't compile module");

        /*let m_target = PathBuf::from("..").join(PathBuf::from(env::var("CARGO_TARGET_DIR").unwrap_or("target".to_string())));
        let output = Command::new("llvm-objdump-11")
            .args(&["-d", "--arch=bpf"])
            .arg(target.join("target/bpf/programs/kprobe/kprobe.elf"))
            .output()
            .unwrap();
        fs::File::create(m_target.join("kprobe.dump"))
            .unwrap()
            .write_all(output.stdout.as_ref())
            .unwrap();*/

        cargo_bpf::probe_files(&module)
            .expect("couldn't list module files")
            .iter()
            .for_each(|file| {
                println!("cargo:rerun-if-changed={}", file);
            });
    }
}
