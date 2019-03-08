use ns_luis::{luis::*, start_service, Result};
use std::{ffi::CString, thread::spawn};

#[test]
fn simulate() -> Result {
    let conf = CString::new("nsl.toml")?;
    spawn(move || {
        unsafe { start_service(conf.as_ptr()) };
    });

    let _cmd = SessionEvent {
        action: "start".to_owned(),
        sn: "0000000000".to_owned(),
        recordfile: "abc.wav".to_owned(),
        ..Default::default()
    };

    Ok(())
}
