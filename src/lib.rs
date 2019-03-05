//! 提供给通信服务器的流媒体AI处理模块。使用微软 LUIS 服务实现语音识别、意向分析。

use actix_web::actix::System;
use config;
use flexi_logger::{opt_format, Cleanup, Logger};
use log;
use serde::Deserialize;
use std::{env, ffi::CStr, os::raw::c_char, sync::Arc};

mod keeper;
/// Common error type of the crate.
pub use failure::{err_msg, Error};
use keeper::{Frame, KEEPER};
/// Redefine the result of the crate for convenience.
pub type Result<T = (), E = Error> = std::result::Result<T, E>;

#[no_mangle]
pub unsafe extern "C" fn start_hss(conf_file: *const c_char) -> i32 {
    let filename = CStr::from_ptr(conf_file).to_string_lossy();
    match start(&filename) {
        Ok(_) => 0,
        Err(err) => {
            log::error!("Failed to start ns_sys service: {}", err);
            -1
        }
    }
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
    KEEPER.do_send(frame);
    return len as i32;
}

pub fn start(filename: &str) -> Result {
    let cfg = Arc::new(get_config(filename)?);
    let settings = Arc::clone(&cfg);
    // 是否启用调试模式。
    if cfg.debug {
        env::set_var("RUST_BACKTRACE", "1");
    }

    // 启动日志系统。
    let logger = Logger::with_str(&cfg.log)
        .format(opt_format)
        .rotate(cfg.log_rotate_size, Cleanup::Never);
    let logger = if !cfg.log_folder.is_empty() {
        logger.log_to_file().directory(cfg.log_folder.as_str())
    } else {
        logger
    };
    logger.start()?;
    let system = System::new(cfg.name.clone());

    // 此后写入日志文件。
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    log::info!("Http stream server v{} is running.", VERSION);
    start_web(settings)?;
    system.run();
    log::info!("Http stream server is stopped.");

    Ok(())
}

fn get_config(filename: &str) -> Result<Settings> {
    let mut settings = config::Config::new();
    settings.merge(config::File::with_name(filename))?;
    Ok(settings.try_into()?)
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Settings {
    name: String,
    debug: bool,
    log: String,
    log_folder: String,
    log_rotate_size: usize,
    endpoint: String,
    web_prefix: String,
    asr_prefix: String,
    notify_prefix: String,
    file_prefix: String,
    test_prefix: String,
    max_json_size: usize,
    app_id: String,
    auth_key: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: "Neunit Http Stream Server".to_owned(),
            debug: false,
            log: "hss=info".to_owned(),
            log_folder: String::new(),
            log_rotate_size: 10_000_000,  // 10MB
            endpoint: "127.0.0.1:8059".to_owned(),
            web_prefix: "xlp/receive_voice_stream/v1".to_owned(),
            asr_prefix: "/xlp/short_voice_silence_server".to_owned(),
            notify_prefix: "http://127.0.0.1:8059/xlp/ai_robot/v1?action=streamplay&from=zhuiyi&streamName=1&serialNo=".to_owned(),
            file_prefix: String::new(),
            test_prefix: "xlp/ai_robot/v1".to_owned(),
            // data size of 200 seconds voice block.
            max_json_size: 320*20*10_000,
            // Application ID for outgong stream
            app_id: "1500000615".to_string(),
            auth_key: "05e31d5bbdcd81dfb4ad13b03f2cd28c5708b3e784b6a15d15777fc7f584d29a".to_string(),
        }
    }
}

fn start_web(conf: Arc<Settings>) -> Result {
    Ok(())
}
