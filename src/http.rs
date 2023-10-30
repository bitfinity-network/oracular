use std::borrow::Cow;
use std::collections::HashMap;

use candid::CandidType;
use did::U256;
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod,
    HttpResponse as MHttpResponse, TransformArgs, TransformContext,
};
use jsonrpc_core::Output;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use serde_json::Value;
use url::Url;

use crate::constants::{
    HTTP_OUTCALL_BYTE_RECEIVED_COST, HTTP_OUTCALL_REQUEST_COST, INGRESS_MESSAGE_BYTE_RECEIVED_COST,
    INGRESS_MESSAGE_RECEIVED_COST, INGRESS_OVERHEAD_BYTES,
};
use crate::error::{Error, Result};
use crate::parser::ValueParser;

pub const PRICE_MULTIPLE: f64 = 100_000_000.0;

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
    /// Whether the query call should be upgraded to an update call.
    pub upgrade: Option<bool>,
}

impl HttpResponse {
    pub fn new(
        status_code: u16,
        headers: HashMap<&'static str, &'static str>,
        body: ByteBuf,
        upgrade: Option<bool>,
    ) -> Self {
        Self {
            status_code,
            headers,
            body,
            upgrade,
        }
    }

    pub fn error(status_code: u16, message: String) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: ByteBuf::from(message.into_bytes()),
            upgrade: None,
        }
    }
}

async fn http_outcall(
    url: &str,
    method: HttpMethod,
    body: Option<Vec<u8>>,
    cost: u128,
    max_response_bytes: Option<u64>,
) -> Result<MHttpResponse> {
    let real_url = Url::parse(url).map_err(|e| Error::Http(e.to_string()))?;

    let host = real_url
        .host_str()
        .ok_or_else(|| Error::Http("empty host of url".to_string()))?;

    let headers = vec![
        HttpHeader {
            name: "Host".to_string(),
            value: host.to_string(),
        },
        HttpHeader {
            name: "User-Agent".to_string(),
            value: "Oracular".to_string(),
        },
        HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/json".to_string(),
        },
    ];

    let request = CanisterHttpRequestArgument {
        url: url.to_string(),
        max_response_bytes,
        method,
        headers,
        body,
        transform: Some(TransformContext::from_name("transform".to_string(), vec![])),
    };

    let res = http_request(request.clone(), cost)
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

pub async fn call_jsonrpc(
    url: &str,
    method: &str,
    params: Value,
    max_response_bytes: Option<u64>,
) -> Result<Value> {
    let body = serde_json::to_vec(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    }))
    .map_err(|e| Error::Http(format!("serde_json err: {e}")))?;

    let cost = get_request_costs(url, body.len(), max_response_bytes.unwrap_or(8000));

    let res = http_outcall(url, HttpMethod::POST, Some(body), cost, max_response_bytes).await?;

    if res.status != 200 {
        return Err(Error::Internal(format!(
            "url is not valid, status: {} res: {}",
            res.status,
            String::from_utf8(res.body).unwrap_or_default()
        )));
    }

    let json_body = serde_json::from_slice::<Output>(&res.body)
        .map_err(|e| Error::Http(format!("serde_json err: {e}")))?;

    let output = match json_body {
        Output::Success(success) => success.result,
        Output::Failure(failure) => {
            return Err(Error::Http(format!(
                "JSON-RPC error: {}",
                failure.error.message
            )))
        }
    };

    Ok(output)
}

pub async fn get_price(url: &str, json_path: &str) -> Result<U256> {
    let cost = get_request_costs(url, 0, 8000);
    let res = http_outcall(url, HttpMethod::GET, None, cost, Some(8000)).await?;

    if res.status != 200 {
        return Err(Error::Internal(format!(
            "url is not valid, status: {} res: {}",
            res.status,
            String::from_utf8(res.body).unwrap_or_default()
        )));
    }

    let json_body = serde_json::from_slice::<Value>(&res.body)
        .map_err(|e| Error::Http(format!("serde_json err: {e}")))?;

    let price = json_body.parse(json_path)?;

    let price_f64 = price
        .as_str()
        .map(|s| s.parse::<f64>())
        .ok_or_else(|| Error::Internal(format!("price is not a f64, price: {}", price)))?
        .unwrap();

    let price_u64 = (price_f64 * PRICE_MULTIPLE).round() as u64;

    Ok(U256::from(price_u64))
}

pub fn get_request_costs(source: &str, json_rpc_payload: usize, max_response_bytes: u64) -> u128 {
    let ingress_bytes = (json_rpc_payload + source.len()) as u128 + INGRESS_OVERHEAD_BYTES;
    INGRESS_MESSAGE_RECEIVED_COST
        + INGRESS_MESSAGE_BYTE_RECEIVED_COST * ingress_bytes
        + HTTP_OUTCALL_REQUEST_COST
        + HTTP_OUTCALL_BYTE_RECEIVED_COST * (ingress_bytes + max_response_bytes as u128)
}
