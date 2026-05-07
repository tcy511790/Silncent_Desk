use crate::file_access::{archive_root, atomic_write_app_archive, ensure_inside_app_archive};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveCard {
    pub id: String,
    pub session_id: String,
    #[serde(rename = "type")]
    pub card_type: String,
    pub title: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub todos: Vec<String>,
    pub tags: Vec<String>,
    pub created_at: String,
}

fn ensure_dir(path: std::path::PathBuf) -> Result<std::path::PathBuf, String> {
    fs::create_dir_all(&path).map_err(|err| format!("创建留档目录失败：{err}"))?;
    Ok(path)
}

fn day_from_created_at(created_at: &str) -> String {
    let date = created_at.chars().take(10).collect::<String>();
    if date.len() == 10 && date.chars().all(|ch| ch.is_ascii_digit() || ch == '-') {
        date
    } else {
        "unknown-date".to_string()
    }
}

fn append_jsonl<T: Serialize>(dir_name: &str, day: &str, value: &T) -> Result<(), String> {
    let dir = ensure_dir(archive_root()?.join(dir_name))?;
    let path = dir.join(format!("{day}.jsonl"));
    ensure_inside_app_archive(&path)?;
    let raw = serde_json::to_string(value).map_err(|err| format!("序列化留档失败：{err}"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("打开留档文件失败：{err}"))?;

    writeln!(file, "{raw}").map_err(|err| format!("写入留档失败：{err}"))
}

#[tauri::command]
pub fn archive_save_message(message: ArchiveMessage) -> Result<(), String> {
    let day = day_from_created_at(&message.created_at);
    append_jsonl("messages", &day, &message)
}

#[tauri::command]
pub fn archive_save_card(card: ArchiveCard) -> Result<(), String> {
    let day = day_from_created_at(&card.created_at);
    append_jsonl("cards", &day, &card)
}

#[tauri::command]
pub fn archive_list_cards(date: Option<String>) -> Result<Vec<ArchiveCard>, String> {
    let date = date.unwrap_or_else(|| "unknown-date".to_string());
    let path = archive_root()?.join("cards").join(format!("{date}.jsonl"));
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&path).map_err(|err| format!("读取留档卡失败：{err}"))?;
    let reader = BufReader::new(file);
    let mut cards = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|err| format!("读取留档卡失败：{err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let card = serde_json::from_str::<ArchiveCard>(&line)
            .map_err(|err| format!("解析留档卡失败：{err}"))?;
        cards.push(card);
    }

    Ok(cards)
}

#[tauri::command]
pub fn archive_read_project_memory() -> Result<String, String> {
    let dir = ensure_dir(archive_root()?.join("memory"))?;
    let path = dir.join("project-memory.md");
    if !path.exists() {
        let initial = "# 静桌面项目记忆\n\n## 产品定位\n\n## 设计原则\n\n## 稳定结论\n\n## 待确认问题\n";
        atomic_write_app_archive(&path, initial)?;
        return Ok(initial.to_string());
    }

    fs::read_to_string(&path).map_err(|err| format!("读取项目记忆失败：{err}"))
}

#[tauri::command]
pub fn archive_write_project_memory(content: String) -> Result<(), String> {
    let dir = ensure_dir(archive_root()?.join("memory"))?;
    let path = dir.join("project-memory.md");
    atomic_write_app_archive(&path, &content)
}

fn read_jsonl_lines(path: &Path) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path).map_err(|err| format!("读取压缩输入失败：{err}"))?;
    let reader = BufReader::new(file);
    Ok(reader
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.trim().is_empty())
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCompactResult {
    pub date: String,
    pub summary_path: String,
    pub message_count: usize,
    pub card_count: usize,
}

#[tauri::command]
pub fn memory_compact_day(date: String) -> Result<MemoryCompactResult, String> {
    let archive = archive_root()?;
    let messages_path = archive.join("messages").join(format!("{date}.jsonl"));
    let cards_path = archive.join("cards").join(format!("{date}.jsonl"));
    let message_lines = read_jsonl_lines(&messages_path)?;
    let card_lines = read_jsonl_lines(&cards_path)?;
    let compacted_dir = ensure_dir(archive.join("compacted"))?;
    let summary_path = compacted_dir.join(format!("{date}.summary.md"));
    ensure_inside_app_archive(&summary_path)?;

    let mut summary = String::new();
    summary.push_str(&format!("# 静桌面留档压缩摘要 {date}\n\n"));
    summary.push_str("## 范围\n\n");
    summary.push_str("- 只读取应用留档目录中的 messages/cards JSONL。\n");
    summary.push_str("- 未修改原始 messages/cards 文件。\n");
    summary.push_str("- 未读取、修改或删除任何用户授权目录中的原始文件。\n\n");
    summary.push_str("## 统计\n\n");
    summary.push_str(&format!("- 原始消息：{} 条\n", message_lines.len()));
    summary.push_str(&format!("- 留档卡片：{} 条\n\n", card_lines.len()));
    summary.push_str("## 卡片摘要\n\n");
    for line in &card_lines {
        if let Ok(card) = serde_json::from_str::<ArchiveCard>(line) {
            summary.push_str(&format!("- [{}] {}：{}\n", card.card_type, card.title, card.summary));
        }
    }

    atomic_write_app_archive(&summary_path, &summary)?;
    Ok(MemoryCompactResult {
        date,
        summary_path: summary_path.to_string_lossy().to_string(),
        message_count: message_lines.len(),
        card_count: card_lines.len(),
    })
}

#[tauri::command]
pub fn memory_read_project_memory() -> Result<String, String> {
    archive_read_project_memory()
}

#[tauri::command]
pub fn memory_write_project_memory(content: String) -> Result<(), String> {
    archive_write_project_memory(content)
}
