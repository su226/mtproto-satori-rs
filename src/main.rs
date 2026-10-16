mod authorization;
mod convert;
mod error;
mod event_publisher;
mod routes;
mod satori;
mod self_info_cache;
mod settings;
mod state;
mod telegram;

use std::io;
use std::net::{SocketAddrV4, SocketAddrV6};
use std::str::FromStr;
use std::sync::Arc;

use grammers_client::sender::ConnectionParams;
use grammers_client::session::storages::SqliteSession;
use grammers_client::{Client, SenderPool, SignInError};
use grammers_session::Session;
use grammers_session::types::DcOption;
use log::debug;
use ntex::rt::spawn;
use ntex::web;
use regex::Regex;
use tokio::sync::Mutex;

use crate::authorization::SatoriAuthorization;
use crate::event_publisher::{EventPublisher, events};
use crate::routes::internal::internal;
use crate::routes::login_get::login_get;
use crate::routes::message_create::message_create;
use crate::routes::message_get::message_get;
use crate::routes::message_update::message_update;
use crate::routes::upload_create::{clean_uploads, upload_create};
use crate::routes::user_channel_create::user_channel_create;
use crate::routes::user_get::user_get;
use crate::self_info_cache::SelfInfoCache;
use crate::settings::Settings;
use crate::state::{AppStateBuilder, SessionName};

async fn is_fresh_login(session: &SqliteSession) -> bool {
    let updates_state = session
        .updates_state()
        .await
        .expect("Failed to read session");
    updates_state.date == 0
}

#[ntex::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let settings = Arc::new(Settings::read().unwrap());
    let is_user = !settings.phone.is_empty();
    let is_bot = !settings.bot_token.is_empty();

    if (!is_user && !is_bot) || (is_user && is_bot) {
        panic!("Either phone or bot_token is required (but not both).");
    }

    let session_name = if is_user {
        format!(
            "user_{}",
            Regex::new(r"[+()\s_-]")
                .unwrap()
                .replace_all(&settings.phone, "")
        )
    } else {
        format!(
            "bot_{}",
            settings
                .bot_token
                .split(":")
                .next()
                .expect("Invalid bot token")
        )
    };
    let session_name = if settings.test_mode {
        format!("test_{}", session_name)
    } else {
        session_name
    };

    let session = Arc::new(
        SqliteSession::open(&format!("session_{}.db", session_name))
            .await
            .expect("Failed to open session database"),
    );
    if settings.test_mode && is_fresh_login(&session).await {
        session
            .set_dc_option(&DcOption {
                id: 2,
                ipv4: SocketAddrV4::from_str("149.154.167.40:80").unwrap(),
                ipv6: SocketAddrV6::from_str("[2001:67c:4e8:f002::e]:80").unwrap(),
                auth_key: None,
            })
            .await
            .expect("Failed to enable test mode");
    }
    let params = ConnectionParams {
        proxy_url: if settings.proxy.is_empty() {
            None
        } else {
            Some(settings.proxy.clone())
        },
        ..Default::default()
    };
    let pool = SenderPool::with_configuration(session, settings.api_id, params);
    let client = Client::new(pool.handle);

    debug!("Logging in.");

    spawn(pool.runner.run());

    let user = if client
        .is_authorized()
        .await
        .expect("Check login status failed")
    {
        client.get_me().await.expect("Get login info failed.")
    } else if is_user {
        let token = client
            .request_login_code(&settings.phone, &settings.api_hash)
            .await
            .expect("Request login code failed");
        print!("Input login code: ");
        let mut code = String::new();
        io::stdin().read_line(&mut code)?;
        match client.sign_in(&token, &code).await {
            Err(SignInError::PasswordRequired(password_token)) => client
                .check_password(password_token, &settings.password)
                .await
                .expect("Check password failed"),
            Err(err) => panic!("Login failed: {err:?}"),
            Ok(user) => user,
        }
    } else {
        client
            .bot_sign_in(&settings.bot_token, &settings.api_hash)
            .await
            .expect("Login failed")
    };

    debug!("Logged in as: {} ({})", user.full_name(), user.id());

    let self_info_cache = Arc::new(Mutex::new(SelfInfoCache::new(user, client.clone())));
    let event_publisher = Arc::new(EventPublisher::new(
        self_info_cache.clone(),
        settings.clone(),
    ));
    let session_name = Arc::new(SessionName::new(session_name));

    spawn(
        event_publisher
            .clone()
            .publish(client.clone(), pool.updates),
    );
    spawn(clean_uploads(session_name.clone()));

    let bind = &settings.clone().bind;
    let state = AppStateBuilder::new(
        settings.clone(),
        client,
        self_info_cache,
        event_publisher,
        session_name,
    );

    web::server_with_config(state, async move |_| {
        let path = if settings.path.is_empty() {
            "/"
        } else {
            &settings.path
        };
        let cfg = web::WebAppConfig::new()
            .set_state(web::types::JsonConfig::default().limit(settings.json_size_limit));
        let scope = web::scope(path)
            .middleware(SatoriAuthorization)
            .route(
                "/v1/proxy/internal:telegram/{user_id}/{path}",
                web::get().to_with_state(internal),
            )
            .route("/v1/login.get", web::post().to_with_state(login_get))
            .route(
                "/v1/message.create",
                web::post().to_with_state(message_create),
            )
            .route("/v1/message.get", web::post().to_with_state(message_get))
            .route(
                "/v1/message.update",
                web::post().to_with_state(message_update),
            )
            .route(
                "/v1/upload.create",
                web::post().to_with_state(upload_create),
            )
            .route(
                "/v1/user.channel.create",
                web::post().to_with_state(user_channel_create),
            )
            .route("/v1/user.get", web::post().to_with_state(user_get))
            .route("/v1/events", web::get().to_with_state(events));
        web::App::new().with_config(cfg).service(scope)
    })
    .bind(bind, ntex::SharedCfg::default())?
    .run()
    .await
}
