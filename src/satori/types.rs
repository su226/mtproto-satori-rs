use std::collections::HashMap;

use presence_rs::Presence;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[inline]
pub fn provide<T>(value: Option<T>) -> Presence<T> {
    match value {
        Some(value) => Presence::Some(value),
        None => Presence::Null,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: String,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub name: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub nick: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub avatar: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub is_bot: Presence<bool>,
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
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub platform: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub user: Presence<User>,
    pub status: LoginStatus,
    pub adapter: String,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub features: Presence<Vec<String>>,
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
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub name: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub parent_id: Presence<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Guild {
    pub id: String,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub name: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub avatar: Presence<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Member {
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub user: Presence<User>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub nick: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub avatar: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub joined_at: Presence<f64>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub roles: Presence<Vec<Role>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Role {
    pub id: String,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub name: Presence<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: String,
    pub content: String,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub channel: Presence<Channel>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub guild: Presence<Guild>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub member: Presence<Member>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub user: Presence<User>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub created_at: Presence<f64>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub updated_at: Presence<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub sn: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub timestamp: f64,
    pub login: Login,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub argv: Presence<Argv>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub button: Presence<Button>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub channel: Presence<Channel>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub guild: Presence<Guild>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub member: Presence<Member>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub message: Presence<Message>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub operator: Presence<User>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub role: Presence<Role>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub user: Presence<User>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub referrer: Presence<()>,
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
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub token: Presence<String>,
    #[serde(skip_serializing_if = "Presence::is_absent")]
    pub sn: Presence<u32>,
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
