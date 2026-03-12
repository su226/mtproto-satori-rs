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

use std::{io, sync::Arc};

use grammers_client::{
    Client, SenderPool, SignInError, sender::ConnectionParams, session::storages::SqliteSession,
};
use log::debug;
use ntex::web::{self, types::JsonConfig};
use tokio::sync::Mutex;

use crate::{
    authorization::SatoriAuthorization,
    event_publisher::{EventPublisher, events},
    routes::{
        internal::internal, login_get::login_get, message_create::message_create,
        message_get::message_get, message_update::message_update,
        user_channel_create::user_channel_create, user_get::user_get,
    },
    self_info_cache::SelfInfoCache,
    session::SessionName,
    settings::Settings,
};

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

    let session = Arc::new(
        SqliteSession::open(format!("session_{}.db", session_name))
            .await
            .expect("Failed to open session database"),
    );
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

    tokio::spawn(pool.runner.run());

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
    let event_publisher = Arc::new(EventPublisher::new(self_info_cache.clone(), &settings));
    let session_name = Arc::new(SessionName(session_name));

    tokio::spawn((async |client, publisher: Arc<EventPublisher>| {
        publisher.publish(client, pool.updates).await
    })(client.clone(), event_publisher.clone()));

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
            .state(JsonConfig::default().limit(settings.json_limit))
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
