//! Keeper of recognizers

use crate::{err_msg, Error, Result};
use actix_web::actix::{
    spawn, Actor, ActorFuture, ActorStream, Addr, Context, Handler, Message,
    Supervised, SystemService,
};
use futures::Future;
use lazy_static::lazy_static;
use log;
use luis_sys::recognizer::{EventStream, Recognizer};
use std::collections::HashMap;

lazy_static! {
    pub static ref KEEPER: Addr<Keeper> = { Keeper::from_registry() };
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

#[derive(Default)]
pub struct Keeper {
    table: HashMap<String, Addr<Session>>,
}

impl Actor for Keeper {
    type Context = Context<Self>;
}

impl Supervised for Keeper {}

impl SystemService for Keeper {
    fn service_started(&mut self, _: &mut Context<Self>) {
        log::trace!("Recognizers keeper is started");
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

pub struct Session {
    id: String,
    recognizer: Recognizer,
    stream: EventStream,
}

impl Actor for Session {
    type Context = Context<Self>;
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
