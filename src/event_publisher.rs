use crate::{
    convert::{event::satori_event_from_tg_update, login::satori_login_from_tg_user},
    error::MyError,
    satori::types::{Event, WsOp, WsReadyBody},
    self_info_cache::SelfInfoCache,
    settings::Settings,
};
use grammers_client::Client;
use grammers_client::update::Update;
use grammers_session::updates::UpdatesLike;
use log::{debug, trace, warn};
use ntex::time::sleep;
use ntex::web::types::State;
use ntex::web::{self, HttpRequest, HttpResponse, ws};
use ntex::ws::error::ProtocolError;
use ntex::{Service, chain, fn_service, rt};
use ntex::{
    service::{fn_factory_with_config, fn_shutdown, map_config},
    ws::Item,
};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::{
    select,
    sync::{Mutex, broadcast, mpsc},
};

pub struct EventPublisher {
    tx: broadcast::Sender<(Update, Option<Event>)>,
    self_info_cache: Arc<Mutex<SelfInfoCache>>,
    token: String,
}

impl EventPublisher {
    pub fn new(self_info_cache: Arc<Mutex<SelfInfoCache>>, settings: &Settings) -> Self {
        Self {
            tx: broadcast::channel(100).0,
            self_info_cache,
            token: settings.token.clone(),
        }
    }

    fn authorize(&self, token: &Option<String>) -> bool {
        if self.token.is_empty() {
            true
        } else if let Some(token) = token {
            *token == self.token
        } else {
            false
        }
    }

    pub async fn publish(
        &self,
        client: Arc<Client>,
        updates: mpsc::UnboundedReceiver<UpdatesLike>,
    ) {
        let mut updates = client.stream_updates(updates, Default::default()).await;
        let mut sn = 0;
        loop {
            let update = match updates.next().await {
                Ok(update) => update,
                Err(err) => {
                    warn!("Failed to get update: {:?}", err);
                    continue;
                }
            };
            let mut self_info_cache = self.self_info_cache.lock().await;
            let self_id = self_info_cache.get_id().bot_api_dialog_id();
            let login = match self_info_cache.get().await {
                Ok(user) => satori_login_from_tg_user(&user),
                Err(err) => {
                    warn!(
                        "Failed to get login info, event won't be published: {:?}",
                        err
                    );
                    continue;
                }
            };
            drop(self_info_cache);
            let mut event = satori_event_from_tg_update(self_id, &update, login);
            match event {
                Some(ref mut event) => {
                    event.sn = sn;
                    sn += 1;
                    debug!("Received update: {:#?} => {:#?}", update, event);
                }
                None => debug!("Received unsupported update: {:#?}", update),
            }
            // Send will fail when no clients.
            let _ = self.tx.send((update, event));
        }
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
    req: HttpRequest,
    publisher: State<Arc<EventPublisher>>,
) -> Result<HttpResponse, web::Error> {
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
) -> Result<impl Service<ws::Frame, Response = Option<ws::Message>, Error = MyError>, web::Error> {
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

    let service = fn_service(async move |frame| -> Result<Option<ws::Message>, MyError> {
        let handle_op = async |bytes: &[u8]| -> Result<Option<ws::Message>, MyError> {
            let op = match serde_json::from_slice::<WsOp>(&bytes) {
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
    mut event_rx: broadcast::Receiver<(Update, Option<Event>)>,
    mut stop_rx: broadcast::Receiver<()>,
) {
    let mut running = true;
    while running {
        select! {
            _ = async {
                let (_, event) = match event_rx.recv().await {
                    Ok(update) => update,
                    Err(err) => {
                        warn!("Receive channel is lagging behind: {:?}", err);
                        return;
                    }
                };
                let event = match event {
                    Some(event) => event,
                    None => return,
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
