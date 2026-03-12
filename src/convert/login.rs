use crate::{
    convert::user::satori_user_from_tg_user,
    satori::types::{Login, LoginStatus},
};
use grammers_client::peer::User;

pub fn satori_login_from_tg_user(user: &User) -> Login {
    Login {
        sn: 0,
        platform: Some("telegram".into()),
        user: Some(satori_user_from_tg_user(
            user.id().bot_api_dialog_id(),
            user,
        )),
        status: LoginStatus::Online,
        adapter: "mtproto".into(),
        features: Some(Vec::new()), // nonebot-adapter-satori complains about null features.
    }
}
