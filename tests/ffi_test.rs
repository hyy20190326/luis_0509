use ns_luis::{start_service, Result, luis::*};
use std::{
    ffi::CString,
    thread::{sleep, spawn},
    time::Duration,
};
use serde_json;

#[test]
fn simulate() -> Result {
    let conf = CString::new("nsl.toml")?;
    spawn(move || {
        unsafe { start_service(conf.as_ptr()) };
    });

    let cmd = SessionEvent {
        action: "start".to_owned(),
        sn: "0000000000".to_owned(),
        recordfile: "abc.wav".to_owned(),
        ..Default::default()
    };

    Ok(())
}
