use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(alias = "weather_text")]
    pub weather_text: String,
    #[serde(alias = "focus_minutes")]
    pub focus_minutes: u32,
    #[serde(alias = "show_spectrum")]
    pub show_spectrum: bool,
    #[serde(alias = "wallpaper_status_text")]
    pub wallpaper_status_text: String,
    #[serde(
        alias = "default_wallpaper_folder",
        alias = "wallpaperDir",
        alias = "wallpaper_dir"
    )]
    pub default_wallpaper_folder: String,
    #[serde(
        alias = "alternate_wallpaper_folder",
        alias = "backupWallpaperDir",
        alias = "backup_wallpaper_dir"
    )]
    pub alternate_wallpaper_folder: String,
    #[serde(alias = "wallpaper_interval_ms")]
    pub wallpaper_interval_ms: u32,
    #[serde(alias = "deepseek_api_key")]
    pub deepseek_api_key: String,
    #[serde(alias = "deepseek_base_url")]
    pub deepseek_base_url: String,
    #[serde(alias = "deepseek_model")]
    pub deepseek_model: String,
    #[serde(alias = "ai_reply_mode")]
    pub ai_reply_mode: String,
    #[serde(alias = "weather_city_code")]
    pub weather_city_code: String,
    #[serde(alias = "weather_provider")]
    pub weather_provider: String,
    #[serde(alias = "weather_api_id")]
    pub weather_api_id: String,
    #[serde(alias = "weather_api_key")]
    pub weather_api_key: String,
    #[serde(alias = "weather_province")]
    pub weather_province: String,
    #[serde(alias = "weather_place")]
    pub weather_place: String,
    #[serde(alias = "weather_fallback_text")]
    pub weather_fallback_text: String,
    #[serde(alias = "smart_weather_app_id")]
    pub smart_weather_app_id: String,
    #[serde(alias = "smart_weather_private_key")]
    pub smart_weather_private_key: String,
    #[serde(alias = "last_weather_text")]
    pub last_weather_text: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            weather_text: "天气未更新".to_string(),
            focus_minutes: 25,
            show_spectrum: true,
            wallpaper_status_text: "背景".to_string(),
            default_wallpaper_folder: r"D:\BaiduNetdiskDownload\desk_1".to_string(),
            alternate_wallpaper_folder: r"D:\BaiduNetdiskDownload\desk_2".to_string(),
            wallpaper_interval_ms: 60_000,
            deepseek_api_key: String::new(),
            deepseek_base_url: "https://api.deepseek.com".to_string(),
            deepseek_model: "deepseek-chat".to_string(),
            ai_reply_mode: "normal".to_string(),
            weather_city_code: "101010100".to_string(),
            weather_provider: "apihz".to_string(),
            weather_api_id: String::new(),
            weather_api_key: String::new(),
            weather_province: "北京".to_string(),
            weather_place: "北京".to_string(),
            weather_fallback_text: "未更新".to_string(),
            smart_weather_app_id: String::new(),
            smart_weather_private_key: String::new(),
            last_weather_text: "天气未更新".to_string(),
            extra: BTreeMap::new(),
        }
    }
}

fn config_path() -> PathBuf {
    if let Ok(appdata) = env::var("APPDATA") {
        PathBuf::from(appdata)
            .join("Jingzhuo")
            .join("config.json")
    } else {
        PathBuf::from("jingzhuo.config.json")
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn loose_value(raw: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(&needle) {
            return None;
        }

        let (_, value) = trimmed.split_once(':')?;
        let value = value.trim().trim_end_matches(',').trim();
        if value.is_empty() || value == "null" {
            return None;
        }

        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            serde_json::from_str::<String>(value).ok()
        } else if let Some(value) = value.strip_prefix('"') {
            Some(value.trim_end_matches('"').to_string())
        } else {
            Some(value.trim_matches('"').to_string())
        }
    })
}

fn loose_string(raw: &str, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| loose_value(raw, key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn loose_u32(raw: &str, keys: &[&str]) -> Option<u32> {
    loose_string(raw, keys).and_then(|value| value.parse::<u32>().ok())
}

fn loose_bool(raw: &str, keys: &[&str]) -> Option<bool> {
    loose_string(raw, keys).and_then(|value| match value.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    })
}

fn recover_config_from_loose_json(raw: &str) -> Option<AppConfig> {
    let mut config = AppConfig::default();
    let mut recovered = false;

    macro_rules! set_string {
        ($field:ident, [$($key:expr),+]) => {
            if let Some(value) = loose_string(raw, &[$($key),+]) {
                config.$field = value;
                recovered = true;
            }
        };
    }

    if let Some(value) = loose_u32(raw, &["focusMinutes", "focus_minutes"]) {
        config.focus_minutes = value;
        recovered = true;
    }
    if let Some(value) = loose_bool(raw, &["showSpectrum", "show_spectrum"]) {
        config.show_spectrum = value;
        recovered = true;
    }
    if let Some(value) = loose_u32(raw, &["wallpaperIntervalMs", "wallpaper_interval_ms"]) {
        config.wallpaper_interval_ms = value;
        recovered = true;
    }

    set_string!(weather_text, ["weatherText", "weather_text"]);
    set_string!(wallpaper_status_text, ["wallpaperStatusText", "wallpaper_status_text"]);
    set_string!(
        default_wallpaper_folder,
        ["defaultWallpaperFolder", "default_wallpaper_folder", "wallpaperDir", "wallpaper_dir"]
    );
    set_string!(
        alternate_wallpaper_folder,
        [
            "alternateWallpaperFolder",
            "alternate_wallpaper_folder",
            "backupWallpaperDir",
            "backup_wallpaper_dir"
        ]
    );
    set_string!(deepseek_api_key, ["deepseekApiKey", "deepseek_api_key"]);
    set_string!(deepseek_base_url, ["deepseekBaseUrl", "deepseek_base_url"]);
    set_string!(deepseek_model, ["deepseekModel", "deepseek_model"]);
    set_string!(ai_reply_mode, ["aiReplyMode", "ai_reply_mode"]);
    set_string!(weather_city_code, ["weatherCityCode", "weather_city_code"]);
    set_string!(weather_provider, ["weatherProvider", "weather_provider"]);
    set_string!(weather_api_id, ["weatherApiId", "weather_api_id"]);
    set_string!(weather_api_key, ["weatherApiKey", "weather_api_key"]);
    set_string!(weather_province, ["weatherProvince", "weather_province"]);
    set_string!(weather_place, ["weatherPlace", "weather_place"]);
    set_string!(
        weather_fallback_text,
        ["weatherFallbackText", "weather_fallback_text", "weatherText", "weather_text"]
    );
    set_string!(smart_weather_app_id, ["smartWeatherAppId", "smart_weather_app_id"]);
    set_string!(
        smart_weather_private_key,
        ["smartWeatherPrivateKey", "smart_weather_private_key"]
    );
    set_string!(last_weather_text, ["lastWeatherText", "last_weather_text"]);

    recovered.then(|| normalize_config(config, None))
}

fn normalize_config(mut config: AppConfig, raw: Option<&Value>) -> AppConfig {
    let defaults = AppConfig::default();

    if !matches!(config.ai_reply_mode.as_str(), "normal" | "expert" | "night") {
        config.ai_reply_mode = defaults.ai_reply_mode;
    }

    if config.weather_provider.trim().is_empty() {
        config.weather_provider = defaults.weather_provider;
    }

    if config.weather_fallback_text.trim().is_empty() {
        config.weather_fallback_text = defaults.weather_fallback_text.clone();
    }

    if let Some(raw) = raw {
        let has_new_fallback =
            raw.get("weatherFallbackText").is_some() || raw.get("weather_fallback_text").is_some();
        if !has_new_fallback && config.weather_fallback_text == defaults.weather_fallback_text {
            if let Some(text) = string_field(raw, &["weatherText", "weather_text"]) {
                config.weather_fallback_text = text.to_string();
            }
        }
    }

    config
}

fn read_existing_config(path: &Path) -> Result<Option<AppConfig>, String> {
    match fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(value) => {
                let parsed: AppConfig = serde_json::from_value(value.clone())
                    .map_err(|err| format!("配置字段解析失败，已停止读取：{err}"))?;
                Ok(Some(normalize_config(parsed, Some(&value))))
            }
            Err(err) => recover_config_from_loose_json(&raw)
                .map(Some)
                .ok_or_else(|| format!("配置文件解析失败，已停止读取：{err}")),
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("配置文件读取失败：{err}")),
    }
}

fn nonempty_critical_count(config: &AppConfig) -> usize {
    [
        config.deepseek_api_key.as_str(),
        config.deepseek_base_url.as_str(),
        config.deepseek_model.as_str(),
        config.weather_api_id.as_str(),
        config.weather_api_key.as_str(),
        config.weather_province.as_str(),
        config.weather_place.as_str(),
        config.default_wallpaper_folder.as_str(),
        config.alternate_wallpaper_folder.as_str(),
    ]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .count()
}

fn has_empty_form_overwrite_risk(old: &AppConfig, new_config: &AppConfig) -> bool {
    let old_count = nonempty_critical_count(old);
    let new_count = nonempty_critical_count(new_config);
    old_count >= 5 && new_count + 3 < old_count
}

fn write_config_atomic(path: &Path, raw: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("配置目录创建失败：{err}"))?;
    }

    let stamp = timestamp();
    let tmp_path = path.with_file_name("config.json.tmp");
    let backup_path = path.with_file_name(format!("config.json.bak.{stamp}"));
    let replaced_path = path.with_file_name(format!("config.json.replaced.{stamp}"));

    fs::write(&tmp_path, raw).map_err(|err| format!("临时配置写入失败：{err}"))?;

    if path.exists() {
        fs::copy(path, &backup_path).map_err(|err| format!("配置备份失败：{err}"))?;
        fs::rename(path, &replaced_path).map_err(|err| format!("旧配置暂存失败：{err}"))?;
    }

    if let Err(err) = fs::rename(&tmp_path, path) {
        if replaced_path.exists() {
            let _ = fs::rename(&replaced_path, path);
        }
        return Err(format!("配置替换失败：{err}"));
    }

    Ok(())
}

#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    let path = config_path();
    read_existing_config(&path).map(|config| config.unwrap_or_default())
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    let path = config_path();
    let next = normalize_config(config, None);

    if let Some(old) = read_existing_config(&path)? {
        if has_empty_form_overwrite_risk(&old, &next) {
            return Err("配置尚未正确加载，已阻止覆盖旧配置".to_string());
        }
    }

    let raw = serde_json::to_string_pretty(&next).map_err(|err| format!("配置序列化失败：{err}"))?;
    write_config_atomic(&path, &raw)
}
