use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    convert::{
        login::satori_login_from_tg_user,
        message_receive::{satori_elements_from_tg_message, satori_message_from_tg_message},
        user::satori_user_from_tg_peer,
    },
    satori::{
        element::dump,
        types::{Button, Event},
    },
};
use grammers_client::{message::Message, peer::User, update::CallbackQuery};

fn timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards.")
        .as_secs_f64()
}

pub fn satori_event_from_tg_message(login: &User, message: &Message) -> Event {
    let self_id = login.id().bot_api_dialog_id();
    let login = satori_login_from_tg_user(login);
    let satori_msg = satori_message_from_tg_message(self_id, message);
    let user = satori_msg.user.clone();
    Event {
        sn: 0,
        event_type: "message-created".to_string(),
        timestamp: satori_msg.created_at.unwrap_or_else(timestamp),
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
    }
}

pub fn satori_event_from_tg_messages(login: &User, messages: &[&Message]) -> Event {
    let self_id = login.id().bot_api_dialog_id();
    let login = satori_login_from_tg_user(login);
    let mut first_msg = satori_message_from_tg_message(self_id, &messages[0]);
    let other_msgs = messages[1..]
        .into_iter()
        .map(|message| satori_elements_from_tg_message(self_id, message))
        .flatten()
        .collect::<Vec<_>>();
    first_msg.content += &dump(other_msgs);
    let user = first_msg.user.clone();
    Event {
        sn: 0,
        event_type: "message-created".to_string(),
        timestamp: first_msg.created_at.unwrap_or_else(timestamp),
        login,
        argv: None,
        button: None,
        channel: first_msg.channel.clone(),
        guild: first_msg.guild.clone(),
        member: first_msg.member.clone(),
        message: Some(first_msg),
        operator: None,
        role: None,
        user: user,
        referrer: None,
    }
}

pub fn satori_event_from_tg_callback(
    login: &User,
    callback: &CallbackQuery,
    message: &Message,
) -> Event {
    let self_id = login.id().bot_api_dialog_id();
    let login = satori_login_from_tg_user(login);
    let message = satori_message_from_tg_message(self_id, message);
    let user = callback
        .peer()
        .map(|peer| satori_user_from_tg_peer(self_id, peer));
    let button = Button {
        id: String::from_utf8_lossy(callback.data()).to_string(),
    };
    Event {
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
    }
}
