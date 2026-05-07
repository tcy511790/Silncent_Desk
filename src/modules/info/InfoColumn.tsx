import type { ReactNode } from "react";
import type { WeatherDisplay } from "../../types";

interface InfoColumnProps {
  now: Date;
  weather: WeatherDisplay | null;
  wallpaperStatusText: string;
  focus: ReactNode;
  compact?: boolean;
}

function formatTime(date: Date) {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false
  }).format(date);
}

function formatWeatherLabel(weather: WeatherDisplay | null) {
  if (!weather) {
    return "天气未更新";
  }

  if (weather.source === "fallback") {
    return weather.text ? `${weather.text} · 备用` : "天气未更新";
  }

  if (weather.source === "none") {
    return "天气未更新";
  }

  if (weather.city && typeof weather.temperature === "number") {
    return `${weather.city} ${Math.round(weather.temperature)}°`;
  }

  return weather.text || "天气未更新";
}

export function InfoColumn({ now, weather, compact = false }: InfoColumnProps) {
  const weatherLabel = formatWeatherLabel(weather);

  return (
    <aside className={`info-column ${compact ? "is-compact" : ""}`} aria-label="桌面信息">
      <div className="compact-info">
        {formatTime(now)} · {weatherLabel}
      </div>
    </aside>
  );
}
