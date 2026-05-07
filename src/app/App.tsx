import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";
import { AiPanel } from "../modules/ai/AiPanel";
import { ArchivePanel } from "../modules/archive/ArchivePanel";
import { CommandPanel, type CommandAction } from "../modules/command/CommandPanel";
import { FocusStatus } from "../modules/focus/FocusStatus";
import { InfoColumn } from "../modules/info/InfoColumn";
import { SettingsPanel } from "../modules/settings/SettingsPanel";
import { SpectrumBar } from "../modules/spectrum/SpectrumBar";
import { FunctionWheel } from "../modules/wheel/FunctionWheel";
import type {
  AppConfig,
  ArchiveCard,
  ArchiveCardType,
  ArchiveMessage,
  AuthorizedRoot,
  AudioDiagnostics,
  FileIndexItem,
  MemoryCompactResult,
  ChatMessage,
  PanelMode,
  SpectrumPayload,
  WeatherDisplay
} from "../types";

const TOAST_DURATION_MS = 1800;
const TOAST_EXIT_MS = 180;
const OVERLAY_EXIT_MS = 220;
const SPACE_HOLD_MS = 420;
const MUSIC_BAR_ENABLED = true;
const FORCE_SPECTRUM_TEST =
  import.meta.env.DEV &&
  typeof window !== "undefined" &&
  window.location.search.includes("forceSpectrum");

const defaultConfig: AppConfig = {
  weatherText: "细雨 18°",
  focusMinutes: 25,
  showSpectrum: true,
  wallpaperStatusText: "背景",
  defaultWallpaperFolder: String.raw`D:\BaiduNetdiskDownload\desk_1`,
  alternateWallpaperFolder: String.raw`D:\BaiduNetdiskDownload\desk_2`,
  wallpaperIntervalMs: 60_000,
  deepseekApiKey: "",
  deepseekBaseUrl: "https://api.deepseek.com",
  deepseekModel: "deepseek-chat",
  aiReplyMode: "normal",
  weatherCityCode: "101010100",
  weatherProvider: "apihz",
  weatherApiId: "",
  weatherApiKey: "",
  weatherProvince: "北京",
  weatherPlace: "北京",
  weatherFallbackText: "未更新",
  smartWeatherAppId: "",
  smartWeatherPrivateKey: "",
  lastWeatherText: "细雨 18°"
};

function folderLabel(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function intervalLabel(intervalMs: number) {
  if (intervalMs < 60_000) {
    return `${Math.max(1, Math.round(intervalMs / 1000))}s`;
  }

  return `${Math.max(1, Math.round(intervalMs / 60_000))}min`;
}

function isTextInputTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return (
    target.isContentEditable ||
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT"
  );
}

function getBackupWeatherText(config: AppConfig) {
  return (config.weatherFallbackText || "").trim();
}

function configDebugSnapshot(config: AppConfig) {
  return {
    hasDeepseekApiKey: Boolean(config.deepseekApiKey),
    deepseekApiKeyLength: config.deepseekApiKey?.length ?? 0,
    deepseekBaseUrl: config.deepseekBaseUrl,
    deepseekModel: config.deepseekModel,
    aiReplyMode: config.aiReplyMode ?? "normal",
    hasWeatherApiId: Boolean(config.weatherApiId),
    weatherApiIdLength: config.weatherApiId?.length ?? 0,
    hasWeatherApiKey: Boolean(config.weatherApiKey),
    weatherApiKeyLength: config.weatherApiKey?.length ?? 0,
    weatherCityCode: config.weatherCityCode,
    weatherProvider: config.weatherProvider,
    weatherProvince: config.weatherProvince,
    weatherPlace: config.weatherPlace,
    defaultWallpaperFolder: config.defaultWallpaperFolder,
    alternateWallpaperFolder: config.alternateWallpaperFolder,
    showSpectrum: config.showSpectrum
  };
}

function formatWeatherToastLabel(weather: WeatherDisplay) {
  if (weather.city && typeof weather.temperature === "number") {
    return `${weather.city} ${Math.round(weather.temperature)}°`;
  }

  return weather.text || "天气未更新";
}

function makeId(prefix: string) {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `${prefix}-${crypto.randomUUID()}`;
  }

  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function todayKey(date = new Date()) {
  return date.toISOString().slice(0, 10);
}

function redactSensitiveText(value: string) {
  return value
    .replace(/sk-[A-Za-z0-9_-]{12,}/g, "[已隐藏密钥]")
    .replace(/(api[_ -]?key|token|password|密码|密钥)(\s*[:=]\s*)(\S+)/gi, "$1$2[已隐藏]");
}

function extractJsonObject(raw: string) {
  const trimmed = raw.trim().replace(/^```json\s*/i, "").replace(/^```\s*/i, "").replace(/```$/i, "");
  const start = trimmed.indexOf("{");
  const end = trimmed.lastIndexOf("}");
  if (start < 0 || end < start) {
    throw new Error("模型没有返回 JSON");
  }

  return trimmed.slice(start, end + 1);
}

function normalizeArchiveCardType(value: unknown): ArchiveCardType {
  const allowed: ArchiveCardType[] = ["decision", "bug", "todo", "design", "note"];
  return allowed.includes(value as ArchiveCardType) ? (value as ArchiveCardType) : "note";
}

function stringArray(value: unknown) {
  return Array.isArray(value)
    ? value.map((item) => String(item).trim()).filter(Boolean).slice(0, 8)
    : [];
}

export function App() {
  const spaceHoldTimer = useRef<number | null>(null);
  const panelCloseTimer = useRef<number | null>(null);
  const toastClearTimer = useRef<number | null>(null);
  const toastHideTimer = useRef<number | null>(null);
  const wheelCloseTimer = useRef<number | null>(null);
  const sessionIdRef = useRef(makeId("session"));
  const [config, setConfig] = useState<AppConfig>(defaultConfig);
  const [configLoaded, setConfigLoaded] = useState(false);
  const [loadConfigError, setLoadConfigError] = useState("");
  const [wheelOpen, setWheelOpen] = useState(false);
  const [wheelClosing, setWheelClosing] = useState(false);
  const [panelMode, setPanelMode] = useState<PanelMode>("none");
  const [panelClosing, setPanelClosing] = useState(false);
  const [toastText, setToastText] = useState("");
  const [toastVisible, setToastVisible] = useState(false);
  const [now, setNow] = useState(() => new Date());
  const [currentWallpaperFolderLabel, setCurrentWallpaperFolderLabel] = useState(
    folderLabel(defaultConfig.defaultWallpaperFolder)
  );
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [aiLoading, setAiLoading] = useState(false);
  const [aiErrorText, setAiErrorText] = useState("");
  const [archiveCards, setArchiveCards] = useState<ArchiveCard[]>([]);
  const [archiveErrorText, setArchiveErrorText] = useState("");
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [archiveSaving, setArchiveSaving] = useState(false);
  const [authorizedRoots, setAuthorizedRoots] = useState<AuthorizedRoot[]>([]);
  const [fileAccessPath, setFileAccessPath] = useState("");
  const [fileAccessStatus, setFileAccessStatus] = useState("");
  const [spectrumBars, setSpectrumBars] = useState<number[]>([]);
  const [spectrumActive, setSpectrumActive] = useState(false);
  const [forceSpectrumBars, setForceSpectrumBars] = useState<number[]>([]);
  const [audioDebugOpen, setAudioDebugOpen] = useState(false);
  const [audioDebugText, setAudioDebugText] = useState("");
  const [spectrumDebug, setSpectrumDebug] = useState({
    startStatus: "idle",
    startError: "",
    lastEventAt: "",
    lastEventSummary: "no event",
    lastBarsLength: 0,
    lastMaxBar: 0
  });
  const [weatherDisplay, setWeatherDisplay] = useState<WeatherDisplay>({
    text: "天气未更新",
    source: "none"
  });
  const lastSpectrumBarsRef = useRef<number[]>([]);
  const spectrumEventCountRef = useRef(0);
  const spectrumRateWindowStartRef = useRef<number | null>(null);
  const spectrumEventRateRef = useRef(0);

  const closeWheel = () => {
    if (wheelOpen && !wheelClosing) {
      setWheelClosing(true);
      if (wheelCloseTimer.current !== null) {
        window.clearTimeout(wheelCloseTimer.current);
      }
      wheelCloseTimer.current = window.setTimeout(() => {
        setWheelOpen(false);
        setWheelClosing(false);
        wheelCloseTimer.current = null;
      }, OVERLAY_EXIT_MS);
    }
  };

  const closePanel = () => {
    if (panelMode !== "none" && !panelClosing) {
      setPanelClosing(true);
      if (panelCloseTimer.current !== null) {
        window.clearTimeout(panelCloseTimer.current);
      }
      panelCloseTimer.current = window.setTimeout(() => {
        setPanelMode("none");
        setPanelClosing(false);
        panelCloseTimer.current = null;
      }, OVERLAY_EXIT_MS);
    }
  };

  const closeOverlay = () => {
    closeWheel();
    closePanel();

    setAudioDebugOpen(false);
  };

  const closeTopLayer = () => {
    if (wheelOpen) {
      closeWheel();
      return;
    }

    if (panelMode === "settings") {
      closePanel();
      return;
    }

    if (panelMode === "archive") {
      closePanel();
      return;
    }

    if (panelMode === "command") {
      closePanel();
      return;
    }

    if (panelMode === "ai") {
      closePanel();
      return;
    }

    if (audioDebugOpen) {
      setAudioDebugOpen(false);
    }
  };

  const openWheel = () => {
    if (panelCloseTimer.current !== null) {
      window.clearTimeout(panelCloseTimer.current);
      panelCloseTimer.current = null;
    }
    if (wheelCloseTimer.current !== null) {
      window.clearTimeout(wheelCloseTimer.current);
      wheelCloseTimer.current = null;
    }
    setPanelClosing(false);
    setPanelMode("none");
    setWheelClosing(false);
    setWheelOpen(true);
  };

  const openPanel = (mode: Exclude<PanelMode, "none">) => {
    if (panelCloseTimer.current !== null) {
      window.clearTimeout(panelCloseTimer.current);
      panelCloseTimer.current = null;
    }
    if (wheelCloseTimer.current !== null) {
      window.clearTimeout(wheelCloseTimer.current);
      wheelCloseTimer.current = null;
    }
    setWheelOpen(false);
    setWheelClosing(false);
    setPanelClosing(false);
    setPanelMode(mode);
  };

  const cancelSpaceHold = () => {
    if (spaceHoldTimer.current !== null) {
      window.clearTimeout(spaceHoldTimer.current);
      spaceHoldTimer.current = null;
    }
  };

  const showToast = (message: string) => {
    if (toastHideTimer.current !== null) {
      window.clearTimeout(toastHideTimer.current);
    }
    if (toastClearTimer.current !== null) {
      window.clearTimeout(toastClearTimer.current);
    }
    setToastText(message);
    setToastVisible(true);
    toastHideTimer.current = window.setTimeout(() => {
      setToastVisible(false);
      toastClearTimer.current = window.setTimeout(() => {
        setToastText("");
        toastClearTimer.current = null;
      }, TOAST_EXIT_MS);
    }, TOAST_DURATION_MS);
  };

  useEffect(() => {
    console.log("[config] load_config invoke");
    invoke<AppConfig>("load_config")
      .then((loaded) => {
        const merged = { ...defaultConfig, ...loaded };
        console.log("[config] loaded", configDebugSnapshot(merged));
        setLoadConfigError("");
        setConfig(merged);
        setCurrentWallpaperFolderLabel(folderLabel(merged.defaultWallpaperFolder));
        const backupText = getBackupWeatherText(merged);
        setWeatherDisplay(
          backupText && backupText !== "未更新"
            ? { text: backupText, source: "fallback" }
            : { text: "天气未更新", source: "none" }
        );
      })
      .catch((err) => {
        console.error("[config] load_config failed", String(err));
        setLoadConfigError(String(err || "配置读取失败"));
        setConfig(defaultConfig);
        setCurrentWallpaperFolderLabel(folderLabel(defaultConfig.defaultWallpaperFolder));
        const backupText = getBackupWeatherText(defaultConfig);
        setWeatherDisplay({
          text: backupText && backupText !== "未更新" ? backupText : "天气未更新",
          source: backupText && backupText !== "未更新" ? "fallback" : "none"
        });
      })
      .finally(() => setConfigLoaded(true));
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(
    () => () => {
      [
        panelCloseTimer.current,
        toastClearTimer.current,
        toastHideTimer.current,
        wheelCloseTimer.current
      ].forEach((timer) => {
        if (timer !== null) {
          window.clearTimeout(timer);
        }
      });
    },
    []
  );

  useEffect(() => {
    const unlisteners = [
      listen("jingzhuo-wheel-open", openWheel),
      listen("jingzhuo-wheel-close", closeOverlay),
      listen("jingzhuo-toolbar-open", () => openPanel("command")),
      listen("jingzhuo-ai-open", () => openPanel("ai"))
    ];

    return () => {
      unlisteners.forEach((promise) => {
        promise.then((unlisten) => unlisten()).catch(() => undefined);
      });
    };
  }, []);

  useEffect(() => {
    const onOpen = () => openWheel();
    const onClose = () => closeOverlay();
    const onToolbarOpen = () => openPanel("command");
    const onAiOpen = () => openPanel("ai");

    window.addEventListener("jingzhuo-wheel-open", onOpen);
    window.addEventListener("jingzhuo-wheel-close", onClose);
    window.addEventListener("jingzhuo-toolbar-open", onToolbarOpen);
    window.addEventListener("jingzhuo-ai-open", onAiOpen);

    return () => {
      window.removeEventListener("jingzhuo-wheel-open", onOpen);
      window.removeEventListener("jingzhuo-wheel-close", onClose);
      window.removeEventListener("jingzhuo-toolbar-open", onToolbarOpen);
      window.removeEventListener("jingzhuo-ai-open", onAiOpen);
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const hasAnyOverlayOpen = wheelOpen || panelMode !== "none" || audioDebugOpen;

      if (event.code === "KeyQ" && event.ctrlKey && event.shiftKey) {
        invoke("quit_app").catch(() => undefined);
        return;
      }

      if (event.key === "Escape") {
        if (!hasAnyOverlayOpen) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        closeTopLayer();
        return;
      }

      if (event.code === "Space" && event.ctrlKey && panelMode === "none") {
        if (event.altKey) {
          openWheel();
        } else {
          openPanel("command");
        }
        return;
      }

      if (event.code === "KeyT" && event.ctrlKey && panelMode === "none") {
        openPanel("ai");
        return;
      }

      if (
        event.code !== "Space" ||
        event.ctrlKey ||
        wheelOpen ||
        panelMode !== "none" ||
        isTextInputTarget(event.target)
      ) {
        return;
      }

      if (spaceHoldTimer.current === null) {
        spaceHoldTimer.current = window.setTimeout(() => {
          spaceHoldTimer.current = null;
          openWheel();
        }, SPACE_HOLD_MS);
      }
    };

    const onKeyUp = (event: KeyboardEvent) => {
      if (event.code === "Space") {
        cancelSpaceHold();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      cancelSpaceHold();
    };
  }, [audioDebugOpen, panelMode, wheelOpen]);

  useEffect(() => {
    invoke("set_overlay_interactive", {
      interactive: wheelOpen || panelMode !== "none" || audioDebugOpen
    }).catch(() => undefined);
  }, [audioDebugOpen, panelMode, wheelOpen]);

  useEffect(() => {
    if (!FORCE_SPECTRUM_TEST) {
      return undefined;
    }

    let frameId = 0;
    const animate = (now: number) => {
      const time = now / 1000;
      setForceSpectrumBars(
        Array.from({ length: 42 }, (_, index) => {
          const waveA = Math.sin(time * 4.8 + index * 0.42) * 0.5 + 0.5;
          const waveB = Math.sin(time * 2.2 - index * 0.18) * 0.5 + 0.5;
          const value = 0.08 + (waveA * 0.68 + waveB * 0.24) * 0.85;
          return Math.max(0.08, Math.min(0.93, value));
        })
      );
      frameId = window.requestAnimationFrame(animate);
    };

    frameId = window.requestAnimationFrame(animate);
    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, []);

  useEffect(() => {
    if (!MUSIC_BAR_ENABLED || !config.showSpectrum) {
      setSpectrumBars([]);
      setSpectrumActive(false);
      setSpectrumDebug((current) => ({
        ...current,
        startStatus: "disabled",
        startError: "",
        lastEventSummary: "spectrum disabled by config"
      }));
      return undefined;
    }

    let disposed = false;
    lastSpectrumBarsRef.current = [];
    spectrumEventCountRef.current = 0;
    spectrumRateWindowStartRef.current = null;
    spectrumEventRateRef.current = 0;

    const updateEventRate = () => {
      const nowMs = performance.now();
      const windowStart = spectrumRateWindowStartRef.current ?? nowMs;
      spectrumRateWindowStartRef.current = windowStart;
      spectrumEventCountRef.current += 1;

      const elapsed = nowMs - windowStart;
      if (elapsed >= 1000) {
        spectrumEventRateRef.current = (spectrumEventCountRef.current * 1000) / elapsed;
        spectrumEventCountRef.current = 0;
        spectrumRateWindowStartRef.current = nowMs;
      }

      return spectrumEventRateRef.current;
    };

    const applySpectrumPayload = (payload: SpectrumPayload, label: "event" | "snapshot") => {
      const bars = payload.bars ?? [];
      const rms = payload.rms ?? 0;
      const maxBar = bars.length > 0 ? Math.max(...bars) : 0;
      const minBar = bars.length > 0 ? Math.min(...bars) : 0;
      const avgBar = bars.length > 0 ? bars.reduce((sum, value) => sum + value, 0) / bars.length : 0;
      const eventTime = new Date().toLocaleTimeString("zh-CN", { hour12: false });
      const diff = bars.reduce(
        (sum, value, index) => sum + Math.abs(value - (lastSpectrumBarsRef.current[index] ?? 0)),
        0
      );
      const changed = diff > 0.01;
      const eventRate = label === "event" ? updateEventRate() : spectrumEventRateRef.current;
      const hasAudio = Boolean(
        payload.active ||
          payload.source === "loopback" ||
          rms > 0.003 ||
          maxBar > 0.045
      );

      console.log("[App] jingzhuo-audio-spectrum event", {
        source: payload.source,
        active: payload.active,
        rms,
        peak: payload.peak,
        barsLength: bars.length,
        minBar,
        maxBar,
        avgBar,
        first5: bars.slice(0, 5),
        debug: payload.debug
      });

      lastSpectrumBarsRef.current = bars;
      setSpectrumBars(bars);
      setSpectrumActive(hasAudio);
      setSpectrumDebug({
        startStatus: "started",
        startError: "",
        lastEventAt: eventTime,
        lastEventSummary: `${label} payload=${Boolean(payload)} source=${
          payload.source ?? "unknown"
        } active=${payload.active}`,
        lastBarsLength: bars.length,
        lastMaxBar: maxBar
      });
      setAudioDebugText(
        [
          `${label} active=${payload.active} source=${payload.source ?? "unknown"} rms=${rms.toFixed(
            5
          )} peak=${(payload.peak ?? 0).toFixed(5)} maxBar=${maxBar.toFixed(
            5
          )} minBar=${minBar.toFixed(5)} avgBar=${avgBar.toFixed(5)} changed=${changed} eps=${eventRate.toFixed(
            1
          )} bars=${bars.length}`,
          payload.timestamp ? `timestamp=${payload.timestamp}` : "",
          payload.error ? `error=${payload.error}` : "",
          payload.debug ? `debug=${payload.debug}` : ""
        ]
          .filter(Boolean)
          .join("\n")
      );
    };

    const unlistenPromise = listen<SpectrumPayload>("jingzhuo-audio-spectrum", (event) => {
      if (disposed) {
        return;
      }

      applySpectrumPayload(event.payload, "event");
    });

    setSpectrumDebug((current) => ({
      ...current,
      startStatus: "starting",
      startError: "",
      lastEventSummary: "starting audio spectrum..."
    }));
    setAudioDebugText("starting audio spectrum...");

    invoke("start_audio_spectrum")
      .then(() => {
        setSpectrumDebug((current) => ({
          ...current,
          startStatus: "started",
          startError: "",
          lastEventSummary:
            current.lastEventSummary === "starting audio spectrum..."
              ? "audio spectrum started"
              : current.lastEventSummary
        }));
        setAudioDebugText("audio spectrum started");
      })
      .catch((err) => {
        const message = String(err);
        setSpectrumBars([]);
        setSpectrumActive(false);
        setSpectrumDebug((current) => ({
          ...current,
          startStatus: "failed",
          startError: message,
          lastEventSummary: "start_audio_spectrum failed"
        }));
        setAudioDebugText(`start_audio_spectrum failed: ${message}`);
      });

    return () => {
      disposed = true;
      unlistenPromise.then((unlisten) => unlisten()).catch(() => undefined);
      invoke("stop_audio_spectrum").catch(() => undefined);
    };
  }, [config.showSpectrum]);

  useEffect(() => {
    setWeatherDisplay((current) => {
      if (
        current.source !== "apihz" ||
        (current.city === config.weatherPlace && current.province === config.weatherProvince)
      ) {
        return current;
      }

      const backupText = getBackupWeatherText(config);
      return backupText && backupText !== "未更新"
        ? { text: backupText, source: "fallback" }
        : { text: "天气未更新", source: "none" };
    });
  }, [config, config.weatherPlace, config.weatherProvince]);

  useEffect(() => {
    if (panelMode === "archive" && !panelClosing) {
      loadArchiveCards();
    }
  }, [panelClosing, panelMode]);

  const loadAuthorizedRoots = () => {
    invoke<AuthorizedRoot[]>("file_list_authorized_roots")
      .then((roots) => setAuthorizedRoots(roots))
      .catch((err) => setFileAccessStatus(String(err)));
  };

  useEffect(() => {
    if (panelMode === "settings" && !panelClosing) {
      loadAuthorizedRoots();
    }
  }, [panelClosing, panelMode]);

  const handleAddAuthorizedRoot = () => {
    const path = fileAccessPath.trim();
    if (!path) {
      setFileAccessStatus("请先输入文件夹路径");
      return;
    }

    invoke<AuthorizedRoot>("file_add_authorized_root", { path })
      .then((root) => {
        setAuthorizedRoots((roots) => {
          if (roots.some((item) => item.id === root.id)) {
            return roots;
          }
          return [...roots, root];
        });
        setFileAccessPath("");
        setFileAccessStatus("已添加授权文件夹。用户原始文件保持只读。");
      })
      .catch((err) => setFileAccessStatus(String(err)));
  };

  const handleRemoveAuthorizedRoot = (rootId: string) => {
    invoke("file_remove_authorized_root", { rootId })
      .then(() => {
        setAuthorizedRoots((roots) => roots.filter((root) => root.id !== rootId));
        setFileAccessStatus("已移除访问权限，原文件未删除。");
        showToast("已移除访问权限，原文件未删除。");
      })
      .catch((err) => setFileAccessStatus(String(err)));
  };

  const handleScanAuthorizedRoot = (rootId: string) => {
    invoke<FileIndexItem[]>("file_scan_authorized_root", { rootId })
      .then((items) => {
        setFileAccessStatus(`已重新索引 ${items.length} 个文件。未读取文件内容。`);
        loadAuthorizedRoots();
      })
      .catch((err) => setFileAccessStatus(String(err)));
  };

  const handleCompactTodayMemory = () => {
    invoke<MemoryCompactResult>("memory_compact_day", { date: todayKey() })
      .then((result) => {
        showToast(`已生成压缩摘要：${result.messageCount} 条消息，${result.cardCount} 张卡片`);
      })
      .catch((err) => showToast(`记忆压缩失败：${String(err)}`));
  };

  const saveSettings = (nextConfig: AppConfig, showSuccess = true) => {
    if (!configLoaded || loadConfigError) {
      showToast(loadConfigError || "配置尚未正确加载，已阻止保存");
      return;
    }

    console.log("[config] save_config invoke", configDebugSnapshot(nextConfig));
    setConfig(nextConfig);
    invoke("save_config", { config: nextConfig })
      .then(() => {
        if (showSuccess) {
          showToast("设置已保存");
        }
      })
      .catch((err) => showToast(`设置保存失败：${String(err)}`));
  };
  const isProbablyAppConfig = (value: unknown): value is AppConfig => {
  if (!value || typeof value !== "object") {
    return false;
  }

  const maybe = value as Partial<AppConfig> & {
    currentTarget?: unknown;
    nativeEvent?: unknown;
    target?: unknown;
  };

  if ("currentTarget" in maybe || "nativeEvent" in maybe || "target" in maybe) {
    return false;
  }

  return (
    "deepseekBaseUrl" in maybe ||
    "deepseekApiKey" in maybe ||
    "weatherProvider" in maybe ||
    "weatherCityCode" in maybe ||
    "showSpectrum" in maybe
  );
};

  const refreshWeather = (overrideConfig?: unknown) => {
    if (!configLoaded || loadConfigError) {
      showToast(loadConfigError || "配置尚未正确加载，已阻止刷新天气");
      return;
    }
    const safeOverrideConfig = isProbablyAppConfig(overrideConfig) ? overrideConfig : undefined;

if (overrideConfig && !safeOverrideConfig) {
  console.warn("[weather] ignored non-config argument passed to refreshWeather", {
    type: typeof overrideConfig,
    hasCurrentTarget:
      typeof overrideConfig === "object" &&
      overrideConfig !== null &&
      "currentTarget" in overrideConfig
  });
}
    const weatherConfig = safeOverrideConfig ?? config;
    console.log("[weather] refresh_weather invoke", {
      hasWeatherApiId: Boolean(weatherConfig.weatherApiId),
      weatherApiIdLength: weatherConfig.weatherApiId?.length ?? 0,
      hasWeatherApiKey: Boolean(weatherConfig.weatherApiKey),
      weatherApiKeyLength: weatherConfig.weatherApiKey?.length ?? 0,
      weatherProvider: weatherConfig.weatherProvider,
      weatherProvince: weatherConfig.weatherProvince,
      weatherPlace: weatherConfig.weatherPlace,
      weatherCityCode: weatherConfig.weatherCityCode
    });
  if (safeOverrideConfig) {
    setConfig((current) => ({
      ...current,
      ...safeOverrideConfig
    }));
  }

    invoke<WeatherDisplay>("refresh_weather", { config: weatherConfig })
      .then((result) => {
        if (result.source === "apihz") {
          setConfig((value) => ({
            ...value,
            lastWeatherText: result.text
          }));
        }
        setWeatherDisplay(result);
        if (result.source === "apihz") {
          showToast(`天气已刷新：${formatWeatherToastLabel(result)}`);
        } else if (result.source === "fallback") {
          showToast(`天气接口失败，显示备用：${result.text}`);
        } else {
          showToast(`天气刷新失败：${result.error || result.text || "天气未更新"}`);
        }
      })
      .catch((err) => {
        const backupText = getBackupWeatherText(weatherConfig);
        setWeatherDisplay(
          backupText && backupText !== "未更新"
            ? { text: backupText, source: "fallback", error: String(err) }
            : { text: "天气未更新", source: "none", error: String(err) }
        );
        if (backupText && backupText !== "未更新") {
          showToast(`天气接口失败，显示备用：${backupText}`);
        } else {
          showToast(`天气刷新失败：${String(err)}`);
        }
      });
  };

  const saveArchiveMessage = (role: ArchiveMessage["role"], content: string) => {
    const message: ArchiveMessage = {
      id: makeId("msg"),
      sessionId: sessionIdRef.current,
      role,
      content: redactSensitiveText(content),
      createdAt: new Date().toISOString()
    };

    invoke("archive_save_message", { message }).catch((err) => {
      console.warn("[archive] save message failed", String(err));
    });
  };

  const loadArchiveCards = () => {
    setArchiveLoading(true);
    setArchiveErrorText("");
    console.log("[archive] archive_list_cards invoke", { date: todayKey() });
    invoke<ArchiveCard[]>("archive_list_cards", { date: todayKey() })
      .then((cards) => {
        console.log("[archive] archive_list_cards loaded", { count: cards.length });
        setArchiveCards(cards);
      })
      .catch((err) => {
        const message = String(err);
        setArchiveErrorText(message);
        showToast(`留档读取失败：${message}`);
      })
      .finally(() => setArchiveLoading(false));
  };

  const handleSaveCurrentRound = () => {
    const assistantIndex = [...chatMessages]
      .reverse()
      .findIndex((message) => message.role === "assistant");
    if (assistantIndex < 0) {
      showToast("没有可保存的本轮回复");
      return;
    }

    const actualAssistantIndex = chatMessages.length - 1 - assistantIndex;
    const userMessage = [...chatMessages.slice(0, actualAssistantIndex)]
      .reverse()
      .find((message) => message.role === "user");
    const assistantMessage = chatMessages[actualAssistantIndex];

    if (!userMessage || !assistantMessage) {
      showToast("没有可保存的本轮回复");
      return;
    }

    setArchiveSaving(true);
    invoke<string>("create_archive_card", {
      request: {
        userMessage: redactSensitiveText(userMessage.content),
        assistantMessage: redactSensitiveText(assistantMessage.content),
        config
      }
    })
      .then((raw) => {
        const parsed = JSON.parse(extractJsonObject(raw)) as Record<string, unknown>;
        const card: ArchiveCard = {
          id: makeId("card"),
          sessionId: sessionIdRef.current,
          type: normalizeArchiveCardType(parsed.type),
          title: String(parsed.title || "未命名留档").slice(0, 60),
          summary: String(parsed.summary || "").slice(0, 160),
          keyPoints: stringArray(parsed.keyPoints),
          todos: stringArray(parsed.todos),
          tags: stringArray(parsed.tags),
          createdAt: new Date().toISOString()
        };

        return invoke("archive_save_card", { card }).then(() => card);
      })
      .then((card) => {
        setArchiveCards((cards) => [...cards, card]);
        showToast("已保存到留档");
      })
      .catch((err) => {
        showToast(`留档保存失败：${String(err)}`);
      })
      .finally(() => setArchiveSaving(false));
  };

  const handleSendAiMessage = (message: string) => {
    setAiLoading(true);
    setAiErrorText("");
    const history = chatMessages;
    console.log("[ai] chat_with_deepseek invoke", {
      hasDeepseekApiKey: Boolean(config.deepseekApiKey),
      deepseekApiKeyLength: config.deepseekApiKey?.length ?? 0,
      deepseekBaseUrl: config.deepseekBaseUrl,
      deepseekModel: config.deepseekModel,
      aiReplyMode: config.aiReplyMode ?? "normal",
      historyLength: history.length
    });
    saveArchiveMessage("user", message);
    setChatMessages((messages) => [...messages, { role: "user", content: message }]);

    invoke<string>("chat_with_deepseek", {
      request: { message, history, config }
    })
      .then((answer) => {
        setChatMessages((messages) => [...messages, { role: "assistant", content: answer }]);
        saveArchiveMessage("assistant", answer);
      })
      .catch((err) => {
        setAiErrorText(String(err));
      })
      .finally(() => setAiLoading(false));
  };

  const handleNextWallpaper = () => {
    invoke<string>("next_wallpaper")
      .then(() => {
        showToast("已换下一张");
        closeOverlay();
      })
      .catch((err) => showToast(`切换失败：${String(err)}`));
  };

  const handleSwitchWallpaperFolder = () => {
    invoke<string>("switch_wallpaper_folder")
      .then((label) => {
        if (label) {
          setCurrentWallpaperFolderLabel(label);
          showToast(`已切换至 ${label}`);
        } else {
          showToast("已切换画册");
        }
        closeOverlay();
      })
      .catch((err) => showToast(`切换失败：${String(err)}`));
  };

  const runAudioDiagnostics = () => {
    setAudioDebugOpen(true);
    setAudioDebugText("running audio diagnostics...");

    invoke<AudioDiagnostics>("audio_diagnostics")
      .then((diagnostics) => {
        setAudioDebugText(
          [
            `log=${diagnostics.logPath}`,
            `MM peak=${diagnostics.defaultMultimediaPeak.toFixed(5)} ${diagnostics.multimediaState}`,
            `Console peak=${diagnostics.defaultConsolePeak.toFixed(5)} ${diagnostics.consoleState}`,
            `sessions=${diagnostics.activeSessionCount}/${diagnostics.sessionCount}`,
            ...diagnostics.sessionPeaks
          ].join("\n")
        );
      })
      .catch((err) => {
        setAudioDebugText(`audio diagnostics failed: ${String(err)}`);
      });
  };

  const focusLabel = useMemo(
    () => `${String(config.focusMinutes).padStart(2, "0")}:00`,
    [config.focusMinutes]
  );

  const wallpaperStatusText = useMemo(
    () =>
      `${config.wallpaperStatusText} · ${currentWallpaperFolderLabel} · ${intervalLabel(
        config.wallpaperIntervalMs
      )}`,
    [config.wallpaperIntervalMs, config.wallpaperStatusText, currentWallpaperFolderLabel]
  );

  const commands = useMemo<CommandAction[]>(
    () => [
      {
        id: "open-ai",
        title: "打开 AI",
        hint: "Ctrl+T",
        run: () => openPanel("ai")
      },
      {
        id: "next-wallpaper",
        title: "下一张壁纸",
        hint: "从当前文件夹切换",
        run: handleNextWallpaper
      },
      {
        id: "switch-wallpaper-folder",
        title: "切换壁纸文件夹",
        hint: currentWallpaperFolderLabel,
        run: handleSwitchWallpaperFolder
      },
      {
        id: "refresh-weather",
        title: "刷新天气",
        hint: config.weatherCityCode,
        run: () => refreshWeather()
      },
      {
        id: "audio-diagnostics",
        title: "音频诊断",
        hint: spectrumActive ? "active" : "silent",
        run: runAudioDiagnostics
      },
      {
        id: "settings",
        title: "打开设置",
        hint: "本地配置",
        run: () => openPanel("settings")
      },
      {
        id: "archive",
        title: "打开留档",
        hint: "今日卡片",
        run: () => openPanel("archive")
      },
      {
        id: "compact-memory",
        title: "压缩今日留档",
        hint: "只写入应用留档目录",
        run: handleCompactTodayMemory
      },
      {
        id: "quit",
        title: "退出静桌",
        hint: "Ctrl+Shift+Q",
        run: () => invoke("quit_app").catch(() => undefined)
      }
    ],
    [config.weatherCityCode, currentWallpaperFolderLabel, spectrumActive]
  );

  const renderedSpectrumBars = FORCE_SPECTRUM_TEST ? forceSpectrumBars : spectrumBars;
  const renderedSpectrumActive = FORCE_SPECTRUM_TEST || spectrumActive;
  const renderedSpectrumMaxBar =
    renderedSpectrumBars.length > 0 ? Math.max(...renderedSpectrumBars) : 0;
  const spectrumBarsMax = spectrumBars.length > 0 ? Math.max(...spectrumBars) : 0;
  const isInteractive = panelMode !== "none" || wheelOpen;
  const isOverlayActive = isInteractive || audioDebugOpen;

  console.log("[App] render spectrum props", {
    forceSpectrum: FORCE_SPECTRUM_TEST,
    spectrumActive: renderedSpectrumActive,
    spectrumBarsLength: renderedSpectrumBars.length,
    maxBar: renderedSpectrumMaxBar,
    first5: renderedSpectrumBars.slice(0, 5)
  });

  return (
    <main
      className={`overlay-root ${
        isInteractive ? "mode-interactive" : "mode-silent"
      } ${panelMode !== "none" ? "mode-panel" : ""} ${wheelOpen ? "mode-wheel" : ""}`}
    >
      <div className="edge-shade" />
      <InfoColumn
        now={now}
        weather={weatherDisplay}
        wallpaperStatusText={wallpaperStatusText}
        focus={<FocusStatus label={focusLabel} />}
        compact={isOverlayActive}
      />

      {MUSIC_BAR_ENABLED && config.showSpectrum && (
        <SpectrumBar
          bars={renderedSpectrumBars}
          active={renderedSpectrumActive}
          compact={isOverlayActive}
        />
      )}

      <div className="shortcut-mark">
        长按 Space / Ctrl+Alt+Space / Ctrl+Space / Ctrl+T / Esc / Ctrl+Shift+Q
      </div>

      {wheelOpen && (
        <FunctionWheel
          currentFolderLabel={currentWallpaperFolderLabel}
          onNextWallpaper={handleNextWallpaper}
          onSwitchWallpaperFolder={handleSwitchWallpaperFolder}
          onClose={closeOverlay}
          exiting={wheelClosing}
        />
      )}

      {panelMode === "command" && (
        <CommandPanel commands={commands} onClose={closeOverlay} exiting={panelClosing} />
      )}

      {panelMode === "ai" && (
        <AiPanel
          messages={chatMessages}
          loading={aiLoading}
          errorText={aiErrorText}
          archiveSaving={archiveSaving}
          onSend={handleSendAiMessage}
          onSaveRound={handleSaveCurrentRound}
          onClose={closeOverlay}
          exiting={panelClosing}
        />
      )}

      {panelMode === "settings" && (
        <SettingsPanel
          config={config}
          configLoaded={configLoaded}
          loadConfigError={loadConfigError}
          authorizedRoots={authorizedRoots}
          fileAccessPath={fileAccessPath}
          fileAccessStatus={fileAccessStatus}
          onSave={saveSettings}
          onFileAccessPathChange={setFileAccessPath}
          onAddAuthorizedRoot={handleAddAuthorizedRoot}
          onRemoveAuthorizedRoot={handleRemoveAuthorizedRoot}
          onScanAuthorizedRoot={handleScanAuthorizedRoot}
          onRefreshWeather={() => refreshWeather()}
          onClose={closeOverlay}
          exiting={panelClosing}
        />
      )}

      {panelMode === "archive" && (
        <ArchivePanel
          cards={archiveCards}
          loading={archiveLoading}
          errorText={archiveErrorText}
          onRefresh={loadArchiveCards}
          onCopied={() => showToast("已复制摘要")}
          onClose={closeOverlay}
          exiting={panelClosing}
        />
      )}

      {audioDebugOpen && (
        <section className="audio-debug-sheet" aria-label="音频诊断">
          <header>
            <span>音频诊断</span>
            <button type="button" onClick={() => setAudioDebugOpen(false)}>
              关闭
            </button>
          </header>
          <pre>{audioDebugText || "waiting for audio event..."}</pre>
        </section>
      )}

      <div
        style={{
          position: "absolute",
          right: 18,
          bottom: 18,
          zIndex: 20,
          maxWidth: 360,
          padding: "10px 12px",
          borderRadius: 10,
          border: "1px solid rgba(255,255,255,0.16)",
          background: "rgba(0,0,0,0.56)",
          color: "rgba(255,255,255,0.86)",
          fontFamily: "Consolas, 'Cascadia Mono', monospace",
          fontSize: 12,
          lineHeight: 1.5,
          whiteSpace: "pre-wrap",
          pointerEvents: "none"
        }}
      >
        {[
          `spectrumBars.length: ${spectrumBars.length}`,
          `Math.max(...spectrumBars): ${spectrumBarsMax.toFixed(5)}`,
          `spectrumActive: ${String(spectrumActive)}`,
          `last event: ${spectrumDebug.lastEventAt || "none"}`,
          `start_audio_spectrum: ${spectrumDebug.startStatus}`,
          spectrumDebug.startError ? `start error: ${spectrumDebug.startError}` : "",
          `event detail: ${spectrumDebug.lastEventSummary}`,
          `last event bars: ${spectrumDebug.lastBarsLength}`,
          `last event max: ${spectrumDebug.lastMaxBar.toFixed(5)}`
        ]
          .filter(Boolean)
          .join("\n")}
      </div>

      {toastText && (
        <div className={`toast ${toastVisible ? "" : "is-exiting"}`} role="status" aria-live="polite">
          {toastText}
        </div>
      )}
    </main>
  );
}
