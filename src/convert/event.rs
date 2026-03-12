use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    convert::{
        login::satori_login_from_tg_user, message_receive::satori_message_from_tg_message,
        user::satori_user_from_tg_peer,
    },
    satori::types::{Button, Event},
};
use grammers_client::{message::Message, peer::User, update::Update};

fn timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards.")
        .as_secs_f64()
}

pub fn satori_event_from_tg_update(
    login: &User,
    update: &Update,
    additional_message: Option<&Message>,
) -> Option<Event> {
    let self_id = login.id().bot_api_dialog_id();
    let login = satori_login_from_tg_user(login);
    match update {
        Update::NewMessage(message) => {
            let satori_msg = satori_message_from_tg_message(self_id, message);
            let user = satori_msg.user.clone();
            Some(Event {
                sn: 0,
                event_type: "message-created".to_string(),
                timestamp: satori_msg.created_at?,
                login,
                argv: None,
                button: None,
                channel: satori_msg.channel.clone(),
                guild: satori_msg.guild.clone(),
                member: satori_msg.member.clone(),
                message: Some(satori_msg),
                operator: None,
                role: None,
                user: user,
                referrer: None,
            })
        }
        Update::CallbackQuery(callback) => {
            let satori_msg = additional_message.map(|x| satori_message_from_tg_message(self_id, x));
            let user = callback
                .peer()
                .map(|peer| satori_user_from_tg_peer(self_id, peer));
            let button = Button {
                id: String::from_utf8_lossy(callback.data()).to_string(),
            };
            if let Some(message) = satori_msg {
                Some(Event {
                    sn: 0,
                    event_type: "interaction/button".to_string(),
                    timestamp: timestamp(),
                    login,
                    argv: None,
                    button: Some(button),
                    channel: message.channel.clone(),
                    guild: message.guild.clone(),
                    member: None,
                    message: Some(message),
                    operator: None,
                    role: None,
                    user,
                    referrer: None,
                })
            } else {
                Some(Event {
                    sn: 0,
                    event_type: "interaction/button".to_string(),
                    timestamp: timestamp(),
                    login,
                    argv: None,
                    button: Some(button),
                    channel: None,
                    guild: None,
                    member: None,
                    message: None,
                    operator: None,
                    role: None,
                    user,
                    referrer: None,
                })
            }
        }
        _ => None,
    }
}
