//! Kaiheila HTTP API response types

use std::{collections::HashMap, fmt::Display, str::FromStr};

use serde::Deserialize;
use serde_with::serde_as;
use snafu::prelude::*;

use crate::ws::message::{Message, SN};

/// Response is common response structure with a code and message, and a data field.
#[derive(Debug, Deserialize)]
pub struct Response<T> {
    /// zero is success, see kaiheila doc for details
    pub code: i64,
    /// error message
    pub message: String,
    /// result data, differ type for each api
    pub data: T,
}

/// data type for api /gateway/index
#[derive(Debug, Deserialize)]
pub struct GatewayIndexData {
    /// gateway url
    pub url: String,
}

/// Parse string as gateway url error
#[derive(Debug, Snafu)]
#[snafu(
    visibility(pub(crate)),
    module(parse_gateway_url_error_variant),
    context(suffix(false))
)]
pub enum ParseGatewayURLError {
    #[snafu(display("{s} is an invalid url: {source}"))]
    /// the str is not a valid url
    InvalidURL {
        /// string be parsed
        s: String,
        /// source error
        source: url::ParseError,
    },

    /// the parsed url schema is not websocket
    #[snafu(display("the url {s} has invalid schema {schema}, only ws or wss is ok"))]
    InvalidSchema {
        /// the url
        s: String,
        /// invalid schema
        schema: String,
    },

    /// the parsed url has no host
    #[snafu(display("the gateway url {s} has no host"))]
    NoHost {
        /// the url
        s: String,
    },

    /// the parsed url has no token
    #[snafu(display("the gateway url {s} has no token"))]
    NoToken {
        /// the url
        s: String,
    },

    /// the parsed url has no sn for resume
    #[snafu(display("the gateway url {s} has no sn when resume is 1"))]
    NoSN {
        /// the url
        s: String,
    },

    /// the parsed url has invalid sn(not unsigned number) for resume
    #[snafu(display("the gateway url {s} has invalid sn"))]
    InvalidSN {
        /// the url
        s: String,
        /// source error
        source: std::num::ParseIntError,
    },

    /// the parsed url has no session id for resume
    #[snafu(display("the gateway url {s} has no session_id when resume is 1"))]
    NoSessionID {
        /// the url
        s: String,
    },
}

/// needed arguments when reconnect to a gateway
#[derive(Debug, Clone, Default)]
pub struct GatewayResumeArguments {
    /// last message id
    pub sn: u64,
    /// last session id
    pub session_id: String,
}

impl GatewayResumeArguments {
    pub(crate) fn ping(&self) -> Message {
        Message::Ping(SN { sn: self.sn })
    }
}

/// parsed returned gateway url
#[derive(Debug, Clone)]
pub struct GatewayURLInfo {
    /// url schema
    pub schema: String,
    /// gateway host(domain)
    pub host: String,
    /// gateway port
    pub port: Option<u16>,
    /// request path
    pub path: String,

    /// enable server->client message compress
    pub compress: bool,
    /// gateway token
    pub token: String,
    /// resume conversion arguments
    pub resume: Option<GatewayResumeArguments>,
}

impl GatewayURLInfo {
    /// construct final url
    pub fn url(&self) -> url::Url {
        let mut u =
            url::Url::parse(&format!("{}://{}{}", self.schema, self.host, self.path)).unwrap();
        let _ = u.set_port(self.port);

        {
            let mut query = u.query_pairs_mut();
            query.append_pair("compress", if self.compress { "1" } else { "0" });
            query.append_pair("token", &self.token);
            if let Some(ref resume) = self.resume {
                query.append_pair("resume", "1");
                query.append_pair("sn", &format!("{}", resume.sn));
                query.append_pair("session_id", resume.session_id.as_str());
            }
        }

        u
    }
}

impl FromStr for GatewayURLInfo {
    type Err = ParseGatewayURLError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = url::Url::parse(s)
            .with_context(|_| parse_gateway_url_error_variant::InvalidURL { s: s.to_string() })?;

        ensure!(
            url.scheme() == "wss" || url.scheme() == "ws",
            parse_gateway_url_error_variant::InvalidSchema {
                s,
                schema: url.scheme(),
            }
        );

        ensure!(
            url.host().is_some(),
            parse_gateway_url_error_variant::NoHost { s }
        );

        let query = url.query_pairs().collect::<HashMap<_, _>>();

        let compress = query
            .get("compress")
            .map(|val| val == "1")
            .unwrap_or_default();

        let token = query
            .get("token")
            .with_context(|| parse_gateway_url_error_variant::NoToken { s })?;

        let resume = match query.get("resume") {
            Some(val) if val == "1" => {
                let sn = query
                    .get("sn")
                    .with_context(|| parse_gateway_url_error_variant::NoSN { s })?;

                let session_id = query
                    .get("session_id")
                    .with_context(|| parse_gateway_url_error_variant::NoSessionID { s })?;

                Some(GatewayResumeArguments {
                    sn: sn
                        .parse()
                        .with_context(|_| parse_gateway_url_error_variant::InvalidSN { s })?,
                    session_id: session_id.to_string(),
                })
            }
            _ => None,
        };

        Ok(GatewayURLInfo {
            schema: url.scheme().to_string(),
            host: url.host().unwrap().to_string(),
            port: url.port(),
            path: url.path().to_string(),
            compress,
            token: token.to_string(),
            resume,
        })
    }
}

impl Display for GatewayURLInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.url().fmt(f)
    }
}

/// data type for api /guild/list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildListData {
    /// items
    pub items: Vec< GuildListItem>,
    /// meta
    pub meta: GuildListMeta,
    /// sort
    pub sort: GuildSort,
}

/// data.items type for api /guild/list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct  GuildListItem {
    /// id
    pub id: String,
    /// name
    pub name: String,
    /// topic
    pub topic: String,
    /// master_id
    #[serde(rename = "master_id")]
    pub master_id: String,
    /// icon
    pub icon: String,
    /// notify_type
    #[serde(rename = "notify_type")]
    pub notify_type: i64,
    /// region
    pub region: String,
    /// enable_open
    #[serde(rename = "enable_open")]
    pub enable_open: bool,
    /// open_id
    #[serde(rename = "open_id")]
    pub open_id: String,
    /// default_channel_id
    #[serde(rename = "default_channel_id")]
    pub default_channel_id: String,
    /// welcome_channel_id
    #[serde(rename = "welcome_channel_id")]
    pub welcome_channel_id: String,
    /// boost_num
    #[serde(rename = "boost_num")]
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub boost_num: i64,
    /// level
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub level: i64,
}

/// data.meta type for api /guild/list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct  GuildListMeta {
    /// page
    pub page: i64,
    /// page_total
    #[serde(rename = "page_total")]
    pub page_total: i64,
    /// page_size
    #[serde(rename = "page_size")]
    pub page_size: i64,
    /// total
    pub total: i64,
}

/// data.sort type for api /guild/list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct GuildSort {
    /// id
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub id: i64,
}

/// data type for api /guild/view
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildViewData {
    /// roles
    pub roles: Vec<GuildViewRole>,
    /// chanels
    pub channels: Vec<GuildViewChannel>,
    /// id
    pub id: String,
    /// name
    pub name: String,
    /// topic
    pub topic: String,
    /// master_id
    #[serde(rename = "master_id")]
    pub master_id: String,
    /// icon
    pub icon: String,
    #[serde(rename = "notify_type")]
    /// notify_type
    pub notify_type: i64,
    /// region
    pub region: String,
    /// enable_open
    #[serde(rename = "enable_open")]
    pub enable_open: bool,
    /// open_id
    #[serde(rename = "open_id")]
    pub open_id: String,
    /// default_channel_id
    #[serde(rename = "default_channel_id")]
    pub default_channel_id: String,
    /// welcome_channel_id
    #[serde(rename = "welcome_channel_id")]
    pub welcome_channel_id: String,
    /// boost_num
    #[serde(rename = "boost_num")]
    pub boost_num: i64,
    /// level
    pub level: i64,
}

/// data.role type for api /guild/view
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildViewRole {
    /// role_id
    #[serde(rename = "role_id")]
    pub role_id: i64,
    /// name
    pub name: String,
    /// color
    pub color: i64,
    /// position
    pub position: i64,
    /// hoist
    pub hoist: i64,
    /// mentionable
    pub mentionable: i64,
    /// permissions
    pub permissions: i64,
}

/// data.channel type for api /guild/view
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildViewChannel {
    /// id
    pub id: String,
    /// guild_id
    #[serde(rename = "guild_id")]
    pub guild_id: String,
    /// master_id
    #[serde(rename = "master_id")]
    pub master_id: String,
    /// parent_id
    #[serde(rename = "parent_id")]
    pub parent_id: String,
    /// name
    pub name: String,
    /// topic
    pub topic: String,
    /// type
    #[serde(rename = "type")]
    pub type_field: i64,
    /// level
    pub level: i64,
    /// slow_mode
    #[serde(rename = "slow_mode")]
    pub slow_mode: i64,
    /// is_category
    #[serde(rename = "is_category")]
    pub is_category: bool,
}

/// data type for api /guild/user-list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildListUserData {
    /// items
    pub items: Vec<GuildListUserItem>,
    /// meta
    pub meta: GuildListUserMeta,
    /// sort 
    pub sort: GuildSort,
    /// user_count 
    #[serde(rename = "user_count")]
    pub user_count: i64,
    /// online_count
    #[serde(rename = "online_count")]
    pub online_count: i64,
    /// offline_count
    #[serde(rename = "offline_count")]
    pub offline_count: i64,
}

/// data.item type for api /guild/user-list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildListUserItem {
    /// id
    pub id: String,
    /// username
    pub username: String,
    /// identify_num
    #[serde(rename = "identify_num")]
    pub identify_num: String,
    /// online
    pub online: bool,
    /// status
    pub status: i64,
    /// bot
    pub bot: bool,
    /// avatar
    pub avatar: String,
    /// vip_avatar
    #[serde(rename = "vip_avatar")]
    pub vip_avatar: String,
    /// nickname
    pub nickname: String,
    /// roles
    pub roles: Vec<i64>,
}

/// data.meta type for api /guild/user-list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildListUserMeta {
    /// page
    pub page: i64,
    /// page_total
    #[serde(rename = "page_total")]
    pub page_total: i64,
    /// page_size
    #[serde(rename = "page_size")]
    pub page_size: i64,
    /// total
    pub total: i64,
}

/// data type for api /guild-mute/list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMuteListData {
    /// mic
    pub mic: GuildMuteListMic,
    /// headset
    pub headset: GuildMuteListHeadset,
}

/// data.mic type for api /guild-mute/list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMuteListMic {
    /// type
    #[serde(rename = "type")]
    pub type_field: i64,
    /// user_ids
    #[serde(rename = "user_ids")]
    pub user_ids: Vec<String>,
}

/// data.headset type for api /guild-mute/list
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMuteListHeadset {
    /// type
    #[serde(rename = "type")]
    pub type_field: i64,
    /// user_ids
    #[serde(rename = "user_ids")]
    pub user_ids: Vec<String>,
}