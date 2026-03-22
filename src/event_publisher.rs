use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use grammers_client::Client;
use grammers_client::update::{Message, Update};
use grammers_session::updates::UpdatesLike;
use log::{debug, trace, warn};
use ntex::rt::spawn;
use ntex::service::{fn_factory_with_config, fn_shutdown, map_config};
use ntex::time::sleep;
use ntex::web::ws;
use ntex::ws::Item;
use ntex::ws::error::ProtocolError;
use ntex::{Service, chain, fn_service, rt, web};
use tokio::select;
use tokio::sync::{Mutex, broadcast, mpsc};

use crate::convert::event::{
    satori_event_from_tg_callback,
    satori_event_from_tg_message,
    satori_event_from_tg_messages,
};
use crate::convert::login::satori_login_from_tg_user;
use crate::error::WebError;
use crate::satori::types::{Event, WsOp, WsReadyBody};
use crate::self_info_cache::SelfInfoCache;
use crate::settings::Settings;

struct MediaGroup {
    time: Instant,
    messages: Vec<Message>,
}

const BROADCAST_CAPACITY: usize = 100;

pub struct EventPublisher {
    tx: broadcast::Sender<Event>,
    queue: Mutex<VecDeque<Event>>,
    self_info_cache: Arc<Mutex<SelfInfoCache>>,
    settings: Arc<Settings>,
    sn: Mutex<u32>,
    media_groups: Mutex<HashMap<i64, MediaGroup>>,
}

impl EventPublisher {
    pub fn new(self_info_cache: Arc<Mutex<SelfInfoCache>>, settings: Arc<Settings>) -> Self {
        Self {
            tx: broadcast::channel(BROADCAST_CAPACITY).0,
            queue: Mutex::new(VecDeque::with_capacity(settings.recovery_events)),
            self_info_cache,
            settings,
            sn: Mutex::new(0),
            media_groups: Mutex::new(HashMap::new()),
        }
    }

    fn authorize(&self, token: &Option<String>) -> bool {
        if self.settings.token.is_empty() {
            true
        } else if let Some(token) = token {
            *token == self.settings.token
        } else {
            false
        }
    }

    async fn send(&self, mut event: Event) {
        let mut sn = self.sn.lock().await;
        event.sn = *sn;
        *sn += 1;
        drop(sn);
        let mut queue = self.queue.lock().await;
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
            Update::NewMessage(message) => {
                if self.settings.merge_media_group.receive > 0
                    && let Some(group_id) = message.grouped_id()
                {
                    let mut media_groups = self.media_groups.lock().await;
                    let mut messages = if let Some(group) = media_groups.remove(&group_id) {
                        group.messages
                    } else {
                        Vec::new()
                    };
                    let now = Instant::now();
                    messages.push(message.clone());
                    media_groups.insert(
                        group_id,
                        MediaGroup {
                            time: now,
                            messages,
                        },
                    );
                    drop(media_groups);
                    sleep(Duration::from_millis(
                        self.settings.merge_media_group.receive,
                    ))
                    .await;
                    let mut media_groups = self.media_groups.lock().await;
                    let group = match media_groups.get(&group_id) {
                        Some(group) => group,
                        None => return,
                    };
                    if group.time != now {
                        return;
                    }
                    let group = match media_groups.remove(&group_id) {
                        Some(group) => group,
                        None => return,
                    };
                    drop(media_groups);
                    let mut messages = group.messages;
                    messages.sort_by_key(|m| m.id());
                    let mut self_info_cache = self.self_info_cache.lock().await;
                    let login = match self_info_cache.get().await {
                        Ok(user) => user,
                        Err(err) => {
                            warn!(
                                "Failed to get login info, event won't be published: {:?}",
                                err
                            );
                            return;
                        }
                    };
                    drop(self_info_cache);
                    let contents = messages.iter().map(|x| &**x).collect::<Vec<_>>();
                    let event = satori_event_from_tg_messages(&login, contents.as_slice());
                    debug!("Received message: {:#?} => {:#?}", messages, event);
                    self.send(event).await;
                } else {
                    let mut self_info_cache = self.self_info_cache.lock().await;
                    let login = match self_info_cache.get().await {
                        Ok(user) => user,
                        Err(err) => {
                            warn!(
                                "Failed to get login info, event won't be published: {:?}",
                                err
                            );
                            return;
                        }
                    };
                    drop(self_info_cache);
                    let event = satori_event_from_tg_message(&login, message);
                    debug!("Received message: {:#?} => {:#?}", message, event);
                    self.send(event).await;
                }
            }
            Update::CallbackQuery(callback) => {
                let mut self_info_cache = self.self_info_cache.lock().await;
                let login = match self_info_cache.get().await {
                    Ok(user) => user,
                    Err(err) => {
                        warn!(
                            "Failed to get login info, event won't be published: {:?}",
                            err
                        );
                        return;
                    }
                };
                drop(self_info_cache);
                if let Err(err) = callback.answer().send().await {
                    warn!("Failed to answer callback query: {:?}", err);
                }
                let message = match callback.load_message().await {
                    Ok(message) => message,
                    Err(err) => {
                        warn!("Failed to get callback query message: {:?}", err);
                        return;
                    }
                };
                let event = satori_event_from_tg_callback(&login, callback, &message);
                debug!("Received callback: {:#?} => {:#?}", callback, event);
                self.send(event).await;
            }
            _ => {
                debug!("Received unsupported update: {:#?}", update);
            }
        }
    }

    pub async fn publish(
        self: Arc<Self>,
        client: Arc<Client>,
        updates: mpsc::UnboundedReceiver<UpdatesLike>,
    ) {
        let mut updates = client.stream_updates(updates, Default::default()).await;
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
        let queue = self.queue.lock().await;
        to_send.extend(queue.iter().filter(|event| event.sn > sn).cloned());
        to_send
    }
}

struct ClientState {
    authorized: bool,
    last_heartbeat: Option<Instant>,
}

impl ClientState {
    fn check_heartbeat(&self) -> Option<Duration> {
        match self.last_heartbeat {
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
        }
    }
}

#[web::get("/v1/events")]
pub async fn events(
    req: web::HttpRequest,
    publisher: web::types::State<Arc<EventPublisher>>,
) -> Result<web::HttpResponse, web::Error> {
    ws::start(
        req,
        None::<&str>,
        map_config(fn_factory_with_config(events_service), move |cfg| {
            (cfg, (*publisher).clone())
        }),
    )
    .await
}

async fn events_service(
    (sink, publisher): (ws::WsSink, Arc<EventPublisher>),
) -> Result<impl Service<ws::Frame, Response = Option<ws::Message>, Error = WebError>, web::Error> {
    let state = Arc::new(Mutex::new(ClientState {
        authorized: false,
        last_heartbeat: None,
    }));

    let (init_stop_tx1, init_stop_rx) = mpsc::channel(1);
    let init_stop_tx2 = init_stop_tx1.clone();
    let (loop_stop_tx1, _) = broadcast::channel(1);
    let loop_stop_tx2 = loop_stop_tx1.clone();
    rt::spawn(check_identify(sink.clone(), state.clone(), init_stop_rx));

    let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    let service = fn_service(async move |frame| -> Result<_, WebError> {
        let handle_op = async |bytes| -> Result<_, WebError> {
            let op = match serde_json::from_slice::<WsOp>(bytes) {
                Ok(op) => op,
                Err(err) => {
                    warn!("Unable to deserialize payload: {:?}", err);
                    return Ok(None);
                }
            };
            Ok(match op {
                WsOp::Identify(identify) => {
                    let mut state1 = state.lock().await;
                    if state1.authorized {
                        warn!("Multiple IDENTIFY received.");
                        None
                    } else if publisher.authorize(&identify.token) {
                        debug!("Successfully authorized WebSocket.");
                        state1.authorized = true;
                        let _ = init_stop_tx1.send(()).await;
                        sink.send(ws::Message::Text(
                            serde_json::to_string(&WsOp::Ready(WsReadyBody {
                                logins: vec![satori_login_from_tg_user(
                                    &publisher.self_info_cache.lock().await.get().await?,
                                )],
                                proxy_urls: Vec::new(),
                            }))?
                            .into(),
                        ))
                        .await?;
                        rt::spawn(check_heartbeat(
                            sink.clone(),
                            state.clone(),
                            loop_stop_tx1.subscribe(),
                        ));
                        rt::spawn(send_events(
                            sink.clone(),
                            publisher.tx.subscribe(),
                            loop_stop_tx1.subscribe(),
                        ));
                        if let Some(sn) = identify.sn {
                            recovery_events(publisher.clone(), sink.clone(), sn).await;
                        }
                        None
                    } else {
                        warn!("Invalid WebSocket token received.");
                        Some(ws::Message::Close(Some(ws::CloseReason {
                            code: ws::CloseCode::Policy,
                            description: Some("Invalievent_rxd token.".to_string()),
                        })))
                    }
                }
                WsOp::Ping => {
                    trace!("WebSocket received PING.");
                    state.lock().await.last_heartbeat = Some(Instant::now());
                    Some(ws::Message::Text(
                        serde_json::to_string(&WsOp::Pong)?.into(),
                    ))
                }
                _ => {
                    warn!("{:?} should not be received.", op);
                    None
                }
            })
        };

        Ok(match frame {
            ws::Frame::Text(bytes) | ws::Frame::Binary(bytes) => handle_op(&bytes).await?,
            ws::Frame::Ping(bytes) => Some(ws::Message::Pong(bytes)),
            ws::Frame::Pong(_) => None,
            ws::Frame::Continuation(item) => match item {
                Item::FirstText(bytes) | Item::FirstBinary(bytes) | Item::Continue(bytes) => {
                    buffer.lock().await.extend(&bytes);
                    None
                }
                Item::Last(bytes) => {
                    let mut buffer = buffer.lock().await;
                    buffer.extend(&bytes);
                    let message = handle_op(&buffer).await?;
                    buffer.clear();
                    message
                }
            },
            ws::Frame::Close(_) => Some(ws::Message::Close(None)),
        })
    });

    let on_shutdown = fn_shutdown(async move || {
        let _ = init_stop_tx2.send(()).await;
        let _ = loop_stop_tx2.send(());
    });

    Ok(chain(service).and_then(on_shutdown))
}

async fn check_identify(
    sink: ws::WsSink,
    state: Arc<Mutex<ClientState>>,
    mut stop_rx: mpsc::Receiver<()>,
) -> Result<(), ProtocolError> {
    select! {
        _ = sleep(Duration::from_secs(30)) => {},
        _ = stop_rx.recv() => {},
    }
    if !state.lock().await.authorized {
        warn!("IDENTIFY not received.");
        sink.send(ws::Message::Close(Some(ws::CloseReason {
            code: ws::CloseCode::Policy,
            description: Some("IDENTIFY not received.".to_string()),
        })))
        .await?;
    }
    debug!("Stopping check_identify.");
    Ok(())
}

async fn check_heartbeat(
    sink: ws::WsSink,
    state: Arc<Mutex<ClientState>>,
    mut stop_rx: broadcast::Receiver<()>,
) -> Result<(), ProtocolError> {
    let mut running = true;
    select! {
        _ = sleep(Duration::from_secs(30)) => (),
        _ = stop_rx.recv() => running = false,
    };
    while running {
        let Some(duration) = state.lock().await.check_heartbeat() else {
            warn!("PING not received.");
            sink.send(ws::Message::Close(Some(ws::CloseReason {
                code: ws::CloseCode::Policy,
                description: Some("PING not received.".to_string()),
            })))
            .await?;
            break;
        };
        select! {
            _ = sleep(duration) => (),
            _ = stop_rx.recv() => running = false,
        }
    }
    debug!("Stopping check_heartbeat.");
    Ok(())
}

async fn send_events(
    sink: ws::WsSink,
    mut event_rx: broadcast::Receiver<Event>,
    mut stop_rx: broadcast::Receiver<()>,
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
                let serialized = match serde_json::to_string(&WsOp::Event(event)) {
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
            _ = stop_rx.recv() => running = false,
        }
    }
    debug!("Stopping send_events.");
}

async fn recovery_events(publisher: Arc<EventPublisher>, sink: ws::WsSink, sn: u32) {
    for event in publisher.get_events_from(sn).await {
        let serialized = match serde_json::to_string(&WsOp::Event(event)) {
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
