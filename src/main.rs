mod authorization;
mod convert;
mod error;
mod event_publisher;
mod routes;
mod satori;
mod self_info_cache;
mod session;
mod settings;
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
use tokio::sync::Mutex;

use crate::authorization::SatoriAuthorization;
use crate::event_publisher::{EventPublisher, events};
use crate::routes::internal::internal;
use crate::routes::login_get::login_get;
use crate::routes::message_create::message_create;
use crate::routes::message_get::message_get;
use crate::routes::message_update::message_update;
use crate::routes::user_channel_create::user_channel_create;
use crate::routes::user_get::user_get;
use crate::self_info_cache::SelfInfoCache;
use crate::session::SessionName;
use crate::settings::Settings;

#[ntex::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let settings = Arc::new(Settings::read().unwrap());
    let is_user = !settings.phone.is_empty();
    let is_bot = !settings.bot_token.is_empty();

    if !is_user && !is_bot {
        panic!("Either phone or bot_token is required.");
    }

    let session_name = if is_user {
        // Can phone contain underscores or spaces? Can phone omit leading plus? Maybe normalize it?
        format!("user_{}", settings.phone)
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
        SqliteSession::open(format!("session_{}.db", session_name))
            .await
            .expect("Failed to open session database"),
    );
    if settings.test_mode {
        session
            .set_dc_option(&DcOption {
                id: 2,
                ipv4: SocketAddrV4::from_str("149.154.167.40:80").unwrap(),
                ipv6: SocketAddrV6::from_str("[2001:67c:4e8:f002::e]:80").unwrap(),
                auth_key: None,
            })
            .await;
    }
    let params = ConnectionParams {
        proxy_url: if settings.proxy.is_empty() {
            None
        } else {
            Some(settings.proxy.clone())
        },
        ..Default::default()
    };
    let pool = SenderPool::with_configuration(session.clone(), settings.api_id, params);
    let client = Arc::new(Client::new(pool.handle));

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
    let session_name = Arc::new(SessionName(session_name));

    spawn(
        event_publisher
            .clone()
            .publish(client.clone(), pool.updates),
    );

    let bind = &settings.clone().bind;

    web::HttpServer::new(async move || {
        let path = if settings.path.is_empty() {
            "/"
        } else {
            &settings.path
        };
        web::App::new()
            .state(settings.clone())
            .state(client.clone())
            .state(self_info_cache.clone())
            .state(event_publisher.clone())
            .state(session_name.clone())
            .state(web::types::JsonConfig::default().limit(settings.json_size_limit))
            .service(
                web::scope(path)
                    .middleware(SatoriAuthorization)
                    .service(internal)
                    .service(login_get)
                    .service(message_create)
                    .service(message_get)
                    .service(message_update)
                    .service(user_channel_create)
                    .service(user_get)
                    .service(events),
            )
    })
    .bind(bind)?
    .run()
    .await
}
