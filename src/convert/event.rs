use crate::{
    convert::message_receive::satori_message_from_tg_message,
    satori::types::{Event, Login},
};
use grammers_client::update::Update;

pub fn satori_event_from_tg_update(self_id: i64, update: &Update, login: Login) -> Option<Event> {
    match update {
        Update::NewMessage(message) => {
            let satori_msg = satori_message_from_tg_message(self_id, message);
            let user = satori_msg.user.clone();
            Some(Event {
                sn: 0,
                event_type: "message-created".to_string(),
                timestamp: satori_msg.created_at?,
                login: login,
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
        _ => None,
    }
}
