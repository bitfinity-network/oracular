use did::U256;
use ic_exports::ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse, TransformArgs,
    TransformContext,
};

use serde_json::Value;
use url::Url;

use crate::error::{Error, Result};
use crate::parser::ValueParser;

pub const PRICE_MULTIPLE: f64 = 1_0000_0000.0;

async fn http_outcall(
    url: &str,
    method: HttpMethod,
    body: Option<Vec<u8>>,
    max_response_bytes: Option<u64>,
) -> Result<HttpResponse> {
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

pub fn transform(raw: TransformArgs) -> HttpResponse {
    HttpResponse {
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
