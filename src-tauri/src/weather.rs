use crate::config::AppConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const APIHZ_WEATHER_URL: &str = "https://cn.apihz.cn/api/tianqi/tqyb.php";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherResult {
    pub text: String,
    pub city: Option<String>,
    pub province: Option<String>,
    pub temperature: Option<f32>,
    pub humidity: Option<String>,
    pub wind_direction: Option<String>,
    pub wind_scale: Option<String>,
    pub condition: Option<String>,
    pub updated_at: Option<String>,
    pub source: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApihzWeatherResponse {
    code: Value,
    msg: Option<String>,
    sheng: Option<String>,
    shi: Option<String>,
    name: Option<String>,
    uptime: Option<String>,
    weather1: Option<String>,
    weather2: Option<String>,
    wd1: Option<Value>,
    wd2: Option<Value>,
    nowinfo: Option<ApihzNowInfo>,
}

#[derive(Debug, Deserialize)]
struct ApihzNowInfo {
    temperature: Option<Value>,
    humidity: Option<Value>,
    #[serde(rename = "windDirection")]
    wind_direction: Option<String>,
    #[serde(rename = "windScale")]
    wind_scale: Option<String>,
    uptime: Option<String>,
}

fn clean_text(value: Option<String>) -> Option<String> {
    value.map(|text| text.trim().to_string()).filter(|text| !text.is_empty())
}

fn value_to_text(value: Option<Value>) -> Option<String> {
    match value? {
        Value::String(text) => clean_text(Some(text)),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn fallback_text(config: &AppConfig) -> String {
    config.weather_fallback_text.trim().to_string()
}

fn empty_weather_result(text: String, source: &str, error: Option<String>) -> WeatherResult {
    WeatherResult {
        text,
        city: None,
        province: None,
        temperature: None,
        humidity: None,
        wind_direction: None,
        wind_scale: None,
        condition: None,
        updated_at: None,
        source: source.to_string(),
        error,
    }
}

fn fallback_result(config: &AppConfig, error: Option<String>) -> WeatherResult {
    let fallback = fallback_text(config);

    if !fallback.is_empty() && fallback != "未更新" {
        return empty_weather_result(fallback, "fallback", error);
    }

    empty_weather_result("天气未更新".to_string(), "none", error)
}

fn code_is_success(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_i64() == Some(200),
        Value::String(text) => text == "200",
        _ => false,
    }
}

fn parse_f32(value: Option<Value>) -> Option<f32> {
    match value? {
        Value::Number(number) => number.as_f64().map(|value| value as f32),
        Value::String(text) => parse_number_from_text(&text),
        _ => None,
    }
}

fn parse_number_from_text(text: &str) -> Option<f32> {
    let mut started = false;
    let mut number = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' || (ch == '-' && !started) {
            started = true;
            number.push(ch);
        } else if started {
            break;
        }
    }

    number.parse::<f32>().ok()
}

fn condition_text(weather1: Option<String>, weather2: Option<String>) -> Option<String> {
    let first = clean_text(weather1);
    let second = clean_text(weather2);

    match (first, second) {
        (Some(a), Some(b)) if a == b => Some(a),
        (Some(a), Some(b)) => Some(format!("{a}转{b}")),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn masked_id(api_id: &str) -> String {
    let chars: Vec<char> = api_id.chars().collect();
    if chars.len() <= 4 {
        return "****".to_string();
    }

    let prefix: String = chars.iter().take(2).collect();
    let suffix: String = chars.iter().skip(chars.len() - 2).collect();
    format!("{prefix}****{suffix}")
}

#[tauri::command]
pub async fn refresh_weather(config: AppConfig) -> Result<WeatherResult, String> {
    let api_id = config.weather_api_id.trim();
    let api_key = config.weather_api_key.trim();
    let province = config.weather_province.trim();
    let place = config.weather_place.trim();

    if api_id.is_empty() || api_key.is_empty() || province.is_empty() || place.is_empty() {
        eprintln!("[weather] failed provider=apihz reason=incomplete-config");
        return Ok(fallback_result(&config, Some("接口盒子天气配置不完整".to_string())));
    }

    eprintln!(
        "[weather] request provider=apihz id={} sheng={} place={}",
        masked_id(api_id),
        province,
        place
    );

    let client = reqwest::Client::new();
    let response = match client
        .get(APIHZ_WEATHER_URL)
        .query(&[
            ("id", api_id),
            ("key", api_key),
            ("sheng", province),
            ("place", place),
            ("day", "1"),
            ("hourtype", "0"),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            let error = format!("接口盒子天气请求失败：{err}");
            eprintln!("[weather] failed request error={error}");
            return Ok(fallback_result(&config, Some(error)));
        }
    };

    let payload = match response.json::<ApihzWeatherResponse>().await {
        Ok(payload) => payload,
        Err(err) => {
            let error = format!("接口盒子天气响应解析失败：{err}");
            eprintln!("[weather] failed parse error={error}");
            return Ok(fallback_result(&config, Some(error)));
        }
    };

    if !code_is_success(&payload.code) {
        let message = payload
            .msg
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "接口盒子天气接口返回错误".to_string());
        eprintln!("[weather] failed code={:?} msg={}", payload.code, message);
        return Ok(fallback_result(&config, Some(message)));
    }

    let nowinfo = payload.nowinfo;
    let now_temperature = nowinfo
        .as_ref()
        .and_then(|info| parse_f32(info.temperature.clone()));
    let forecast_temperature = parse_f32(payload.wd1.clone()).or_else(|| parse_f32(payload.wd2.clone()));
    let temperature = now_temperature.or(forecast_temperature);

    let Some(temperature) = temperature else {
        eprintln!("[weather] failed code=200 msg=missing-temperature");
        return Ok(fallback_result(
            &config,
            Some("接口盒子天气缺少温度字段".to_string()),
        ));
    };

    let city = clean_text(payload.name)
        .or_else(|| clean_text(payload.shi))
        .unwrap_or_else(|| place.to_string());
    let province_text = clean_text(payload.sheng).unwrap_or_else(|| province.to_string());
    let condition = condition_text(payload.weather1, payload.weather2);
    let text = match &condition {
        Some(condition) => format!("{city} {:.0}° · {condition}", temperature.round()),
        None => format!("{city} {:.0}°", temperature.round()),
    };
    let humidity = nowinfo
        .as_ref()
        .and_then(|info| value_to_text(info.humidity.clone()));
    let wind_direction = nowinfo.as_ref().and_then(|info| clean_text(info.wind_direction.clone()));
    let wind_scale = nowinfo.as_ref().and_then(|info| clean_text(info.wind_scale.clone()));
    let updated_at = nowinfo
        .as_ref()
        .and_then(|info| clean_text(info.uptime.clone()))
        .or_else(|| clean_text(payload.uptime));

    eprintln!(
        "[weather] response code=200 city={} temperature={:.0}",
        city,
        temperature.round()
    );

    Ok(WeatherResult {
        text,
        city: Some(city),
        province: Some(province_text),
        temperature: Some(temperature),
        humidity,
        wind_direction,
        wind_scale,
        condition,
        updated_at,
        source: "apihz".to_string(),
        error: None,
    })
}
