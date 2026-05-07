import { FormEvent, useState } from "react";
import type { ChatMessage } from "../../types";

interface AiPanelProps {
  messages: ChatMessage[];
  loading: boolean;
  errorText: string;
  archiveSaving?: boolean;
  onSend: (message: string) => void;
  onSaveRound: () => void;
  onClose: () => void;
  exiting?: boolean;
}

export function AiPanel({
  messages,
  loading,
  errorText,
  archiveSaving = false,
  onSend,
  onSaveRound,
  onClose,
  exiting = false
}: AiPanelProps) {
  const [draft, setDraft] = useState("");

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    const message = draft.trim();
    if (!message || loading) {
      return;
    }

    setDraft("");
    onSend(message);
  };

  return (
    <section className={`panel ai-panel ${exiting ? "is-exiting" : ""}`} aria-label="AI 面板">
      <header>
        <span>AI</span>
        <button type="button" onClick={onClose}>
          Esc
        </button>
      </header>

      <div className="ai-history">
        {messages.length === 0 ? (
          <p className="muted">输入一句话，静桌会通过 DeepSeek 回复。</p>
        ) : (
          messages.map((message, index) => (
            <p className={`ai-message ai-message--${message.role}`} key={`${message.role}-${index}`}>
              {message.content}
            </p>
          ))
        )}
        {loading && <p className="muted">正在思考...</p>}
        {errorText && <p className="panel-error">{errorText}</p>}
      </div>

      <form className="ai-input-row" onSubmit={handleSubmit}>
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="问一句..."
          disabled={loading}
        />
        <button type="submit" disabled={loading}>
          发送
        </button>
      </form>

      <div className="ai-panel-actions">
        <button type="button" onClick={onSaveRound} disabled={archiveSaving || messages.length < 2}>
          {archiveSaving ? "保存中..." : "保存本轮"}
        </button>
      </div>
    </section>
  );
}
