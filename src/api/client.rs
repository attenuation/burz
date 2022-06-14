use std::borrow::Borrow;

use futures_util::Stream;
use reqwest::{Method, StatusCode};
use serde::Serialize;
use snafu::prelude::*;

use super::error::variant::*;
use super::types::*;
use super::Result;
use async_stream::try_stream;


static BASE_URL: &str = "https://www.kaiheila.cn/api/v3";

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Kaiheila HTTP API Client
#[derive(Debug)]
pub struct Client {
    client: reqwest::Client,
}

/// guild_user_list_stream arg
#[derive(Debug)]
pub struct GuildUserListSetting {
    pub guild_id: String,
    pub channel_id: Option<String>,
    pub search: Option<String>,
    pub role_id: Option<i32>,
    pub mobile_verified: Option<bool>,
    pub active_time: Option<bool>,
    pub joined_at: Option<bool>,
    pub filter_user_id: Option<String>
}

/// guild_nickname arg
#[derive(Debug)]
pub struct GuildNicknameSetting {
    pub guild_id: String,
    pub nickname: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Serialize)]
struct GuildNicknamePostData {
    pub guild_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nickname: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub user_id: String
}

#[derive(Serialize)]
pub struct GuildLeavePostData {
    pub guild_id: String
}

#[derive(Serialize)]
pub struct GuildKickoutPostData {
    pub guild_id: String,
    pub target_id: String
}

#[derive(Serialize, Debug)]
pub struct GuildMutePostSetting {
    pub guild_id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub type_field: i32
}

impl Client {
    fn new<S: AsRef<str> + ?Sized>(auth_type: &'static str, token: &S) -> Result<Self> {
        let token = token.as_ref();
        let auth_header_value = format!("{} {}", auth_type, token).parse().map_err(|_| {
            TokenInvalid {
                token: token.to_string(),
            }
            .build()
        })?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, auth_header_value);

        let client = reqwest::Client::builder()
            .gzip(true)
            .deflate(true)
            .user_agent(APP_USER_AGENT)
            .default_headers(headers)
            .build()
            .context(ClientCreateFailed)?;

        Ok(Self { client })
    }

    /// create a new api client using bot token
    pub fn new_from_bot_token<S: AsRef<str> + ?Sized>(token: &S) -> Result<Self> {
        Self::new("Bot", token)
    }

    /// create a new api client using oauth2 token
    pub fn new_from_oauth2_token<S: AsRef<str> + ?Sized>(token: &S) -> Result<Self> {
        Self::new("Bearer", token)
    }

    async fn request<R, P, Q, K, V>(&self, path: &P, method: Method, query: Option<Q>, forms: Option<Q>, json: Option<&str>) -> Result<R>
    where
        P: AsRef<str> + ?Sized,
        Q: IntoIterator,
        Q::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
        R: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", BASE_URL, path.as_ref());
        let mut req = self.client.get(&url);

        if let Some(query_iner) = query {
            for q in query_iner.into_iter() {
                let (k, v) = q.borrow();
                req = req.query(&[(k.as_ref(), v.as_ref())]);
            }
        }

        if let Some(form) = forms {
            for q in form.into_iter() {
                let (k, v) = q.borrow();
                req = req.form(&[(k.as_ref(), v.as_ref())]);
            }
        }

        if let Some(json_inner) = json {
            req = req.header("Content-type", "application/json").body(json_inner.to_string());
        }


        let req = req.build().context(BuildRequestFailed)?;

        let resp = self
            .client
            .execute(req)
            .await
            .with_context(|_| RequestFailed {
                method: &method,
                url: &url,
            })?;

        ensure!(
            resp.status() == StatusCode::OK,
            HTTPStatusNotOK {
                method: &method,
                url: &url,
                status_code: resp.status()
            }
        );

        let body = resp.bytes().await.with_context(|_| RequestFailed {
            method: &method,
            url: &url,
        })?;

        let result: Response<R> =
            serde_json::from_slice(&body).with_context(|_| ParseBodyFailed { body })?;

        ensure!(
            result.code == 0,
            CodeNotZero {
                code: result.code,
                message: result.message
            }
        );

        Ok(result.data)
    }

    /// Call /gateway/index, get gateway url
    pub async fn gateway_url(&self) -> Result<String> {
        let data: GatewayIndexData = self.request("/gateway/index", Method::GET, Some(&[("compress", "1")]), None, None).await?;
        Ok(data.url)
    }

    ///  Call /guild/list, get guild list stream item
    pub async fn guild_list_stream(&self) -> impl Stream<Item = Result<GuildListItem>> + '_{
        try_stream! {
            let data: GuildListData = self.request("/guild/list",Method::GET, Some(&[("compress", "1")]), None, None).await?;
            for item in data.items {
                yield item
            }
            if data.meta.page_total != 1 {
                for i in 1..data.meta.page_total {
                    let data: GuildListData = self.request("/guild/list", Method::GET, Some(&[("compress", "1"), ("page", &i.to_string()), ("page_size", &data.meta.page_size.to_string())]), None, None).await?;
                    for item in data.items {
                        yield item
                    }
                }
            }
        }
     }

    /// Call /guild/view, get guild view info
    pub async fn guild_view<S: AsRef<str> + ?Sized>(&self, guid: &S) -> Result<GuildViewData> {
        let data: GuildViewData = self.request("/guild/view",Method::GET, Some( &[("compress", "1"), ("guild_id", guid.as_ref())]), None, None).await?;
        Ok(data)
    }

    ///  Call /guild/user-list, get guild list stream item
    pub async fn guild_user_list_stream<'a>(&'a self, setting: &'a GuildUserListSetting) -> impl Stream<Item = Result<GuildListUserItem>> + '_{
        try_stream! {
            let role_id_str: String;
            let mut query_vec: Vec<(&str, &str)> = Vec::new();
            query_vec.push(("compress", "1"));
            query_vec.push(("guild_id", &setting.guild_id));
            if let Some(chanel_id) = &setting.channel_id {
                query_vec.push(("channel_id", &chanel_id));
            }

            if let Some(search) = &setting.search {
                query_vec.push(("search", &search));
            }

            if let Some(role_id) = &setting.role_id {
                role_id_str = role_id.to_string();
                query_vec.push(("role_id", &role_id_str));
            }

            if let Some(mobile_verified) = &setting.mobile_verified {
                if *mobile_verified {
                    query_vec.push(("mobile_verified", "1"));
                } else {
                    query_vec.push(("mobile_verified", "0"));
                }
            }

            if let Some(active_time) = &setting.active_time {
                if *active_time {
                    query_vec.push(("active_time", "1"));
                } else {
                    query_vec.push(("active_time", "0"));
                }
            }

            if let Some(joined_at) = &setting.joined_at {
                if *joined_at {
                    query_vec.push(("active_time", "1"));
                } else {
                    query_vec.push(("active_time", "1"));
                }
            }
        
            let data: GuildListUserData = self.request("/guild/user-list",Method::GET, Some(&query_vec), None, None).await?;
            for item in data.items {
                yield item
            }
            if data.meta.page_total != 1 {
                for i in 1..data.meta.page_total {
                    let mut query_tmp_vec = query_vec.clone();
                    let page_str = i.to_string();
                    let page_size_str = data.meta.page_size.to_string();
                    query_tmp_vec.push(("page", &page_str));
                    query_tmp_vec.push(("page_size", &page_size_str));
                    let data: GuildListUserData = self.request("/guild/user-list", Method::GET, Some(&query_tmp_vec), None, None).await?;
                    for item in data.items {
                        yield item
                    }
                }
            }
        }
    }

    ///  Call /guild/nickname, return ()
    pub async fn guild_nickname(&self, setting: &GuildNicknameSetting) -> Result<()> {
        let mut json_post_data = GuildNicknamePostData {
            guild_id:  setting.guild_id.to_string(),
            nickname: String::new(),
            user_id: String::new(),
        };

        if let Some(nickname) = &setting.nickname {
            json_post_data.nickname = nickname.to_string();
        }

        if let Some(user_id) = &setting.user_id {
            json_post_data.user_id = user_id.to_string();
        }

        let data = serde_json::to_string(&json_post_data).unwrap();

        log::info!("post data: {:?}", data);

        let _: serde_json::Map<_, _> = self.request("/guild/nickname", Method::POST, Some(&[("compress", "1")]), None, Some(&data)).await?;
        Ok(())
    }

    ///  Call /guild/leave, return ()
    pub async fn guild_leave<S: AsRef<str> + ?Sized>(&self, guid: &S) -> Result<()> {
        let json_post_data = GuildLeavePostData {
            guild_id:  guid.as_ref().to_string(),
        };

        let data = serde_json::to_string(&json_post_data).unwrap();

        let _: serde_json::Map<_, _> = self.request("/guild/leave", Method::POST, Some(&[("compress", "1")]), None, Some(&data)).await?;
        Ok(())
    }

    ///  Call /guild/kickout, return ()
    pub async fn guild_kickout<S: AsRef<str> + ?Sized>(&self, guid: &S, target: &S) -> Result<()> {
        let json_post_data = GuildKickoutPostData {
            guild_id:  guid.as_ref().to_string(),
            target_id: target.as_ref().to_string()
        };

        let data = serde_json::to_string(&json_post_data).unwrap();

        let _: serde_json::Map<_, _> = self.request("/guild/kickout", Method::POST, Some(&[("compress", "1")]), None, Some(&data)).await?;
        Ok(())
    }

    ///  Call /guild-mute/list, return GuildMuteListData
    pub async fn guild_mute_list<S: AsRef<str> + ?Sized>(&self, guid: &S) -> Result<GuildMuteListData> {
        let data = self.request("/guild-mute/list", Method::GET, Some(&[("compress", "1"), ("return_type", "detail"), ("guid_id", guid.as_ref())]), None, None).await?;
        Ok(data)
    }

    ///  Call /guild-mute/create, return ()
    pub async fn guild_mute_create(&self, setting: &GuildMutePostSetting) -> Result<()> {
        let data = serde_json::to_string(&setting).unwrap();

        let _: serde_json::Map<_, _> = self.request("/guild-mute/create", Method::POST, Some(&[("compress", "1")]), None, Some(&data)).await?;
        Ok(())
    }

    ///  Call /guild-mute/create, return ()
    pub async fn guild_mute_delete(&self, setting: &GuildMutePostSetting) -> Result<()> {
        let data = serde_json::to_string(&setting).unwrap();

        let _: serde_json::Map<_, _> = self.request("/guild-mute/delete", Method::POST, Some(&[("compress", "1")]), None, Some(&data)).await?;
        Ok(())
    }
}
