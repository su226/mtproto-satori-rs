mod callback_query;
mod new_message;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use constant_time_eq::constant_time_eq;
use grammers_client::Client;
use grammers_client::tl::Serializable;
use grammers_client::update::{Message, Update};
use grammers_session::updates::UpdatesLike;
use log::{debug, trace, warn};
use ntex::rt::spawn;
use ntex::time::sleep;
use ntex::web::ws;
use ntex::ws::Item;
use ntex::ws::error::ProtocolError;
use ntex::{Ctx, Service, rt, web};
use presence_rs::Presence;
use tokio::select;
use tokio::sync::{Mutex as AMutex, broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::convert::login::satori_login_from_tg_user;
use crate::error::AppError;
use crate::satori::types::{Event, WsOp, WsReadyBody};
use crate::self_info_cache::SelfInfoCache;
use crate::settings::Settings;
use crate::state::AppState;
use crate::telegram::is_supported_message;

struct MediaGroup {
    time: Instant,
    messages: Vec<Message>,
}

const BROADCAST_CAPACITY: usize = 100;

pub struct EventPublisher {
    tx: broadcast::Sender<Event>,
    queue: Mutex<VecDeque<Event>>,
    self_info_cache: Arc<AMutex<SelfInfoCache>>,
    settings: Arc<Settings>,
    sn: Mutex<u32>,
    media_groups: Mutex<HashMap<i64, MediaGroup>>,
}

impl EventPublisher {
    pub fn new(self_info_cache: Arc<AMutex<SelfInfoCache>>, settings: Arc<Settings>) -> Self {
        Self {
            tx: broadcast::channel(BROADCAST_CAPACITY).0,
            queue: Mutex::new(VecDeque::with_capacity(settings.recovery_events)),
            self_info_cache,
            settings,
            sn: Mutex::new(0),
            media_groups: Mutex::new(HashMap::new()),
        }
    }

    fn authorize(&self, token: &Presence<String>) -> bool {
        if self.settings.token.is_empty() {
            true
        } else if let Presence::Some(token) = token {
            constant_time_eq(token.as_bytes(), self.settings.token.as_bytes())
        } else {
            false
        }
    }

    fn send(&self, mut event: Event) {
        let mut sn = self.sn.lock().unwrap();
        event.sn = *sn;
        *sn += 1;
        drop(sn);
        let mut queue = self.queue.lock().unwrap();
        if queue.len() >= self.settings.recovery_events {
            queue.pop_front();
        }
        queue.push_back(event.clone());
        drop(queue);
        // Send will fail when no clients.
        let _ = self.tx.send(event);
    }

    async fn handle(self: Arc<Self>, update: Update) {
        match &update {
            Update::NewMessage(message) if is_supported_message(&(**message).raw) => {
                self.handle_new_message(message).await
            }
            Update::CallbackQuery(callback) => self.handle_callback_query(callback).await,
            _ => {
                let mut buf = Vec::new();
                update.raw().serialize(&mut buf);
                debug!(
                    "Received unsupported update: {:x}",
                    u32::from_le_bytes(*buf[..4].as_array().unwrap())
                );
            }
        }
    }

    pub async fn publish(self: Arc<Self>, client: Client, updates: mpsc::Receiver<UpdatesLike>) {
        let mut updates = client
            .stream_updates(updates, Default::default())
            .await
            .expect("Failed to stream updates");
        loop {
            let update = match updates.next().await {
                Ok(update) => update,
                Err(err) => {
                    warn!("Failed to get update: {:?}", err);
                    continue;
                }
            };
            spawn(self.clone().handle(update));
        }
    }

    pub async fn get_events_from(&self, sn: u32) -> Vec<Event> {
        let mut to_send = Vec::new();
        let queue = self.queue.lock().unwrap();
        to_send.extend(queue.iter().filter(|event| event.sn > sn).cloned());
        to_send
    }
}

struct WsService {
    authorizing: Cell<bool>,
    authorized: Rc<Cell<bool>>,
    last_heartbeat: Rc<Cell<Option<Instant>>>,
    buffer: RefCell<Vec<u8>>,
    publisher: Arc<EventPublisher>,
    authorize_cancellation_token: CancellationToken,
    broadcast_cancellation_token: CancellationToken,
}

impl WsService {
    pub fn new(publisher: Arc<EventPublisher>) -> Self {
        Self {
            authorizing: Cell::new(false),
            authorized: Rc::new(Cell::new(false)),
            last_heartbeat: Rc::new(Cell::new(None)),
            buffer: RefCell::new(Vec::new()),
            publisher,
            authorize_cancellation_token: CancellationToken::new(),
            broadcast_cancellation_token: CancellationToken::new(),
        }
    }
}

impl Service<ws::WsSink, ws::Frame> for WsService {
    type Res = Option<ws::Message>;
    type Error = AppError;

    async fn ready(&self, ctx: Ctx<'_, Self, ws::WsSink>) -> Result<(), Self::Error> {
        if !self.authorizing.replace(true) {
            spawn(check_identify(
                ctx.st().clone(),
                self.authorized.clone(),
                self.authorize_cancellation_token.clone(),
            ));
        }
        Ok(())
    }

    async fn call(
        &self,
        frame: ws::Frame,
        ctx: Ctx<'_, Self, ws::WsSink>,
    ) -> Result<Self::Res, Self::Error> {
        let sink = ctx.st();
        let handle_op = async |bytes| -> Result<_, AppError> {
            let op = match serde_json::from_slice::<WsOp>(bytes) {
                Ok(op) => op,
                Err(err) => {
                    warn!("Unable to deserialize payload: {:?}", err);
                    return Ok(None);
                }
            };
            match op {
                WsOp::Identify(identify) => {
                    {
                        if self.authorized.get() {
                            warn!("Multiple IDENTIFY received.");
                            return Ok(None);
                        }
                        if !self.publisher.authorize(&identify.token) {
                            warn!("Invalid WebSocket token received.");
                            sink.send(ws::Message::Close(Some(ws::CloseReason {
                                code: ws::CloseCode::Policy,
                                description: Some("Invalid token.".to_string()),
                            })))
                            .await?;
                            sink.io().close();
                            return Ok(None);
                        }
                        debug!("Successfully authorized WebSocket.");
                        self.authorized.set(true);
                    }
                    self.authorize_cancellation_token.cancel();
                    sink.send(ws::Message::Text(
                        serde_json::to_string(&WsOp::Ready(WsReadyBody {
                            logins: vec![satori_login_from_tg_user(
                                &self.publisher.self_info_cache.lock().await.get().await?,
                            )],
                            proxy_urls: Vec::new(),
                        }))?
                        .into(),
                    ))
                    .await?;
                    rt::spawn(check_heartbeat(
                        sink.clone(),
                        self.last_heartbeat.clone(),
                        self.broadcast_cancellation_token.clone(),
                    ));
                    rt::spawn(send_events(
                        sink.clone(),
                        self.publisher.tx.subscribe(),
                        self.broadcast_cancellation_token.clone(),
                    ));
                    if let Presence::Some(sn) = identify.sn {
                        recovery_events(self.publisher.clone(), sink.clone(), sn).await;
                    }
                    Ok(None)
                }
                WsOp::Ping => {
                    trace!("WebSocket received PING.");
                    self.last_heartbeat.set(Some(Instant::now()));
                    Ok(Some(ws::Message::Text(
                        serde_json::to_string(&WsOp::Pong)?.into(),
                    )))
                }
                _ => {
                    warn!("{:?} should not be received.", op);
                    Ok(None)
                }
            }
        };

        Ok(match frame {
            ws::Frame::Text(bytes) | ws::Frame::Binary(bytes) => handle_op(&bytes).await?,
            ws::Frame::Ping(bytes) => Some(ws::Message::Pong(bytes)),
            ws::Frame::Pong(_) => None,
            ws::Frame::Continuation(item) => match item {
                Item::FirstText(bytes) | Item::FirstBinary(bytes) | Item::Continue(bytes) => {
                    self.buffer.borrow_mut().extend(&bytes);
                    None
                }
                Item::Last(bytes) => {
                    let data = {
                        let mut buffer = self.buffer.borrow_mut();
                        buffer.extend(&bytes);
                        let data = buffer.clone();
                        buffer.clear();
                        data
                    };
                    handle_op(&data).await?
                }
            },
            ws::Frame::Close(_) => Some(ws::Message::Close(None)),
        })
    }

    async fn shutdown(&self, _: Ctx<'_, Self, ws::WsSink>) {
        self.authorize_cancellation_token.cancel();
        self.broadcast_cancellation_token.cancel();
    }
}

pub async fn events(
    state: &AppState,
    _request_state: (),
    req: web::HttpRequest,
) -> web::HttpResponse {
    let service = WsService::new(state.event_publisher.clone());
    if let Err(err) = ws::start(&req, None, service).await {
        warn!("WebSocket error: {:?}", err);
    }
    web::HttpResponse::Ok().build()
}

async fn check_identify(
    sink: ws::WsSink,
    authorized: Rc<Cell<bool>>,
    cancellation_token: CancellationToken,
) -> Result<(), ProtocolError> {
    select! {
        _ = sleep(Duration::from_secs(30)) => {},
        _ = cancellation_token.cancelled() => {},
    }
    if !authorized.get() {
        warn!("IDENTIFY not received.");
        sink.send(ws::Message::Close(Some(ws::CloseReason {
            code: ws::CloseCode::Policy,
            description: Some("IDENTIFY not received.".to_string()),
        })))
        .await?;
        sink.io().close();
    }
    debug!("Stopping check_identify.");
    Ok(())
}

async fn check_heartbeat(
    sink: ws::WsSink,
    last_heartbeat: Rc<Cell<Option<Instant>>>,
    cancellation_token: CancellationToken,
) -> Result<(), ProtocolError> {
    let mut running = true;
    select! {
        _ = sleep(Duration::from_secs(30)) => (),
        _ = cancellation_token.cancelled() => running = false,
    };
    let next_heartbeat_duration = || match last_heartbeat.get() {
        Some(last_heartbeat) => {
            let elapsed = Instant::now() - last_heartbeat;
            let max_elapsed = Duration::from_secs(30);
            if elapsed > max_elapsed {
                None
            } else {
                Some(max_elapsed - elapsed)
            }
        }
        None => None,
    };
    while running {
        let Some(duration) = next_heartbeat_duration() else {
            warn!("PING not received.");
            sink.send(ws::Message::Close(Some(ws::CloseReason {
                code: ws::CloseCode::Policy,
                description: Some("PING not received.".to_string()),
            })))
            .await?;
            sink.io().close();
            break;
        };
        select! {
            _ = sleep(duration) => (),
            _ = cancellation_token.cancelled() => running = false,
        }
    }
    debug!("Stopping check_heartbeat.");
    Ok(())
}

async fn send_events(
    sink: ws::WsSink,
    mut event_rx: broadcast::Receiver<Event>,
    cancellation_token: CancellationToken,
) {
    let mut running = true;
    while running {
        select! {
            _ = async {
                let event = match event_rx.recv().await {
                    Ok(update) => update,
                    Err(err) => {
                        warn!("Receive channel is lagging behind: {:?}", err);
                        return;
                    }
                };
                let serialized = match serde_json::to_string(&WsOp::Event(Box::new(event))) {
                    Ok(data) => data,
                    Err(err) => {
                        warn!("Failed to serialize event: {:?}", err);
                        return;
                    }
                };
                match sink.send(ws::Message::Text(serialized.into())).await {
                    Ok(_) => (),
                    Err(err) => warn!("Failed to send event: {:?}", err),
                }
            } => (),
            _ = cancellation_token.cancelled() => running = false,
        }
    }
    debug!("Stopping send_events.");
}

async fn recovery_events(publisher: Arc<EventPublisher>, sink: ws::WsSink, sn: u32) {
    for event in publisher.get_events_from(sn).await {
        let serialized = match serde_json::to_string(&WsOp::Event(Box::new(event))) {
            Ok(data) => data,
            Err(err) => {
                warn!("Failed to serialize event: {:?}", err);
                return;
            }
        };
        match sink.send(ws::Message::Text(serialized.into())).await {
            Ok(_) => (),
            Err(err) => warn!("Failed to send event: {:?}", err),
        }
    }
}
