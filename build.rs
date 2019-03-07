// build.rs

use std::process::Command;
use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_dir = out_dir.join("../../../");
    fs::copy("./nsl.toml", out_dir.join("nsl.toml")).unwrap();

    // note that there are a number of downsides to this approach, the comments
    // below detail how to improve the portability of these commands.
    Command::new("g++").args(&["src/bin/main.cpp", "-ldl", "-pthread", "-o"])
                       .arg(out_dir.join("ffi_test"))
                       .status().unwrap();
}
