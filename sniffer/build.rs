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

        cargo_bpf::build(&cargo, &module, &target.join("target"), Vec::new())
            .expect("couldn't compile module");

        cargo_bpf::probe_files(&module)
            .expect("couldn't list module files")
            .iter()
            .for_each(|file| {
                println!("cargo:rerun-if-changed={}", file);
            });
    }
}
