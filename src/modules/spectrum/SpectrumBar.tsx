import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { SpectrumPayload } from "../../types";

interface SpectrumBarProps {
  bars: number[];
  active: boolean;
  compact?: boolean;
}

const BAR_COUNT = 42;

const QUIET_LEVEL = 0.025;
const NOISE_FLOOR = 0.035;
const VISUAL_GAIN = 1.0;
const VISUAL_CURVE = 0.78;

// 闁哄洨顥愮粣锟犲箥鐎ｅ墎绐楁慨锝嗘煣缁狅綁宕滃鍛渐
const ATTACK_SPEED = 13.5;
const RELEASE_SPEED = 5.5;
const NEIGHBOR_SMOOTH_PASSES = 1;

// 蓝牙或默认输出切换兜底：长时间没有有效 bars 就重启采集
const AUTO_RESTART_AFTER_MS = 2500;
const AUTO_RESTART_COOLDOWN_MS = 5000;
const ACTIVE_BAR_THRESHOLD = 0.08;

const fallbackBars = Array.from({ length: BAR_COUNT }, () => QUIET_LEVEL);

function clamp01(value: number) {
  return Math.max(0, Math.min(1, value));
}

function shapeBar(value: number, isActive: boolean) {
  if (!isActive) {
    return QUIET_LEVEL;
  }

  const x = clamp01(value);
  const gated = Math.max(0, (x - NOISE_FLOOR) / (1 - NOISE_FLOOR));

  return Math.max(
    QUIET_LEVEL,
    Math.min(1, Math.pow(gated, VISUAL_CURVE) * VISUAL_GAIN)
  );
}

function smoothNeighborsOnce(values: number[]) {
  return values.map((value, index) => {
    const left = values[index - 1] ?? value;
    const right = values[index + 1] ?? value;
    return left * 0.25 + value * 0.5 + right * 0.25;
  });
}

function smoothNeighbors(values: number[], passes: number) {
  let next = values;

  for (let i = 0; i < passes; i += 1) {
    next = smoothNeighborsOnce(next);
  }

  return next;
}

function getStats(values: number[]) {
  if (values.length === 0) {
    return {
      min: 0,
      max: 0,
      avg: 0
    };
  }

  let min = Number.POSITIVE_INFINITY;
  let max = 0;
  let sum = 0;

  for (const value of values) {
    min = Math.min(min, value);
    max = Math.max(max, value);
    sum += value;
  }

  return {
    min,
    max,
    avg: sum / values.length
  };
}

function approach(current: number, target: number, deltaSeconds: number) {
  const speed = target > current ? ATTACK_SPEED : RELEASE_SPEED;
  const factor = 1 - Math.exp(-speed * deltaSeconds);
  return current + (target - current) * factor;
}

async function restartAudioSpectrum(reason: string) {
  console.warn("[SpectrumBar] restarting audio spectrum:", reason);

  try {
    await invoke("stop_audio_spectrum");
  } catch (error) {
    console.warn("[SpectrumBar] stop_audio_spectrum failed", error);
  }

  await new Promise((resolve) => window.setTimeout(resolve, 250));

  try {
    await invoke("start_audio_spectrum");
    console.log("[SpectrumBar] start_audio_spectrum restarted ok");
  } catch (error) {
    console.error("[SpectrumBar] start_audio_spectrum restart failed", error);
  }
}

export function SpectrumBar({ bars, active, compact = false }: SpectrumBarProps) {
  const [eventBars, setEventBars] = useState<number[]>([]);
  const [eventActive, setEventActive] = useState(false);
  const [displayBars, setDisplayBars] = useState<number[]>(fallbackBars);
  const [debug, setDebug] = useState("waiting for jingzhuo-audio-spectrum");
  const [eps, setEps] = useState(0);

  const targetRef = useRef<number[]>(fallbackBars);
  const displayRef = useRef<number[]>(fallbackBars);
  const eventCountRef = useRef(0);
  const rateStartRef = useRef(performance.now());
  const lastBarsRef = useRef<number[]>([]);
  const lastFrameTimeRef = useRef<number | null>(null);

  const lastStrongAudioAtRef = useRef(performance.now());
  const lastRestartAtRef = useRef(0);
  const restartingRef = useRef(false);

  useEffect(() => {
    let disposed = false;

    void invoke("start_audio_spectrum")
      .then(() => {
        console.log("[SpectrumBar] start_audio_spectrum ok");
      })
      .catch((error) => {
        console.error("[SpectrumBar] start_audio_spectrum failed", error);
        setDebug(`start_audio_spectrum failed: ${String(error)}`);
      });

    const unlistenPromise = listen<SpectrumPayload>("jingzhuo-audio-spectrum", (event) => {
      if (disposed) {
        return;
      }

      const payload = event.payload;
      const nextBars = payload.bars ?? [];
      const rms = payload.rms ?? 0;
      const rawStats = getStats(nextBars);

      const diff = nextBars.reduce((sum, value, index) => {
        return sum + Math.abs(value - (lastBarsRef.current[index] ?? 0));
      }, 0);

      lastBarsRef.current = nextBars;

      const hasAudio = Boolean(
        payload.active ||
          payload.source === "loopback" ||
          payload.source === "meter" ||
          rms > 0.003 ||
          rawStats.max > 0.045
      );

      const now = performance.now();

      if (rawStats.max > ACTIVE_BAR_THRESHOLD || rms > 0.006) {
        lastStrongAudioAtRef.current = now;
      }

      eventCountRef.current += 1;

      if (now - rateStartRef.current >= 1000) {
        setEps(eventCountRef.current);
        eventCountRef.current = 0;
        rateStartRef.current = now;
      }

      setEventBars(nextBars);
      setEventActive(hasAudio);

      setDebug(
        [
          `src=${payload.source ?? "unknown"}`,
          `active=${String(payload.active)}`,
          `rms=${rms.toFixed(5)}`,
          `peak=${(payload.peak ?? 0).toFixed(5)}`,
          `rawMax=${rawStats.max.toFixed(3)}`,
          `diff=${diff.toFixed(3)}`,
          `len=${nextBars.length}`
        ].join(" ")
      );
    });

    return () => {
      disposed = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    let timer = 0;

    const check = async () => {
      const now = performance.now();
      const quietFor = now - lastStrongAudioAtRef.current;
      const cooldown = now - lastRestartAtRef.current;

      if (
        quietFor >= AUTO_RESTART_AFTER_MS &&
        cooldown >= AUTO_RESTART_COOLDOWN_MS &&
        !restartingRef.current
      ) {
        restartingRef.current = true;
        lastRestartAtRef.current = now;
        setDebug((old) => `${old} restarting=auto`);

        await restartAudioSpectrum(`quiet for ${Math.round(quietFor)}ms`);

        lastStrongAudioAtRef.current = performance.now();
        restartingRef.current = false;
      }

      timer = window.setTimeout(check, 1000);
    };

    timer = window.setTimeout(check, 1000);

    return () => {
      window.clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    const inputBars = eventBars.length > 0
      ? eventBars
      : bars.length > 0
        ? bars
        : fallbackBars;

    const stats = getStats(inputBars);
    const visuallyActive = eventBars.length > 0
      ? eventActive || stats.max > 0.045
      : active || stats.max > 0.045;

    const shaped = inputBars.map((value) => shapeBar(value, visuallyActive));
    const spatial = smoothNeighbors(shaped, NEIGHBOR_SMOOTH_PASSES);

    targetRef.current = spatial;

    if (displayRef.current.length !== spatial.length) {
      displayRef.current = spatial.map(() => QUIET_LEVEL);
      setDisplayBars(displayRef.current);
    }
  }, [eventBars, eventActive, bars, active]);

  useEffect(() => {
    let frame = 0;

    const tick = (now: number) => {
      const last = lastFrameTimeRef.current ?? now;
      lastFrameTimeRef.current = now;

      const deltaSeconds = Math.min(0.05, Math.max(0.001, (now - last) / 1000));
      const target = targetRef.current;

      const current = displayRef.current.length === target.length
        ? displayRef.current
        : target.map(() => QUIET_LEVEL);

      const next = target.map((value, index) => {
        const oldValue = current[index] ?? QUIET_LEVEL;
        return Math.max(QUIET_LEVEL, Math.min(1, approach(oldValue, value, deltaSeconds)));
      });

      displayRef.current = next;
      setDisplayBars(next);

      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(frame);
      lastFrameTimeRef.current = null;
    };
  }, []);

  const displayStats = getStats(displayBars);
  const activeNow = displayStats.max > QUIET_LEVEL + 0.01;
  const displayHeight = compact ? 110 : 220;

  return (
    <div
      className={`spectrum-wrap ${activeNow ? "is-active" : ""} ${compact ? "is-compact" : ""}`}
      aria-label="系统声音频谱"
      data-spectrum-owner="src/modules/spectrum/SpectrumBar.tsx"
    >

      {displayBars.map((value, index) => (
        <i
          key={index}
          style={{
            height: `${Math.round(value * displayHeight)}px`
          }}
        />
      ))}
    </div>
  );
}
