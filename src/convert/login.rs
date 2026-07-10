use grammers_client::peer::User;
use presence_rs::Presence;

use crate::convert::user::satori_user_from_tg_user;
use crate::satori::types::{Login, LoginStatus};

pub fn satori_login_from_tg_user(user: &User) -> Login {
    Login {
        sn: 0,
        platform: Presence::Some("telegram".into()),
        user: Presence::Some(satori_user_from_tg_user(
            user.id().bot_api_dialog_id().unwrap_or_default(),
            user,
        )),
        status: LoginStatus::Online,
        adapter: "mtproto".into(),
        features: Presence::Some(Vec::new()), // nonebot-adapter-satori complains about null features.
    }
}
