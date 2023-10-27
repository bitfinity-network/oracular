use std::borrow::Cow;
use std::collections::HashMap;

use candid::{CandidType, Func};
use did::U256;
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
    HttpResponse as MHttpResponse, TransformArgs, TransformContext,
};

use serde::Deserialize;
use serde_bytes::ByteBuf;
use serde_json::Value;
use url::Url;

use crate::error::{Error, Result};
use crate::parser::ValueParser;

pub const PRICE_MULTIPLE: f64 = 1_0000_0000.0;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Token {}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum StreamingStrategy {
    Callback { callback: Func, token: Token },
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackHttpResponse {
    pub body: ByteBuf,
    pub token: Option<Token>,
}

/// The important components of an HTTP request.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    /// The HTTP method string.
    pub method: Cow<'static, str>,
    /// The URL that was visited.
    pub url: String,
    /// The request headers.
    pub headers: HashMap<Cow<'static, str>, Cow<'static, str>>,
    /// The request body.
    pub body: ByteBuf,
}

/// A HTTP response.
#[derive(Clone, Debug, CandidType)]
pub struct HttpResponse {
    /// The HTTP status code.
    pub status_code: u16,
    /// The response header map.
    pub headers: HashMap<&'static str, &'static str>,
    /// The response body.
    pub body: ByteBuf,
    /// The strategy for streaming the rest of the data, if the full response is to be streamed.
    pub streaming_strategy: Option<StreamingStrategy>,
    /// Whether the query call should be upgraded to an update call.
    pub upgrade: Option<bool>,
}

impl HttpResponse {
    pub fn new(
        status_code: u16,
        headers: HashMap<&'static str, &'static str>,
        body: ByteBuf,
        streaming_strategy: Option<StreamingStrategy>,
        upgrade: Option<bool>,
    ) -> Self {
        Self {
            status_code,
            headers,
            body,
            streaming_strategy,
            upgrade,
        }
    }

    pub fn error(status_code: u16, message: String) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: ByteBuf::from(message.into_bytes()),
            streaming_strategy: None,
            upgrade: None,
        }
    }
}

async fn http_outcall(
    url: &str,
    method: HttpMethod,
    body: Option<Vec<u8>>,
    max_response_bytes: Option<u64>,
) -> Result<MHttpResponse> {
    let real_url = Url::parse(url).map_err(|e| Error::Http(e.to_string()))?;
    let headers = vec![
        HttpHeader {
            name: "Host".to_string(),
            value: real_url
                .domain()
                .ok_or_else(|| Error::Http("empty domain of url".to_string()))?
                .to_string(),
        },
        HttpHeader {
            name: "User-Agent".to_string(),
            value: "Oracular".to_string(),
        },
    ];

    let request = CanisterHttpRequestArgument {
        url: url.to_string(),
        max_response_bytes,
        method,
        headers,
        body,
        transform: Some(TransformContext::from_name("transform".to_string(), vec![])), // TODO:: Verify this
    };

    let res = http_request(request.clone(), 0)
        .await
        .map(|(res,)| res)
        .map_err(|(r, m)| Error::Http(format!("RejectionCode: {r:?}, Error: {m}")))?;

    Ok(res)
}

pub fn transform(raw: TransformArgs) -> MHttpResponse {
    MHttpResponse {
        status: raw.response.status,
        body: raw.response.body,
        ..Default::default()
    }
}

pub async fn get_price(url: &str, json_path: &str) -> Result<U256> {
    let res = http_outcall(url, HttpMethod::GET, None, Some(8000)).await?;

    if res.status != 200 {
        return Err(Error::Internal(format!(
            "url is not valid, status: {}",
            res.status
        )));
    }

    let json_body = serde_json::from_slice::<Value>(&res.body)
        .map_err(|e| Error::Http(format!("serde_json err: {e}")))?;

    let price = json_body.parse(json_path)?;

    let price_f64 = price
        .as_f64()
        .ok_or_else(|| Error::Internal(format!("price is not a f64, price: {}", price)))?;

    let price_u64 = (price_f64 * PRICE_MULTIPLE).round() as u64;

    Ok(U256::from(price_u64))
}
