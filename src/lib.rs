//! 提供给通信服务器的流媒体AI处理模块。使用微软 LUIS 服务实现语音识别、意向分析。

use std::{ffi::CStr, os::raw::c_char};

pub mod luis;
pub mod web;

/// Common error type of the crate.
pub use failure::{err_msg, Error};
/// Redefine the result of the crate for convenience.
pub type Result<T = (), E = Error> = std::result::Result<T, E>;

use luis::{Frame, KEEPER};

#[no_mangle]
pub unsafe extern "C" fn start_service(conf_file: *const c_char) -> i32 {
    let filename = CStr::from_ptr(conf_file).to_string_lossy();
    match web::start(&filename) {
        Ok(_) => 0,
        Err(err) => {
            log::error!("Failed to start ns_sys service: {}", err);
            -1
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn read_stream(
    _id: *const c_char,
    _buffer: *mut c_char,
    _len: usize,
) -> i32 {
    -1
}

#[no_mangle]
pub unsafe extern "C" fn write_stream(
    id: *const c_char,
    buffer: *const c_char,
    len: usize,
) -> i32 {
    if buffer.is_null() || id.is_null() {
        return -1;
    }

    let sid = CStr::from_ptr(id);
    let sid = match sid.to_str() {
        Ok(sid) => sid,
        Err(err) => {
            log::error!("UTF-8 parse error: {}", err);
            return -1;
        }
    };
    let buf = std::slice::from_raw_parts(buffer as *const u8, len);
    let frame = Frame::new(sid, buf);
    log::trace!("writer stream: {}", sid);
    KEEPER.do_send(frame);
    return len as i32;
}

