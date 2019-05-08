use crate::{
    luis::{Initialize, SessionEvent, KEEPER},
    Error, Result,
};
use actix_web::{
    actix::System, http, middleware, server, App, AsyncResponder, HttpResponse,
    Query,
};
use config;
use flexi_logger::{opt_format, Cleanup, Logger};
use futures::Future;
use log;
use luis_sys::speech::AudioConfig;
use serde::{Deserialize, Serialize};
use std::{env, fmt, sync::Arc};

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub name: String,
    pub debug: bool,
    pub log: String,
    pub log_folder: String,
    pub log_rotate_size: usize,
    pub endpoint: String,
    pub web_prefix: String,
    pub asr_prefix: String,
    pub offline_asr_prefix: String,
    pub notify_prefix: String,
    pub file_prefix: String,
    pub test_prefix: String,
    pub max_json_size: usize,
    pub app_id: String,
    pub auth_key: String,
    pub luis: LuisConfig,
    pub audio: AudioConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            name: "Neunit Speech AI Server".to_owned(),
            debug: false,
            log: "hss=info".to_owned(),
            log_folder: String::new(),
            log_rotate_size: 10_000_000,  // 10MB
            endpoint: "127.0.0.1:8059".to_owned(),
            web_prefix: "xlp/receive_voice_stream/v1".to_owned(),
            asr_prefix: "/xlp/short_voice_silence_server".to_owned(),
            offline_asr_prefix: "xlp/offline_tencent_asr".to_owned(),
            notify_prefix: "http://127.0.0.1:8059/xlp/ai_robot/v1?action=streamplay&from=zhuiyi&streamName=1&serialNo=".to_owned(),
            file_prefix: String::new(),
            test_prefix: "xlp/ai_robot/v1".to_owned(),
            // data size of 200 seconds voice block.
            max_json_size: 320*20*10_000,
            // Application ID for outgong stream
            app_id: "1500000615".to_string(),
            auth_key: "05e31d5bbdcd81dfb4ad13b03f2cd28c5708b3e784b6a15d15777fc7f584d29a".to_string(),
            luis: LuisConfig::default(),
            audio: AudioConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct LuisConfig {
    pub subscription: String,
    pub region: String,
    pub language: String,
    pub intent_model: String,
    pub intents: Vec<String>,
}

impl Default for LuisConfig {
    fn default() -> Self {
        LuisConfig {
            subscription: String::new(),
            region: String::from("eastasia"),
            language: String::from("zh-CN"),
            intent_model: String::new(),
            intents: Vec::new(),
        }
    }
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

fn start_web(conf: Arc<Settings>) -> Result {
    let cfg = conf.clone();
    server::new(move || {
        log::info!("Start web server.");
        App::new()
            .middleware(middleware::Logger::default())
            .resource(&conf.asr_prefix, |r| {
                r.method(http::Method::GET).with_async(on_session_event)
            })
    })
    .bind(&cfg.endpoint)
    .unwrap()
    .start();
    KEEPER.do_send(Initialize(cfg));
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonResult {
    result: usize,
    msg: String,
}

impl JsonResult {
    fn success() -> HttpResponse {
        let jr = JsonResult {
            result: 0,
            msg: "success".to_string(),
        };
        HttpResponse::Ok().json(jr)
    }

    fn error(err: impl fmt::Display) -> HttpResponse {
        log::error!("Response error: {}", err);
        let jr = JsonResult {
            result: 1,
            msg: err.to_string(),
        };
        HttpResponse::InternalServerError().json(jr)
    }
}

fn on_session_event(
    info: Query<SessionEvent>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    log::trace!("Received session event: {:?}", info);
    KEEPER
        .send(info.into_inner())
        .then(|res| match res {
            Ok(Ok(())) => Ok(JsonResult::success()),
            Ok(Err(err)) => Ok(JsonResult::error(err)),
            Err(err) => Ok(JsonResult::error(err)),
        })
        .responder()
}

