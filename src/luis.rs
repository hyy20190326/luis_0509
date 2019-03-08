//! Keeper of luis recognizers

use crate::{err_msg, web::Settings, Result};
use actix_web::{
    actix::{
        fut, Actor, ActorContext, ActorStream, Addr, Context,
        ContextFutureSpawner, Handler, Message, Supervised, SystemService,
        WrapFuture, WrapStream,
    },
    client as httpc, HttpMessage,
};
use futures::Future;
use lazy_static::lazy_static;
use log;
use luis_sys::speech::{
    AsrResult, Event, EventResult, Flags, RecognitionResult, Recognizer,
    RecognizerConfig, Session as _Session,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, sync::Arc, time::Duration};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

lazy_static! {
    pub(crate) static ref KEEPER: Addr<Keeper> = { Keeper::from_registry() };
}

pub struct Frame {
    sid: String,
    data: Vec<u8>,
}

impl Frame {
    pub fn new(sid: &str, data: &[u8]) -> Self {
        let sid = sid.to_owned();
        let data = data.to_owned();
        Frame { sid, data }
    }
}

impl Message for Frame {
    type Result = Result;
}

pub struct Initialize(pub Arc<Settings>);

impl Message for Initialize {
    type Result = Result;
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SessionEvent {
    pub action: String,
    pub recordfile: String,
    pub sn: String,
    pub client: String,
    pub serverip: String,
    pub from: String,
    pub asrserver: String,
    pub callbackurl: String,
}

impl fmt::Display for SessionEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ sn: {}, action: {}, asr server: {} }}",
            self.sn, self.action, self.asrserver
        )
    }
}

impl Message for SessionEvent {
    type Result = Result;
}

#[derive(Default)]
pub struct Keeper {
    table: HashMap<String, Addr<Session>>,
    builder: Option<RecognizerConfig>,
    settings: Option<Arc<Settings>>,
}

impl Actor for Keeper {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("KEEPER is started!");
    }
}

impl Supervised for Keeper {}

impl SystemService for Keeper {
    fn service_started(&mut self, _: &mut Context<Self>) {
        log::trace!("Recognizers keeper is started");
    }
}

impl Handler<Initialize> for Keeper {
    type Result = Result;

    fn handle(
        &mut self,
        cfg: Initialize,
        _: &mut Context<Self>,
    ) -> Self::Result {
        self.settings = Some(Arc::clone(&cfg.0));

        let c = &cfg.0.luis;
        let mut builder =
            RecognizerConfig::from_subscription(&c.subscription, &c.region)?;

        builder
            .set_flags(Flags::Recognized | Flags::SpeechDetection)
            .put_language(c.language.as_str())?
            .set_audio(cfg.0.audio)
            .set_model_id(c.intent_model.as_str())
            .set_intents(c.intents.as_ref());
        self.builder = Some(builder);
        Ok(())
    }
}

impl Handler<Frame> for Keeper {
    type Result = Result;

    fn handle(&mut self, frame: Frame, _: &mut Context<Self>) -> Self::Result {
        self.table
            .get(&frame.sid)
            .ok_or_else(|| err_msg("session is not found"))?
            .do_send(frame);
        Ok(())
    }
}

impl Handler<SessionEvent> for Keeper {
    type Result = Result;

    fn handle(
        &mut self,
        cmd: SessionEvent,
        _: &mut Context<Self>,
    ) -> Self::Result {
        match cmd.action.as_str() {
            "stop" => {
                log::debug!("Session {} is stopped.", cmd.sn);
                if let Some(session) = self.table.remove(&cmd.sn) {
                    session.do_send(StopSession);
                    Ok(())
                } else {
                    Err(err_msg(format!("session {} is not found", cmd.sn)))
                }
            }
            "start" => {
                if let Some(ref builder) = self.builder {
                    // let recognizer = builder.intent_recognizer()?;
                    let recognizer = builder.recognizer()?;
                    let sn = cmd.sn.clone();
                    let payload = cmd;
                    let session = Session {
                        recognizer,
                        payload,
                        settings: Arc::clone(self.settings.as_ref().unwrap()),
                    };
                    let session = session.start();
                    self.table.insert(sn, session);
                    Ok(())
                } else {
                    Err(err_msg("Keeper is not initialized."))
                }
            }
            _ => Err(err_msg(format!("Unknown session event: {}", cmd))),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LuisEvent {
    pub timestamp: u64,
    pub group_id: usize,
    pub session: String,
    pub event: String,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intention_desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_consume_sequence_id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errormsg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errorcode: Option<isize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_sequence: Option<usize>,
}

pub struct StopSession;
impl Message for StopSession {
    type Result = ();
}

pub struct Session {
    recognizer: Recognizer,
    payload: SessionEvent,
    settings: Arc<Settings>,
}

impl Actor for Session {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        log::debug!("session {} is started.", self.payload.sn);
        let es = match self.recognizer.start() {
            Ok(stream) => stream,
            Err(err) => {
                log::error!("Failed to start recognition session: {}", err);
                ctx.stop();
                return;
            }
        };
        es.into_actor(self)
            .and_then(|evt, a, c| match handle_event_stream(evt, a, c) {
                Ok(report) => fut::ok(report),
                Err(err) => {
                    log::error!("Something wrong: {}", err);
                    fut::err(())
                }
            })
            .finish()
            .spawn(ctx);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        log::trace!("session {} is stopped.", self.payload.sn);
        let _ = self.recognizer.close_stream();
    }
}

impl Handler<Frame> for Session {
    type Result = Result;

    fn handle(
        &mut self,
        mut frame: Frame,
        _: &mut Context<Self>,
    ) -> Self::Result {
        self.recognizer.write_stream(&mut frame.data)?;
        Ok(())
    }
}

impl Handler<StopSession> for Session {
    type Result = ();
    fn handle(&mut self, _: StopSession, ctx: &mut Context<Self>) {
        ctx.stop();
    }
}

fn handle_event_stream(
    evt: Event,
    actor: &mut Session,
    ctx: &mut Context<Session>,
) -> Result {
    let mut se = LuisEvent {
        timestamp: chrono::Local::now().timestamp() as u64,
        group_id: 0,
        session: actor.payload.sn.clone(),
        app_id: "1500000615".to_owned(),
        ..Default::default()
    };
    let flag = evt.flag();
    log::debug!("luis event fired: {:?}", flag);
    if flag.intersects(Flags::SpeechStartDetected) {
        se.event = "session_start".to_owned();
    } else if flag.intersects(Flags::SpeechEndDetected) {
        se.event = "session_end".to_owned();
    } else if flag.intersects(Flags::Recognized) {
        let er = EventResult::from_event(evt)?;
        let reason = er.reason();
        se.event = "session_nlp_event".to_owned();
        if reason.intersects(Flags::NoMatch) {
            se.intention = Some(String::new());
        } else {
            se.intention = Some(er.intent()?);
        }
        if let Ok(detail) = er.details() {
            if detail.is_object() {
                se.confidence = detail["topScoringIntent"]["score"].as_f64();
                se.intention_desc = detail["topScoringIntent"]["intent"]
                    .as_str()
                    .map(|i| i.to_owned());
            }
        }
        se.text = Some(er.text()?);
        se.echo = Some(actor.payload.recordfile.clone());
    } else {
        return Err(err_msg("unknown event type"));
    }

    let url = &actor.settings.notify_prefix;
    log::debug!("Notify started: {}: {:?}", url, serde_json::to_string(&se));
    // body should be consumed for connection keep alive.
    httpc::post(url)
        .header("Authorization", actor.settings.auth_key.as_str())
        .timeout(REQUEST_TIMEOUT)
        .json(se)
        .map_err(|err| {
            log::error!("Failed to push data: {}", err);
            err_msg("Failed to push data.")
        })?
        .send()
        .map_err(|err| log::error!("Push data error: {}", err))
        .and_then(|resp| resp.body().map_err(|err| log::error!("{}", err)))
        .and_then(|_| Ok(()))
        .into_actor(actor)
        .spawn(ctx);
    Ok(())
}
