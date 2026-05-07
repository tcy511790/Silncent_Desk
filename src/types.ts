export type PanelMode = "none" | "command" | "ai" | "settings" | "archive";
export type WeatherSource = "apihz" | "fallback" | "none";
export type ArchiveRole = "user" | "assistant";
export type ArchiveCardType = "decision" | "bug" | "todo" | "design" | "note";
export type AiReplyMode = "normal" | "expert" | "night";

export interface AppConfig {
  [key: string]: unknown;
  weatherText: string;
  focusMinutes: number;
  showSpectrum: boolean;
  wallpaperStatusText: string;
  defaultWallpaperFolder: string;
  alternateWallpaperFolder: string;
  wallpaperDir?: string;
  backupWallpaperDir?: string;
  wallpaperIntervalMs: number;
  deepseekApiKey: string;
  deepseekBaseUrl: string;
  deepseekModel: string;
  aiReplyMode?: AiReplyMode;
  weatherCityCode: string;
  weatherProvider?: "apihz";
  weatherApiId?: string;
  weatherApiKey?: string;
  weatherProvince?: string;
  weatherPlace?: string;
  weatherFallbackText?: string;
  smartWeatherAppId: string;
  smartWeatherPrivateKey: string;
  lastWeatherText: string;
  authorizedRoots?: AuthorizedRoot[];
}

export interface WeatherDisplay {
  text: string;
  city?: string;
  province?: string;
  temperature?: number;
  humidity?: number | string;
  windDirection?: string;
  windScale?: string;
  condition?: string;
  updatedAt?: string;
  source: WeatherSource;
  error?: string | null;
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export interface ArchiveMessage {
  id: string;
  sessionId: string;
  role: ArchiveRole;
  content: string;
  createdAt: string;
}

export interface ArchiveCard {
  id: string;
  sessionId: string;
  type: ArchiveCardType;
  title: string;
  summary: string;
  keyPoints: string[];
  todos: string[];
  tags: string[];
  createdAt: string;
}

export interface AuthorizedRoot {
  id: string;
  path: string;
  label: string;
  createdAt: string;
  lastIndexedAt?: string;
}

export interface FileIndexItem {
  id: string;
  rootId: string;
  path: string;
  relativePath: string;
  name: string;
  extension: string;
  sizeBytes: number;
  modifiedAt: string;
  kind: "text" | "image" | "pdf" | "office" | "unknown";
}

export interface MemoryCompactResult {
  date: string;
  summaryPath: string;
  messageCount: number;
  cardCount: number;
}

export interface SpectrumPayload {
  bars: number[];
  rms: number;
  active: boolean;
  peak?: number;
  timestamp?: number;
  source?: "loopback" | "meter" | "quiet" | "error";
  error?: string | null;
  debug?: string;
}

export interface AudioDiagnostics {
  defaultMultimediaPeak: number;
  defaultConsolePeak: number;
  multimediaState: string;
  consoleState: string;
  sessionCount: number;
  activeSessionCount: number;
  sessionPeaks: string[];
  logPath: string;
}
