use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_SCAN_FILE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_TEXT_FILE_BYTES: u64 = 1024 * 1024;
const SKIP_DIRS: &[&str] = &[".git", ".next", "build", "dist", "node_modules", "target"];
const ALLOWED_EXTENSIONS: &[&str] = &[
    ".md", ".txt", ".json", ".ts", ".tsx", ".css", ".rs", ".toml", ".yaml", ".yml", ".html",
    ".xml",
];
const BLOCKED_PATTERNS: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    "id_rsa",
    "id_ed25519",
    "token",
    "secret",
    "password",
    "cookie",
    "cookies",
    "credential",
    "credentials",
    "key.pem",
    "private",
    ".ssh",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedRoot {
    pub id: String,
    pub path: String,
    pub label: String,
    pub created_at: String,
    pub last_indexed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIndexItem {
    pub id: String,
    pub root_id: String,
    pub path: String,
    pub relative_path: String,
    pub name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_at: String,
    pub kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileReadPolicy {
    pub max_text_file_bytes: u64,
    pub allowed_extensions: Vec<String>,
    pub blocked_patterns: Vec<String>,
}

pub fn app_data_root() -> Result<PathBuf, String> {
    if let Ok(appdata) = env::var("APPDATA") {
        Ok(PathBuf::from(appdata).join("Jingzhuo"))
    } else {
        env::current_dir()
            .map(|path| path.join("Jingzhuo"))
            .map_err(|err| format!("获取应用目录失败：{err}"))
    }
}

pub fn archive_root() -> Result<PathBuf, String> {
    Ok(app_data_root()?.join("archive"))
}

fn file_access_root() -> Result<PathBuf, String> {
    Ok(app_data_root()?.join("file-access"))
}

fn roots_path() -> Result<PathBuf, String> {
    Ok(file_access_root()?.join("authorized-roots.json"))
}

pub fn ensure_inside_app_archive(path: &Path) -> Result<(), String> {
    let root = archive_root()?;
    fs::create_dir_all(&root).map_err(|err| format!("创建留档目录失败：{err}"))?;
    let root = root
        .canonicalize()
        .map_err(|err| format!("解析留档目录失败：{err}"))?;
    let check_path = if path.exists() {
        path.canonicalize()
            .map_err(|err| format!("解析留档路径失败：{err}"))?
    } else {
        path.parent()
            .ok_or_else(|| "留档路径缺少父目录".to_string())?
            .canonicalize()
            .map_err(|err| format!("解析留档父目录失败：{err}"))?
    };

    if check_path.starts_with(&root) {
        Ok(())
    } else {
        Err("拒绝写入应用留档目录之外的路径".to_string())
    }
}

pub fn ensure_inside_authorized_root(path: &Path, roots: &[AuthorizedRoot]) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|err| format!("解析文件路径失败：{err}"))?;

    for root in roots {
        let root_path = PathBuf::from(&root.path)
            .canonicalize()
            .map_err(|err| format!("解析授权目录失败：{err}"))?;
        if path.starts_with(root_path) {
            return Ok(());
        }
    }

    Err("拒绝读取授权目录之外的文件".to_string())
}

pub fn is_blocked_path(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    BLOCKED_PATTERNS
        .iter()
        .any(|pattern| lower.contains(&pattern.to_lowercase()))
}

pub fn is_allowed_text_file(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    ALLOWED_EXTENSIONS.contains(&extension.as_str())
}

pub fn atomic_write_app_archive(path: &Path, content: &str) -> Result<(), String> {
    ensure_inside_app_archive(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建写入目录失败：{err}"))?;
    }

    let tmp = path.with_extension("tmp");
    ensure_inside_app_archive(&tmp)?;
    {
        let mut file = fs::File::create(&tmp).map_err(|err| format!("创建临时文件失败：{err}"))?;
        file.write_all(content.as_bytes())
            .map_err(|err| format!("写入临时文件失败：{err}"))?;
        file.sync_all()
            .map_err(|err| format!("同步临时文件失败：{err}"))?;
    }

    let mut backup_path = None;
    if path.exists() {
        let bak = path.with_extension(format!("bak.{}", timestamp_compact()));
        ensure_inside_app_archive(&bak)?;
        fs::rename(path, &bak).map_err(|err| format!("创建备份失败：{err}"))?;
        backup_path = Some(bak);
    }

    if let Err(err) = fs::rename(&tmp, path) {
        if let Some(bak) = backup_path {
            let _ = fs::rename(&bak, path);
        }
        return Err(format!("替换正式文件失败：{err}"));
    }

    Ok(())
}

fn read_roots() -> Result<Vec<AuthorizedRoot>, String> {
    let path = roots_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(path).map_err(|err| format!("读取授权目录失败：{err}"))?;
    serde_json::from_str(&raw).map_err(|err| format!("解析授权目录失败：{err}"))
}

fn write_roots(roots: &[AuthorizedRoot]) -> Result<(), String> {
    let dir = file_access_root()?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建授权目录配置失败：{err}"))?;
    let path = roots_path()?;
    let raw = serde_json::to_string_pretty(roots)
        .map_err(|err| format!("序列化授权目录失败：{err}"))?;
    fs::write(path, raw).map_err(|err| format!("写入授权目录失败：{err}"))
}

fn now_text() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}Z", duration.as_secs()))
        .unwrap_or_else(|_| "0Z".to_string())
}

fn timestamp_compact() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn make_id(prefix: &str) -> String {
    format!("{prefix}-{}", timestamp_compact())
}

fn is_bad_root(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    let parent_count = path.components().count();
    parent_count <= 1
        || lower.ends_with(":\\")
        || lower.ends_with("\\windows")
        || lower.ends_with("\\users")
        || lower.contains("\\.ssh")
        || lower.contains("\\appdata\\")
        || lower.contains("\\google\\chrome\\")
        || lower.contains("\\microsoft\\edge\\")
        || lower.contains("\\mozilla\\firefox\\")
}

fn skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().map(|value| value.to_string_lossy().to_lowercase()) else {
        return false;
    };
    name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) || is_blocked_path(path)
}

fn kind_from_extension(extension: &str) -> String {
    match extension {
        ".md" | ".txt" | ".json" | ".ts" | ".tsx" | ".css" | ".rs" | ".toml" | ".yaml"
        | ".yml" | ".html" | ".xml" => "text",
        ".png" | ".jpg" | ".jpeg" | ".gif" | ".webp" => "image",
        ".pdf" => "pdf",
        ".doc" | ".docx" | ".ppt" | ".pptx" | ".xls" | ".xlsx" => "office",
        _ => "unknown",
    }
    .to_string()
}

fn modified_text(metadata: &fs::Metadata) -> String {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| format!("{}Z", duration.as_secs()))
        .unwrap_or_else(|| "0Z".to_string())
}

fn scan_dir(root: &AuthorizedRoot, dir: &Path, items: &mut Vec<FileIndexItem>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !skip_dir(&path) {
                scan_dir(root, &path, items);
            }
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > MAX_SCAN_FILE_BYTES || is_blocked_path(&path) {
            continue;
        }

        let extension = path
            .extension()
            .map(|value| format!(".{}", value.to_string_lossy().to_lowercase()))
            .unwrap_or_default();
        let root_path = PathBuf::from(&root.path);
        let relative = path
            .strip_prefix(&root_path)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| relative.clone());

        items.push(FileIndexItem {
            id: format!("{}:{}", root.id, relative),
            root_id: root.id.clone(),
            path: path.to_string_lossy().to_string(),
            relative_path: relative,
            name,
            extension: extension.clone(),
            size_bytes: metadata.len(),
            modified_at: modified_text(&metadata),
            kind: kind_from_extension(&extension),
        });
    }
}

#[tauri::command]
pub fn file_read_policy() -> FileReadPolicy {
    FileReadPolicy {
        max_text_file_bytes: MAX_TEXT_FILE_BYTES,
        allowed_extensions: ALLOWED_EXTENSIONS.iter().map(|value| value.to_string()).collect(),
        blocked_patterns: BLOCKED_PATTERNS.iter().map(|value| value.to_string()).collect(),
    }
}

#[tauri::command]
pub fn file_list_authorized_roots() -> Result<Vec<AuthorizedRoot>, String> {
    read_roots()
}

#[tauri::command]
pub fn file_add_authorized_root(path: String, label: Option<String>) -> Result<AuthorizedRoot, String> {
    let canonical = PathBuf::from(path)
        .canonicalize()
        .map_err(|err| format!("授权目录不存在或无法访问：{err}"))?;
    if !canonical.is_dir() {
        return Err("授权路径必须是文件夹".to_string());
    }
    if is_bad_root(&canonical) || is_blocked_path(&canonical) {
        return Err("拒绝授权系统目录或敏感目录".to_string());
    }

    let mut roots = read_roots()?;
    let path_text = canonical.to_string_lossy().to_string();
    if let Some(existing) = roots.iter().find(|root| root.path == path_text) {
        return Ok(existing.clone());
    }

    let root = AuthorizedRoot {
        id: make_id("root"),
        path: path_text,
        label: label
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                canonical
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| "授权文件夹".to_string())
            }),
        created_at: now_text(),
        last_indexed_at: None,
    };
    roots.push(root.clone());
    write_roots(&roots)?;
    Ok(root)
}

#[tauri::command]
pub fn file_remove_authorized_root(root_id: String) -> Result<(), String> {
    let mut roots = read_roots()?;
    roots.retain(|root| root.id != root_id);
    write_roots(&roots)
}

#[tauri::command]
pub fn file_scan_authorized_root(root_id: String) -> Result<Vec<FileIndexItem>, String> {
    let mut roots = read_roots()?;
    let root = roots
        .iter()
        .find(|root| root.id == root_id)
        .cloned()
        .ok_or_else(|| "未找到授权目录".to_string())?;
    let root_path = PathBuf::from(&root.path)
        .canonicalize()
        .map_err(|err| format!("授权目录无法访问：{err}"))?;
    let mut items = Vec::new();
    scan_dir(&root, &root_path, &mut items);

    if let Some(item) = roots.iter_mut().find(|item| item.id == root_id) {
        item.last_indexed_at = Some(now_text());
    }
    write_roots(&roots)?;
    Ok(items)
}

#[tauri::command]
pub fn file_read_text(root_id: String, path: String) -> Result<String, String> {
    let roots = read_roots()?;
    let root = roots
        .iter()
        .find(|root| root.id == root_id)
        .ok_or_else(|| "未找到授权目录".to_string())?;
    let candidate = {
        let raw = PathBuf::from(&path);
        if raw.is_absolute() {
            raw
        } else {
            PathBuf::from(&root.path).join(raw)
        }
    };
    let candidate = candidate
        .canonicalize()
        .map_err(|err| format!("文件不存在或无法访问：{err}"))?;

    ensure_inside_authorized_root(&candidate, &[root.clone()])?;
    if is_blocked_path(&candidate) {
        return Err("拒绝读取敏感文件".to_string());
    }
    if !is_allowed_text_file(&candidate) {
        return Err("只允许读取授权目录内的小型文本文件".to_string());
    }

    let metadata = fs::metadata(&candidate).map_err(|err| format!("读取文件信息失败：{err}"))?;
    if metadata.len() > MAX_TEXT_FILE_BYTES {
        return Err("文件超过 1MB，已拒绝读取".to_string());
    }

    fs::read_to_string(&candidate).map_err(|err| format!("读取文本失败：{err}"))
}
