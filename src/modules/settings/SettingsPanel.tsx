import { ChangeEvent, useEffect, useState } from "react";
import type { AppConfig, AuthorizedRoot } from "../../types";

type SettingsTab = "ai" | "weather" | "wallpaper" | "display" | "files";

interface SettingsPanelProps {
  config: AppConfig;
  configLoaded: boolean;
  loadConfigError: string;
  authorizedRoots: AuthorizedRoot[];
  fileAccessPath: string;
  fileAccessStatus: string;
  onSave: (config: AppConfig) => void;
  onFileAccessPathChange: (path: string) => void;
  onAddAuthorizedRoot: () => void;
  onRemoveAuthorizedRoot: (rootId: string) => void;
  onScanAuthorizedRoot: (rootId: string) => void;
  onRefreshWeather: (config: AppConfig) => void;
  onClose: () => void;
  exiting?: boolean;
}

const tabs: Array<{ id: SettingsTab; label: string }> = [
  { id: "ai", label: "AI" },
  { id: "weather", label: "天气" },
  { id: "wallpaper", label: "壁纸" },
  { id: "display", label: "显示" },
  { id: "files", label: "文件" }
];

export function SettingsPanel({
  config,
  configLoaded,
  loadConfigError,
  authorizedRoots,
  fileAccessPath,
  fileAccessStatus,
  onSave,
  onFileAccessPathChange,
  onAddAuthorizedRoot,
  onRemoveAuthorizedRoot,
  onScanAuthorizedRoot,
  onRefreshWeather,
  onClose,
  exiting = false
}: SettingsPanelProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("ai");
  const [draft, setDraft] = useState(config);
  const saveDisabled = !configLoaded || Boolean(loadConfigError);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const update = <K extends keyof AppConfig>(key: K, value: AppConfig[K]) => {
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const updateText =
    (key: keyof AppConfig) => (event: ChangeEvent<HTMLInputElement>) =>
      update(key, event.target.value as never);

  const updateSelect =
    (key: keyof AppConfig) => (event: ChangeEvent<HTMLSelectElement>) =>
      update(key, event.target.value as never);

  return (
    <section className={`panel settings-panel ${exiting ? "is-exiting" : ""}`} aria-label="设置">
      <header>
        <span>设置</span>
        <div className="settings-header-actions">
          <button type="button" onClick={() => onSave(draft)} disabled={saveDisabled}>
            保存设置
          </button>
          <button type="button" onClick={onClose}>
            Esc
          </button>
        </div>
      </header>

      {saveDisabled && (
        <p className="settings-load-error">
          {loadConfigError || "配置正在加载，暂不能保存，避免空表单覆盖旧配置。"}
        </p>
      )}

      <div className="settings-tabs" role="tablist">
        {tabs.map((tab) => (
          <button
            className={`settings-tab ${activeTab === tab.id ? "is-active" : ""}`}
            key={tab.id}
            type="button"
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="settings-body">
        {activeTab === "ai" && (
          <div className="settings-grid">
            <label>
              <span>DeepSeek API Key</span>
              <input
                type="password"
                value={draft.deepseekApiKey}
                onChange={updateText("deepseekApiKey")}
                placeholder="sk-..."
              />
            </label>
            <label>
              <span>DeepSeek Base URL</span>
              <input value={draft.deepseekBaseUrl} onChange={updateText("deepseekBaseUrl")} />
            </label>
            <label>
              <span>DeepSeek 模型</span>
              <input value={draft.deepseekModel} onChange={updateText("deepseekModel")} />
            </label>
            <label>
              <span>AI 回复模式</span>
              <select
                value={draft.aiReplyMode ?? "normal"}
                onChange={updateSelect("aiReplyMode")}
              >
                <option value="normal">普通</option>
                <option value="expert">专家</option>
                <option value="night">夜间</option>
              </select>
              <small className="setting-help">
                普通：简短清晰。专家：高信息密度。夜间：极简低打扰。
              </small>
            </label>
          </div>
        )}

        {activeTab === "weather" && (
          <>
            <div className="settings-grid">
              <label>
                <span>天气 API 来源</span>
                <input value="接口盒子" readOnly />
              </label>
              <label>
                <span>接口盒子用户 ID</span>
                <input value={draft.weatherApiId ?? ""} onChange={updateText("weatherApiId")} />
              </label>
              <label>
                <span>接口盒子用户 KEY</span>
                <input
                  type="password"
                  value={draft.weatherApiKey ?? ""}
                  onChange={updateText("weatherApiKey")}
                />
              </label>
              <label>
                <span>天气省份</span>
                <input value={draft.weatherProvince ?? ""} onChange={updateText("weatherProvince")} />
              </label>
              <label>
                <span>天气地点</span>
                <input value={draft.weatherPlace ?? ""} onChange={updateText("weatherPlace")} />
              </label>
              <label>
                <span>天气备用文本</span>
                <input
                  value={draft.weatherFallbackText ?? ""}
                  onChange={updateText("weatherFallbackText")}
                />
              </label>
              <label>
                <span>旧城市代码，暂不用于接口盒子</span>
                <input value={draft.weatherCityCode} onChange={updateText("weatherCityCode")} />
              </label>
            </div>
            <div className="settings-actions">
              <button type="button" onClick={() => onRefreshWeather(draft)} disabled={saveDisabled}>
                保存并刷新天气
              </button>
            </div>
          </>
        )}

        {activeTab === "wallpaper" && (
          <div className="settings-grid">
            <label>
              <span>默认壁纸文件夹</span>
              <input
                value={draft.defaultWallpaperFolder}
                onChange={updateText("defaultWallpaperFolder")}
              />
            </label>
            <label>
              <span>备用壁纸文件夹</span>
              <input
                value={draft.alternateWallpaperFolder}
                onChange={updateText("alternateWallpaperFolder")}
              />
            </label>
          </div>
        )}

        {activeTab === "display" && (
          <div className="settings-grid">
            <label>
              <span>专注分钟</span>
              <input
                type="number"
                min={1}
                max={180}
                value={draft.focusMinutes}
                onChange={(event) => update("focusMinutes", Number(event.target.value) || 25)}
              />
            </label>
            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={draft.showSpectrum}
                onChange={(event) => update("showSpectrum", event.target.checked)}
              />
              <span>显示真实音乐条</span>
            </label>
          </div>
        )}

        {activeTab === "files" && (
          <section className="settings-section">
            <p>
              只读取授权文件夹；记忆压缩只写应用留档目录，不会修改或删除原始文件。
            </p>
            <div className="file-access-add">
              <input
                value={fileAccessPath}
                onChange={(event) => onFileAccessPathChange(event.target.value)}
                placeholder="输入要授权的文件夹路径"
              />
              <button type="button" onClick={onAddAuthorizedRoot}>
                添加文件夹
              </button>
            </div>
            {fileAccessStatus && <p className="file-access-status">{fileAccessStatus}</p>}
            <div className="authorized-root-list">
              {authorizedRoots.map((root) => (
                <article className="authorized-root-card" key={root.id}>
                  <div>
                    <strong>{root.label}</strong>
                    <span>{root.path}</span>
                    {root.lastIndexedAt && <small>上次索引：{root.lastIndexedAt}</small>}
                  </div>
                  <div className="authorized-root-actions">
                    <button type="button" onClick={() => onScanAuthorizedRoot(root.id)}>
                      重新索引
                    </button>
                    <button type="button" onClick={() => onRemoveAuthorizedRoot(root.id)}>
                      移除授权
                    </button>
                  </div>
                </article>
              ))}
              {authorizedRoots.length === 0 && <p className="muted">还没有授权文件夹。</p>}
            </div>
          </section>
        )}
      </div>
    </section>
  );
}
