use std::collections::HashMap;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: String,
    pub name: Option<String>,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub is_bot: Option<bool>,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone)]
#[repr(u8)]
pub enum LoginStatus {
    Offline = 0,
    Online = 1,
    Connect = 2,
    Disconnect = 3,
    Reconnect = 4,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Login {
    pub sn: u8,
    pub platform: Option<String>,
    pub user: Option<User>,
    pub status: LoginStatus,
    pub adapter: String,
    pub features: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Argv {
    pub name: String,
    pub arguments: Vec<String>,
    pub options: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Button {
    pub id: String,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone)]
#[repr(u8)]
pub enum ChannelType {
    Text = 0,
    Direct = 1,
    Category = 2,
    Voice = 3,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Channel {
    pub id: String,
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    pub name: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Guild {
    pub id: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Member {
    pub user: Option<User>,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub joined_at: Option<f64>,
    pub roles: Option<Vec<Role>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Role {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: String,
    pub content: String,
    pub channel: Option<Channel>,
    pub guild: Option<Guild>,
    pub member: Option<Member>,
    pub user: Option<User>,
    pub created_at: Option<f64>,
    pub updated_at: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub sn: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub timestamp: f64,
    pub login: Login,
    pub argv: Option<Argv>,
    pub button: Option<Button>,
    pub channel: Option<Channel>,
    pub guild: Option<Guild>,
    pub member: Option<Member>,
    pub message: Option<Message>,
    pub operator: Option<User>,
    pub role: Option<Role>,
    pub user: Option<User>,
    pub referrer: Option<()>,
}

#[derive(Debug, Clone)]
pub enum WsOp {
    Event(Event),
    Ping,
    Pong,
    Identify(WsIdentifyBody),
    Ready(WsReadyBody),
    Meta(WsMetaBody),
}

impl<'de> Deserialize<'de> for WsOp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        struct Literal<const V: u8>;

        impl<'de, const V: u8> Deserialize<'de> for Literal<V> {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = u8::deserialize(deserializer)?;
                if value == V {
                    Ok(Literal::<V>)
                } else {
                    Err(serde::de::Error::custom("Invalid literal."))
                }
            }
        }

        #[derive(Deserialize, Debug)]
        #[serde(untagged)]
        pub enum WsOpTmp {
            Event {
                #[allow(unused)]
                op: Literal<0>,
                body: Event,
            },
            Ping {
                #[allow(unused)]
                op: Literal<1>,
            },
            Pong {
                #[allow(unused)]
                op: Literal<2>,
            },
            Identify {
                #[allow(unused)]
                op: Literal<3>,
                body: WsIdentifyBody,
            },
            Ready {
                #[allow(unused)]
                op: Literal<4>,
                body: WsReadyBody,
            },
            Meta {
                #[allow(unused)]
                op: Literal<5>,
                body: WsMetaBody,
            },
        }

        let tmp = WsOpTmp::deserialize(deserializer)?;
        Ok(match tmp {
            WsOpTmp::Event { body, .. } => WsOp::Event(body),
            WsOpTmp::Ping { .. } => WsOp::Ping,
            WsOpTmp::Pong { .. } => WsOp::Pong,
            WsOpTmp::Identify { body, .. } => WsOp::Identify(body),
            WsOpTmp::Ready { body, .. } => WsOp::Ready(body),
            WsOpTmp::Meta { body, .. } => WsOp::Meta(body),
        })
    }
}

impl Serialize for WsOp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Protocol", 2)?;
        match self {
            WsOp::Event(body) => {
                state.serialize_field("op", &0)?;
                state.serialize_field("body", &body)?;
            }
            WsOp::Ping => {
                state.serialize_field("op", &1)?;
                state.serialize_field("body", &())?;
            }
            WsOp::Pong => {
                state.serialize_field("op", &2)?;
                state.serialize_field("body", &())?;
            }
            WsOp::Identify(body) => {
                state.serialize_field("op", &3)?;
                state.serialize_field("body", &body)?;
            }
            WsOp::Ready(body) => {
                state.serialize_field("op", &4)?;
                state.serialize_field("body", &body)?;
            }
            WsOp::Meta(body) => {
                state.serialize_field("op", &5)?;
                state.serialize_field("body", &body)?;
            }
        }
        state.end()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsIdentifyBody {
    pub token: Option<String>,
    pub sn: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsReadyBody {
    pub logins: Vec<Login>,
    pub proxy_urls: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WsMetaBody {
    pub proxy_urls: Vec<String>,
}
