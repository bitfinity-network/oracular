use std::cell::RefCell;
use std::rc::Rc;

use candid::Deserialize;
use ic_exports::ic_cdk::api::management_canister::http_request::{
    self, http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse,
    TransformArgs, TransformContext,
};
use ic_exports::ic_kit::ic;
use serde_json::{value::from_value, Value};
use url::Url;

use crate::context::Context;
use crate::error::{Error, Result};
use crate::state::Pair;

pub const PRICE_MULTIPLE: f64 = 1_0000_0000.0;

#[derive(Debug, Default, Deserialize)]
struct CoinbaseResBody {
    pub data: CoinBaseData,
}

#[derive(Debug, Default, Deserialize)]
struct CoinBaseData {
    pub base: String,
    pub currency: String,
    pub amount: String,
}

async fn http_outcall(
    url: String,
    method: HttpMethod,
    body: Option<Vec<u8>>,
    max_response_bytes: Option<u64>,
) -> Result<HttpResponse> {
    let real_url = Url::parse(&url).map_err(|e| Error::Http(e.to_string()))?;
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
        url,
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

pub async fn update_pair_price(context: &Rc<RefCell<dyn Context>>) -> Result<()> {
    let pairs = context.borrow().get_state().pair_storage().all_pairs();
    let mut futures = Vec::new();

    for pair in pairs {
        let url = get_coinbase_url(&pair);
        let context = context.clone();
        futures.push(async move {
            let res = http_outcall(url, HttpMethod::GET, None, Some(8000)).await?;

            if res.status != 200 {
                return Err(Error::Internal(format!(
                    "Pair doesn't exist, status: {}",
                    res.status
                )));
            }

            let json_body = serde_json::from_slice::<CoinbaseResBody>(&res.body)
                .map_err(|e| Error::Http(format!("serde_json err: {e}")))?;

            let price_f64 = json_body.data.amount.parse::<f64>().unwrap();
            let price_u64 = (price_f64 * PRICE_MULTIPLE).round() as u64;
            let base_currency = format!("{}-{}", json_body.data.base, json_body.data.currency);

            if base_currency != pair.to_string() {
                return Err(Error::Internal(
                    "http response's symbol isn't the pair key".to_string(),
                ));
            }

            context
                .borrow_mut()
                .mut_state()
                .mut_pair_storage()
                .update_pair(&pair.id(), price_u64, ic::time())?;

            Ok(price_u64)
        });
    }

    for future in futures {
        future.await?;
    }

    Ok(())
}

pub async fn check_pair_exist(pair: &Pair) -> Result<()> {
    let res = http_outcall(get_coinbase_url(pair), HttpMethod::GET, None, Some(8000)).await?;

    if res.status != 200 {
        return Err(Error::Internal(format!(
            "Pair doesn't exist, status: {}",
            res.status
        )));
    }

    Ok(())
}

pub fn get_coinbase_url(pair_key: &Pair) -> String {
    let mut base_url = "https://api.coinbase.com/v2/prices/".to_string();
    base_url.push_str(&pair_key.to_string());
    base_url.push_str("/spot");
    base_url
}
