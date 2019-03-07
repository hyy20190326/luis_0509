// build.rs

use std::process::Command;
use std::{env, fs, path::PathBuf};

fn main() {
    if env::var("BUILD_CPP").map(|v| v == "1").unwrap_or(false) == false {
        return
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_dir = out_dir.join("../../../");
    fs::copy("./nsl.toml", out_dir.join("nsl.toml")).unwrap();

    Command::new("g++").args(&["src/bin/main.cpp", "-ldl", "-pthread", "-o"])
                       .arg(out_dir.join("ffi_test"))
                       .status().unwrap();
}
