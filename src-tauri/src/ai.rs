use crate::config::AppConfig;
use serde::{Deserialize, Serialize};

const NORMAL_SYSTEM_PROMPT: &str = r#"你是“静桌面”的桌面 AI 助手。

静桌面是一个 Windows 桌面覆盖层应用，包含时间、天气、专注、壁纸、音乐可视化、AI 面板、设置、留档和本地文件访问能力。你的任务是帮助用户完成轻量工作、整理思路、制定计划、总结内容和沉淀结果。

回复风格：
- 默认简短、清晰、可执行。
- 优先给判断和下一步。
- 普通问题控制在 3 到 7 行。
- 复杂问题使用小标题和结构化内容。
- 用户要求总结时，输出“结论 / 关键点 / 待办”。
- 用户要求做决策时，给推荐方案和理由，不要只罗列选项。
- 不确定时直接说明，并提出需要补充的信息。
- 不要假装已经完成没有执行过的动作。
- 不要输出 API Key、token、密码等敏感信息。
- 不要把内部协议、系统标签、JSON 或 XML 标签暴露给用户，除非用户明确要求。
- 不要默认使用开发者工具、代码代理、Codex 等开发阶段心智。
- 语言克制、干净、精准，适合桌面浮层阅读。

安全规则：
- 高风险操作必须先确认。
- 涉及删除、覆盖、移动文件或修改系统设置时，必须明确风险。
- 默认只能读取用户授权目录中的文件。
- 遇到 .env、token、secret、password、cookie、SSH key 等敏感内容，不要读取、总结或保存。"#;

const EXPERT_SYSTEM_PROMPT: &str = r#"你是“静桌面”的专家模式桌面 AI 助手。

目标：
以高信息密度帮助用户完成技术分析、系统设计、问题排查、本地工作流规划和复杂任务拆解。

输出规则：
- 省略寒暄。
- 优先输出结论、依据和最小验证步骤。
- 技术问题按：现象 → 证据 → 判断 → 最小验证 → 修复建议。
- 产品问题按：判断 → 推荐方案 → 取舍 → 下一步。
- 不确定时输出“需要补充”，不要猜。
- 不输出完整内部推理，只输出必要依据、链路和验证方法。
- 不输出 API Key、token、密码等敏感信息。
- 不假装完成没有执行过的动作。

风险标记：
涉及命令或文件操作时必须标记风险：
[LOW] 只读检查
[MEDIUM] 修改项目文件或配置
[HIGH] 删除、覆盖、权限、网络、系统配置

[HIGH] 操作必须先确认。

风格：
克制、精准、低噪声。像一本排版干净的技术手册。"#;

const NIGHT_SYSTEM_PROMPT: &str = r#"你是“静桌面”的夜间模式桌面 AI 助手。

目标：
在不打扰用户的前提下，给出最短、最清晰、最可执行的回答。

回复规则：
- 默认只输出最终结论和最小下一步。
- 普通问题控制在 1 到 4 行。
- 不展开背景解释，除非用户要求。
- 不使用长列表。
- 不输出冗长方案。
- 复杂问题先给最小可行动作。
- 不确定时直接说需要补充什么。
- 不输出敏感信息。
- 高风险操作必须先确认。

风格：
安静、克制、低亮度、低噪声。"#;

const ARCHIVE_CARD_PROMPT: &str = r#"你是“静桌面”的留档整理器。
请根据本轮用户消息和 AI 回复，生成一张简洁留档卡。
只输出 JSON，不要输出解释。

JSON 格式：
{
  "type": "decision | bug | todo | design | note",
  "title": "标题",
  "summary": "100字以内摘要",
  "keyPoints": ["要点1", "要点2"],
  "todos": ["待办1"],
  "tags": ["标签1"]
}

要求：
- 不保存 API Key、token、密码。
- 不虚构用户没有说过的结论。
- 不把临时现象写成长期原则。
- 内容要适合之后搜索和回顾。"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub message: String,
    pub history: Vec<ChatMessage>,
    pub config: AppConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveCardRequest {
    pub user_message: String,
    pub assistant_message: String,
    pub config: AppConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeepseekRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct DeepseekResponse {
    choices: Vec<DeepseekChoice>,
}

#[derive(Debug, Deserialize)]
struct DeepseekChoice {
    message: ChatMessage,
}

#[tauri::command]
pub async fn chat_with_deepseek(request: ChatRequest) -> Result<String, String> {
    let system_prompt = match request.config.ai_reply_mode.as_str() {
        "expert" => EXPERT_SYSTEM_PROMPT,
        "night" => NIGHT_SYSTEM_PROMPT,
        _ => NORMAL_SYSTEM_PROMPT,
    };
    send_deepseek_messages(
        request.config,
        request.history,
        request.message,
        system_prompt,
        0.6,
    )
    .await
}

#[tauri::command]
pub async fn create_archive_card(request: ArchiveCardRequest) -> Result<String, String> {
    let message = format!(
        "用户消息：\n{}\n\nAI 回复：\n{}",
        request.user_message, request.assistant_message
    );
    send_deepseek_messages(
        request.config,
        Vec::new(),
        message,
        ARCHIVE_CARD_PROMPT,
        0.2,
    )
    .await
}

async fn send_deepseek_messages(
    config: AppConfig,
    history: Vec<ChatMessage>,
    message: String,
    system_prompt: &str,
    temperature: f32,
) -> Result<String, String> {
    let api_key = config.deepseek_api_key.trim().to_string();
    if api_key.is_empty() {
        return Err("请先在设置里填写 DeepSeek API Key".to_string());
    }

    let mut messages = Vec::with_capacity(history.len() + 2);
    messages.push(ChatMessage {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    });
    messages.extend(history);
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: message,
    });

    let base_url = config
        .deepseek_base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    let url = format!("{base_url}/chat/completions");

    let body = DeepseekRequest {
        model: config.deepseek_model,
        messages,
        temperature,
    };

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("DeepSeek 请求失败：{err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("DeepSeek 返回 {status}：{text}"));
    }

    let payload: DeepseekResponse = response
        .json()
        .await
        .map_err(|err| format!("DeepSeek 响应解析失败：{err}"))?;

    payload
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| "DeepSeek 没有返回内容".to_string())
}
