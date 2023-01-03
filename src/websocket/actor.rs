use std::time::{Duration, Instant};

use crate::comm_async;
use actix::prelude::*;
use actix::{Actor, AsyncContext, SpawnHandle, StreamHandler};
use actix_web_actors::ws;
use async_stream;
use tokio::sync::mpsc::{channel, Receiver as TokioReceiver};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct WebSocketActor {
    last_heartbeat: Instant,
    serial_comm: comm_async::SerialComm,
    response_receiver: Option<TokioReceiver<String>>,
    spawn_handle: Option<SpawnHandle>,
}

impl WebSocketActor {
    pub fn new(tty: &String) -> Self {
        let (sender, response_receiver) = channel::<String>(42);
        let serial_comm = comm_async::SerialComm::new(tty, 115200, sender);
        Self {
            last_heartbeat: Instant::now(),
            serial_comm,
            response_receiver: Some(response_receiver),
            spawn_handle: None,
        }
    }

    /// helper method that sends ping to client every 5 seconds (HEARTBEAT_INTERVAL).
    ///
    /// also this method checks heartbeats from client
    fn schedule_heartbeat(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.last_heartbeat) > CLIENT_TIMEOUT {
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

// https://github.com/actix/actix-web/discussions/2758
impl Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start. We start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.schedule_heartbeat(ctx);
        let mut receiver = self.response_receiver.take().unwrap();
        self.spawn_handle = Some(ctx.add_stream(async_stream::stream! {
            loop {
                let resp = receiver.recv().await;
                if resp.is_some() {
                    yield resp.unwrap()
                } else {
                    break;
                }

            }
        }));
    }
}

/// Handler for `ws::Message`
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.last_heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.last_heartbeat = Instant::now();
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Text(msg)) => {
                let sender = self.serial_comm.get_sender();
                let fut = Box::pin(async move {
                    sender.send(msg.to_string()).await.unwrap();
                });
                let actor_future = fut.into_actor(self);
                ctx.spawn(actor_future);
            }
            _ => ctx.stop(),
        }
    }
}

impl StreamHandler<String> for WebSocketActor {
    fn handle(&mut self, msg: String, ctx: &mut Self::Context) {
        ctx.text(msg);
    }
}
