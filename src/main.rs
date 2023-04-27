use std::time::SystemTime;
use std::{collections::HashMap, sync::RwLock};

#[macro_use]
extern crate failure;

#[macro_use]
extern crate rocket;
use headless_chrome::protocol::Method;
use rocket::response::status::NotFound;

#[macro_use]
extern crate lazy_static;

use serde::{Deserialize, Serialize};

use headless_chrome::{
    browser::tab::RequestInterceptionDecision,
    protocol::network::{events::RequestInterceptedEventParams, methods::RequestPattern},
    Browser,
};
extern crate openssl_probe;

use hyper::{Client, Uri};

use anyhow::{Context, Result};

static USERNAME: &str = "USER_NAME_X@email.com";
static PASSWROD: &str = "PASS_NAME_X";
lazy_static! {
    static ref LATEST_RESPONSE: RwLock<Option<(SystemTime, LoginRequest)>> = RwLock::new(None);
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct CloseReturnObject {}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Close(pub Option<serde_json::Value>);

impl Method for Close {
    const NAME: &'static str = "Page.close";
    type ReturnObject = CloseReturnObject;
}

#[launch]
fn rocket() -> _ {
    openssl_probe::init_ssl_cert_env_vars();
    rocket::build().mount("/", routes![get_tipsport_login])
}

fn is_cached_valid(cached_value: &Option<(SystemTime, LoginRequest)>) -> bool {
    if let Some((time, _)) = cached_value {
        if SystemTime::now().duration_since(*time).unwrap().as_secs() < 18_000 {
            return true;
        }
    }
    false
}

#[get("/tipsport-login")]
async fn get_tipsport_login() -> Result<String, NotFound<String>> {
    eprintln!("Generating tipsport login request");
    let res = login_tipsport().await;
    if res.is_err() {
        eprintln!("login_tipsport error: {:?}", res);
    }
    {
        let cached = LATEST_RESPONSE.read().unwrap();
        if is_cached_valid(&cached) {
            return Ok(serde_json::to_string(&cached.as_ref().unwrap().1).unwrap());
        }
        Err(NotFound("Can't get login request".to_string()))
    }
}

#[derive(Serialize, Deserialize)]
struct Header {
    key: String,
    value: String,
}

#[derive(Serialize, Deserialize)]
struct LoginRequest {
    version: u8,
    url: String,
    post_data: String,
    username_keyword: String,
    password_keyword: String,
    headers: HashMap<String, String>,
}

async fn get_ws_url() -> Result<String, anyhow::Error> {
    let client = Client::new();
    let uri = Uri::from_static("http://nextools_chromium.localhost:9222/json/version");
    // let uri = Uri::from_static("http://localhost:9222/json/version");
    let resp = client
        .get(uri)
        .await
        .context("Failed to get json/version")?;
    let byte_stream = hyper::body::to_bytes(resp.into_body())
        .await
        .context("Failed to get bytes from response")?;
    let text = std::str::from_utf8(&byte_stream).context("Failed to create string from bytes")?;
    eprintln!("response text: {}", text);
    let data: HashMap<String, String> = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .context("Failed to parse json response")?;
    data.get("webSocketDebuggerUrl")
        .ok_or_else(|| anyhow::Error::msg("Can't get webSocketDebuggerUrl"))
        .map(|str| str.to_owned())
}

async fn login_tipsport() -> Result<(), failure::Error> {
    let wsl_url = get_ws_url()
        .await
        .map_err(|err| format_err!("{}", err.to_string()))?;
    let browser = Browser::connect(wsl_url)?;
    let incognito_context = browser.new_context()?;
    let tab = incognito_context.new_tab()?;
    tab.set_user_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/97.0.4692.99 Safari/537.36 OPR/83.0.4254.27",
        Some("cs-CZ,cs;q=0.9,en-GB;q=0.8,en;q=0.7"),
        None)?;
    let tab = tab.set_default_timeout(std::time::Duration::from_secs(2));
    let patterns = vec![RequestPattern {
        url_pattern: Some("*"),
        resource_type: None,
        interception_stage: None,
    }];
    let interceptor = |_transport,
                       _session_id,
                       params: RequestInterceptedEventParams|
     -> RequestInterceptionDecision {
        if params.request.post_data.as_ref().is_none() {
            return RequestInterceptionDecision::Continue;
        }
        if !params
            .request
            .post_data
            .as_ref()
            .unwrap()
            .contains(USERNAME)
        {
            return RequestInterceptionDecision::Continue;
        }

        let login_request = LoginRequest {
            version: 1,
            url: params.request.url,
            post_data: params.request.post_data.unwrap_or_else(|| "".into()),
            username_keyword: USERNAME.into(),
            password_keyword: PASSWROD.into(),
            headers: params
                .request
                .headers
                .into_iter()
                .filter(|(key, _)| key.to_lowercase().ne("host"))
                .filter(|(key, _)| key.to_lowercase().ne("cookie"))
                .collect(),
        };
        {
            let mut w = LATEST_RESPONSE.write().unwrap();
            *w = Some((SystemTime::now(), login_request));
        }
        eprintln!("Request captured");
        RequestInterceptionDecision::Response("RES".into())
    };

    tab.navigate_to("https://www.tipsport.cz")?;
    tab.enable_request_interception(&patterns, Box::new(interceptor))?;
    tab.wait_for_element("input#userNameId")?.click()?;
    tab.type_str(USERNAME)?;
    tab.wait_for_element("input#passwordId")?.click()?;
    tab.type_str(PASSWROD)?;
    tab.wait_for_element("button#btnLogin")?.click()?;
    let res = tab.wait_for_element("input#NonExistant");
    let _ = tab.call_method(Close(None));
    res?;
    Ok(())
}
