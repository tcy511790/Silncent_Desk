import { useMemo, useState } from "react";
import type { ArchiveCard } from "../../types";

interface ArchivePanelProps {
  cards: ArchiveCard[];
  loading: boolean;
  errorText: string;
  onRefresh: () => void;
  onClose: () => void;
  onCopied?: () => void;
  exiting?: boolean;
}

function safeArray(value: unknown) {
  return Array.isArray(value) ? value.map((item) => String(item)).filter(Boolean) : [];
}

function formatDateTime(value: string | undefined) {
  if (!value) {
    return "";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }

  return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(
    2,
    "0"
  )}`;
}

function buildSummaryText(card: ArchiveCard) {
  const title = card.title || "未命名留档";
  const summary = card.summary || "暂无摘要";
  const keyPoints = safeArray(card.keyPoints);
  const todos = safeArray(card.todos);

  return [
    title,
    "",
    summary,
    keyPoints.length > 0 ? `\n关键点\n${keyPoints.map((item) => `- ${item}`).join("\n")}` : "",
    todos.length > 0 ? `\n待办\n${todos.map((item) => `- ${item}`).join("\n")}` : ""
  ]
    .filter(Boolean)
    .join("\n");
}

export function ArchivePanel({
  cards,
  loading,
  errorText,
  onRefresh,
  onClose,
  onCopied,
  exiting = false
}: ArchivePanelProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const needle = filter.trim().toLowerCase();

  const filteredCards = useMemo(() => {
    if (!needle) {
      return cards;
    }

    return cards.filter((card) => {
      const tags = safeArray(card.tags);
      return `${card.type || ""} ${card.title || ""} ${card.summary || ""} ${tags.join(" ")}`
        .toLowerCase()
        .includes(needle);
    });
  }, [cards, needle]);

  const copySummary = (card: ArchiveCard) => {
    navigator.clipboard
      ?.writeText(buildSummaryText(card))
      .then(() => onCopied?.())
      .catch(() => undefined);
  };

  return (
    <section className={`panel archive-panel ${exiting ? "is-exiting" : ""}`} aria-label="留档">
      <div className="archive-header">
        <h2>留档</h2>
        <div className="archive-actions">
          <button type="button" onClick={onRefresh} disabled={loading}>
            刷新
          </button>
          <button type="button" onClick={onClose}>
            Esc
          </button>
        </div>
      </div>

      <input
        className="archive-search"
        value={filter}
        onChange={(event) => setFilter(event.target.value)}
        placeholder="筛选今天的留档..."
      />

      {errorText && <p className="panel-error">留档读取失败：{errorText}</p>}
      {loading && <p className="muted archive-status">正在读取留档...</p>}

      <div className="archive-list">
        {filteredCards.map((card, index) => {
          const cardId = card.id || `${card.createdAt || "archive"}-${index}`;
          const expanded = expandedId === cardId;
          const title = card.title || "未命名留档";
          const summary = card.summary || "暂无摘要";
          const keyPoints = safeArray(card.keyPoints);
          const todos = safeArray(card.todos);
          const tags = safeArray(card.tags);
          const createdAt = formatDateTime(card.createdAt);

          return (
            <article className="archive-card" key={cardId}>
              <div className="archive-card-meta">
                <span className="archive-badge">{card.type || "note"}</span>
                {createdAt && <time>{createdAt}</time>}
              </div>

              <h3>{title}</h3>
              <p>{summary}</p>

              {tags.length > 0 && (
                <div className="archive-tags">
                  {tags.map((tag) => (
                    <span className="archive-tag" key={tag}>
                      #{tag}
                    </span>
                  ))}
                </div>
              )}

              <div className="archive-card-actions">
                <button type="button" onClick={() => setExpandedId(expanded ? null : cardId)}>
                  {expanded ? "收起详情" : "展开详情"}
                </button>
                <button type="button" onClick={() => copySummary(card)}>
                  复制摘要
                </button>
              </div>

              {expanded && (
                <div className="archive-detail">
                  {keyPoints.length > 0 && (
                    <section>
                      <h4>关键点</h4>
                      <ul>
                        {keyPoints.map((item, itemIndex) => (
                          <li key={`${item}-${itemIndex}`}>{item}</li>
                        ))}
                      </ul>
                    </section>
                  )}

                  {todos.length > 0 && (
                    <section>
                      <h4>待办</h4>
                      <ul>
                        {todos.map((item, itemIndex) => (
                          <li key={`${item}-${itemIndex}`}>{item}</li>
                        ))}
                      </ul>
                    </section>
                  )}

                  {keyPoints.length === 0 && todos.length === 0 && (
                    <p className="archive-detail-empty">暂无详情。</p>
                  )}
                </div>
              )}
            </article>
          );
        })}

        {!loading && filteredCards.length === 0 && (
          <div className="archive-empty">
            <strong>今天还没有留档。</strong>
            <span>你可以在 AI 回复后点击“保存本轮”。</span>
          </div>
        )}
      </div>
    </section>
  );
}
