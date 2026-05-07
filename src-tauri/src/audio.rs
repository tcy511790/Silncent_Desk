use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use windows::core::Interface;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eConsole, eMultimedia, eRender, AudioSessionStateActive, IAudioCaptureClient, IAudioClient,
    IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    DEVICE_STATE_ACTIVE, WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM,
};
use windows::Win32::Media::KernelStreaming::{
    KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE,
};
use windows::Win32::Media::Multimedia::{
    KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};

use tauri::Manager;

const SPECTRUM_EVENT: &str = "jingzhuo-audio-spectrum";
const BAR_COUNT: usize = 42;
const FFT_WINDOW_SIZE: usize = 4096;
const RING_WINDOW_SIZE: usize = FFT_WINDOW_SIZE * 2;
const ACTIVE_RMS: f32 = 0.000_9;
const ACTIVE_PEAK: f32 = 0.004;
const EMIT_INTERVAL: Duration = Duration::from_millis(33);
const QUIET_INTERVAL: Duration = Duration::from_millis(100);
const RPC_E_CHANGED_MODE: i32 = 0x80010106_u32 as i32;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumPayload {
    pub bars: Vec<f32>,
    pub rms: f32,
    pub active: bool,
    pub peak: f32,
    pub timestamp: u64,
    pub source: String,
    pub error: Option<String>,
    pub debug: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDiagnostics {
    pub default_multimedia_peak: f32,
    pub default_console_peak: f32,
    pub multimedia_state: String,
    pub console_state: String,
    pub session_count: i32,
    pub active_session_count: i32,
    pub session_peaks: Vec<String>,
    pub log_path: String,
}

struct AudioState {
    running: Arc<AtomicBool>,
}

static STATE: OnceLock<Mutex<Option<AudioState>>> = OnceLock::new();

fn state() -> &'static Mutex<Option<AudioState>> {
    STATE.get_or_init(|| Mutex::new(None))
}

fn emit_spectrum_to_frontend(app: &AppHandle, payload: &SpectrumPayload) {
    let _ = app.emit(SPECTRUM_EVENT, payload);

    for window in app.webview_windows().values() {
        let _ = window.emit(SPECTRUM_EVENT, payload);
    }
}
#[tauri::command]
pub fn start_audio_spectrum(app: AppHandle) -> Result<(), String> {
    let mut guard = state().lock().map_err(|err| err.to_string())?;
    if guard.is_some() {
        return Ok(());
    }

    let running = Arc::new(AtomicBool::new(true));
    let thread_running = running.clone();
    thread::spawn(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            run_audio_loop(app, thread_running.clone())
        }));
        if result.is_err() {
            let _ = append_audio_diagnostics_log("audio thread panic");
        }

        if let Ok(mut guard) = state().lock() {
            if let Some(current) = guard.as_ref() {
                if Arc::ptr_eq(&current.running, &thread_running) {
                    *guard = None;
                }
            }
        }
    });

    *guard = Some(AudioState { running });
    Ok(())
}

#[tauri::command]
pub fn stop_audio_spectrum() -> Result<(), String> {
    let mut guard = state().lock().map_err(|err| err.to_string())?;
    if let Some(state) = guard.take() {
        state.running.store(false, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub fn audio_meter_snapshot() -> Result<SpectrumPayload, String> {
    Ok(SpectrumPayload {
        bars: quiet_bars(),
        rms: 0.0,
        active: false,
        peak: 0.0,
        timestamp: timestamp_ms(),
        source: "quiet".to_string(),
        error: None,
        debug: "snapshot-disabled: realtime spectrum uses loopback fft only".to_string(),
    })
}

#[tauri::command]
pub fn audio_diagnostics() -> Result<AudioDiagnostics, String> {
    unsafe {
        initialize_com_for_audio()?;
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|err| format!("create audio device enumerator failed: {err}"))?;

        let multimedia = endpoint_diagnostics(&enumerator, eMultimedia)?;
        let console = endpoint_diagnostics(&enumerator, eConsole)?;

        let diagnostics = AudioDiagnostics {
            default_multimedia_peak: multimedia.0,
            default_console_peak: console.0,
            multimedia_state: multimedia.1,
            console_state: console.1,
            session_count: multimedia.2,
            active_session_count: multimedia.3,
            session_peaks: multimedia.4,
            log_path: audio_diagnostics_log_path().display().to_string(),
        };

        let log_text = format!("{diagnostics:#?}");
        write_audio_diagnostics_log(&log_text)?;
        eprintln!("[jingzhuo-audio-diagnostics] {log_text}");
        Ok(diagnostics)
    }
}

fn run_audio_loop(app: AppHandle, running: Arc<AtomicBool>) {
    if let Err(err) = capture_loop(&app, &running) {
        let _ = append_audio_diagnostics_log(&format!("capture_loop error: {err}"));
        let payload = error_payload(err);
        emit_spectrum_to_frontend(&app, &payload);

        while running.load(Ordering::SeqCst) {
            let payload = quiet_payload("fallback-quiet".to_string());
            emit_spectrum_to_frontend(&app, &payload);
            thread::sleep(QUIET_INTERVAL);
        }
    }
}

fn capture_loop(app: &AppHandle, running: &AtomicBool) -> Result<(), String> {
    unsafe {
        initialize_com_for_audio()?;

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|err| format!("create audio device enumerator failed: {err}"))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .map_err(|err| format!("get default render endpoint failed: {err}"))?;
        if device
            .GetState()
            .map_err(|err| format!("read render endpoint state failed: {err}"))?
            != DEVICE_STATE_ACTIVE
        {
            return Err("default render endpoint is not active".to_string());
        }

        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|err| format!("activate audio client failed: {err}"))?;
        let mix_format = audio_client
            .GetMixFormat()
            .map_err(|err| format!("get mix format failed: {err}"))?;
        let format = *mix_format;
        let channels = usize::from(format.nChannels.max(1));
        let block_align = usize::from(format.nBlockAlign.max(1));
        let sample_rate = format.nSamplesPerSec.max(1);
        let sample_bits = format.wBitsPerSample;
        let format_tag = format.wFormatTag;
        let sample_format = sample_format_from_mix_format(mix_format);
        let format_debug = format!(
            "format tag={} rate={} bits={} channels={} block={} kind={sample_format:?}",
            format_tag, sample_rate, sample_bits, channels, block_align
        );

        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                10_000_000,
                0,
                mix_format,
                None,
            )
            .map_err(|err| format!("initialize loopback capture failed: {err}"))?;

        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|err| format!("get capture client failed: {err}"))?;
        audio_client
            .Start()
            .map_err(|err| format!("start loopback capture failed: {err}"))?;

        let _ = append_audio_diagnostics_log(&format!("loopback-start {format_debug}"));

        let mut ring = Vec::<f32>::with_capacity(RING_WINDOW_SIZE);
        let mut last_emit = Instant::now() - EMIT_INTERVAL;
        let mut last_log = Instant::now() - Duration::from_secs(1);
        let mut last_packet = Instant::now();
        let mut total_packets = 0_u64;
        let mut total_frames = 0_u64;

        while running.load(Ordering::SeqCst) {
            let mut packet_size = capture_client.GetNextPacketSize().unwrap_or(0);
            let mut packets_seen = 0_u32;
            let mut frames_seen = 0_u32;
            let mut silent_packets = 0_u32;

            while packet_size > 0 {
                packets_seen += 1;
                total_packets += 1;

                let mut data = std::ptr::null_mut();
                let mut frame_count = 0;
                let mut flags = Default::default();
                capture_client
                    .GetBuffer(&mut data, &mut frame_count, &mut flags, None, None)
                    .map_err(|err| format!("read loopback packet failed: {err}"))?;

                frames_seen += frame_count;
                total_frames += u64::from(frame_count);

                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    silent_packets += 1;
                } else if !data.is_null() && frame_count > 0 {
                    let decoded = decode_mono_samples(
                        data as *const u8,
                        frame_count as usize,
                        channels,
                        block_align,
                        sample_bits,
                        sample_format,
                    );
                    push_ring_samples(&mut ring, decoded);
                    last_packet = Instant::now();
                }

                capture_client
                    .ReleaseBuffer(frame_count)
                    .map_err(|err| format!("release loopback packet failed: {err}"))?;
                packet_size = capture_client.GetNextPacketSize().unwrap_or(0);
            }

            let active_window = !ring.is_empty() && last_packet.elapsed() < Duration::from_millis(250);
            let due = if active_window {
                last_emit.elapsed() >= EMIT_INTERVAL
            } else {
                last_emit.elapsed() >= QUIET_INTERVAL
            };

            if due {
                let payload = if active_window {
                    analyse_ring_window(
                        &ring,
                        sample_rate,
                        format!(
                            "{format_debug}; packets={packets_seen} frames={frames_seen} silent={silent_packets} total_packets={total_packets} total_frames={total_frames} ring={}",
                            ring.len()
                        ),
                    )
                } else {
                    quiet_payload(format!(
                        "{format_debug}; no_recent_loopback_packet ring={} total_packets={total_packets} total_frames={total_frames}",
                        ring.len()
                    ))
                };

                if last_log.elapsed() >= Duration::from_secs(1) {
                    log_payload(&payload);
                    last_log = Instant::now();
                }

                emit_spectrum_to_frontend(app, &payload);
                last_emit = Instant::now();
            }

            thread::sleep(Duration::from_millis(4));
        }

        let _ = audio_client.Stop();
        Ok(())
    }
}

fn decode_mono_samples(
    data: *const u8,
    frames: usize,
    channels: usize,
    block_align: usize,
    sample_bits: u16,
    sample_format: SampleFormatKind,
) -> Vec<f32> {
    if frames == 0 || channels == 0 {
        return Vec::new();
    }

    let bytes = unsafe { std::slice::from_raw_parts(data, frames.saturating_mul(block_align)) };
    let mut samples = Vec::with_capacity(frames);

    for frame in 0..frames {
        let frame_start = frame.saturating_mul(block_align);
        let mut mixed = 0.0_f32;
        for channel in 0..channels {
            mixed += read_sample(bytes, frame_start, channel, channels, sample_bits, sample_format);
        }
        samples.push((mixed / channels as f32).clamp(-1.0, 1.0));
    }

    samples
}

fn push_ring_samples(ring: &mut Vec<f32>, mut samples: Vec<f32>) {
    if samples.is_empty() {
        return;
    }

    ring.append(&mut samples);
    if ring.len() > RING_WINDOW_SIZE {
        let remove_count = ring.len() - RING_WINDOW_SIZE;
        ring.drain(0..remove_count);
    }
}

fn analyse_ring_window(ring: &[f32], sample_rate: u32, debug: String) -> SpectrumPayload {
    let n = ring.len().min(FFT_WINDOW_SIZE);
    if n < 256 || sample_rate == 0 {
        return quiet_payload(format!("{debug}; insufficient_window={n}"));
    }

    let start = ring.len().saturating_sub(n);
    let window = &ring[start..];
    let mut total = 0.0_f32;
    let mut peak = 0.0_f32;
    for sample in window {
        total += sample * sample;
        peak = peak.max(sample.abs());
    }

    let rms = (total / n as f32).sqrt().min(1.0);
    let active = rms > ACTIVE_RMS || peak > ACTIVE_PEAK;
    let bars = if active {
        dft_bars(window, sample_rate, rms)
    } else {
        quiet_bars()
    };

    SpectrumPayload {
        bars,
        rms,
        active,
        peak,
        timestamp: timestamp_ms(),
        source: "loopback".to_string(),
        error: None,
        debug: format!("{debug}; fft_window={n} active={active}"),
    }
}

fn dft_bars(window: &[f32], sample_rate: u32, rms: f32) -> Vec<f32> {
    let n = window.len();
    let nyquist = sample_rate as f32 / 2.0;
    let min_freq = 32.0_f32;
    let max_freq = nyquist.min(14_000.0).max(min_freq * 2.0);
    let loudness = (rms * 26.0).powf(0.55).clamp(0.08, 1.0);
    let mut raw = Vec::with_capacity(BAR_COUNT);

    for index in 0..BAR_COUNT {
        let x = index as f32 / (BAR_COUNT - 1) as f32;
        let freq = min_freq * (max_freq / min_freq).powf(x);
        let bin = ((freq * n as f32) / sample_rate as f32).round().max(1.0);
        let angle_step = 2.0 * std::f32::consts::PI * bin / n as f32;
        let mut real = 0.0_f32;
        let mut imag = 0.0_f32;

        for (sample_index, sample) in window.iter().enumerate() {
            let hann = 0.5
                - 0.5
                    * ((2.0 * std::f32::consts::PI * sample_index as f32)
                        / (n.saturating_sub(1).max(1) as f32))
                        .cos();
            let angle = angle_step * sample_index as f32;
            let value = sample * hann;
            real += value * angle.cos();
            imag -= value * angle.sin();
        }

        let low_bias = 1.22 - x * 0.34;
        let energy = (real.mul_add(real, imag * imag)).sqrt() / n as f32;
        raw.push((energy * low_bias).max(0.0));
    }

    let max_raw = raw
        .iter()
        .copied()
        .fold(0.0_f32, |max_value, value| max_value.max(value));
    if max_raw <= 0.000_000_1 {
        return quiet_bars();
    }

    raw.into_iter()
        .map(|value| {
            let normalized = (value / max_raw).clamp(0.0, 1.0);
            let shaped = normalized.powf(0.46);
            (0.03 + shaped * loudness * 0.94).clamp(0.03, 1.0)
        })
        .collect()
}

fn endpoint_diagnostics(
    enumerator: &IMMDeviceEnumerator,
    role: windows::Win32::Media::Audio::ERole,
) -> Result<(f32, String, i32, i32, Vec<String>), String> {
    unsafe {
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, role)
            .map_err(|err| format!("get render endpoint failed: {err}"))?;
        let state = device
            .GetState()
            .map_err(|err| format!("read render endpoint state failed: {err}"))?;

        let meter: IAudioMeterInformation = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|err| format!("activate endpoint meter failed: {err}"))?;
        let peak = meter
            .GetPeakValue()
            .map_err(|err| format!("read endpoint meter peak failed: {err}"))?
            .clamp(0.0, 1.0);

        let manager: IAudioSessionManager2 = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|err| format!("activate session manager failed: {err}"))?;
        let sessions = manager
            .GetSessionEnumerator()
            .map_err(|err| format!("enumerate audio sessions failed: {err}"))?;
        let session_count = sessions
            .GetCount()
            .map_err(|err| format!("read session count failed: {err}"))?;
        let mut active_count = 0;
        let mut peaks = Vec::new();

        for index in 0..session_count {
            let session = match sessions.GetSession(index) {
                Ok(value) => value,
                Err(err) => {
                    peaks.push(format!("#{index} session-error={err}"));
                    continue;
                }
            };
            let state = session.GetState().unwrap_or_default();
            if state == AudioSessionStateActive {
                active_count += 1;
            }
            let process_id = session
                .cast::<IAudioSessionControl2>()
                .ok()
                .and_then(|control| control.GetProcessId().ok())
                .unwrap_or(0);
            let peak = session
                .cast::<IAudioMeterInformation>()
                .ok()
                .and_then(|meter| meter.GetPeakValue().ok())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);

            peaks.push(format!(
                "#{index} pid={process_id} state={} peak={peak:.5}",
                state.0
            ));
        }

        Ok((
            peak,
            format!("raw={state:?} active={}", state == DEVICE_STATE_ACTIVE),
            session_count,
            active_count,
            peaks,
        ))
    }
}

fn read_sample(
    bytes: &[u8],
    frame_start: usize,
    channel: usize,
    _channels: usize,
    sample_bits: u16,
    sample_format: SampleFormatKind,
) -> f32 {
    let bytes_per_sample = usize::from((sample_bits / 8).max(1));
    let offset = frame_start + channel.saturating_mul(bytes_per_sample);
    if offset + bytes_per_sample > bytes.len() {
        return 0.0;
    }

    if sample_format == SampleFormatKind::Float && sample_bits == 32 {
        let mut raw = [0_u8; 4];
        raw.copy_from_slice(&bytes[offset..offset + 4]);
        return f32::from_le_bytes(raw).clamp(-1.0, 1.0);
    }

    if sample_format == SampleFormatKind::Pcm {
        return match sample_bits {
            16 => {
                let mut raw = [0_u8; 2];
                raw.copy_from_slice(&bytes[offset..offset + 2]);
                i16::from_le_bytes(raw) as f32 / i16::MAX as f32
            }
            24 => {
                let raw = [
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    if bytes[offset + 2] & 0x80 == 0 { 0 } else { 0xff },
                ];
                i32::from_le_bytes(raw) as f32 / 8_388_607.0
            }
            32 => {
                let mut raw = [0_u8; 4];
                raw.copy_from_slice(&bytes[offset..offset + 4]);
                i32::from_le_bytes(raw) as f32 / i32::MAX as f32
            }
            _ => 0.0,
        };
    }

    0.0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SampleFormatKind {
    Float,
    Pcm,
    Unknown,
}

fn sample_format_from_mix_format(
    format: *const windows::Win32::Media::Audio::WAVEFORMATEX,
) -> SampleFormatKind {
    if format.is_null() {
        return SampleFormatKind::Unknown;
    }

    let tag = unsafe { (*format).wFormatTag };
    if tag == WAVE_FORMAT_IEEE_FLOAT as u16 {
        return SampleFormatKind::Float;
    }

    if tag == WAVE_FORMAT_PCM as u16 {
        return SampleFormatKind::Pcm;
    }

    if tag == WAVE_FORMAT_EXTENSIBLE as u16 {
        let extensible = format as *const WAVEFORMATEXTENSIBLE;
        if extensible.is_null() {
            return SampleFormatKind::Unknown;
        }

        let subformat = unsafe { (*extensible).SubFormat };
        if subformat == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
            return SampleFormatKind::Float;
        }

        if subformat == KSDATAFORMAT_SUBTYPE_PCM {
            return SampleFormatKind::Pcm;
        }
    }

    SampleFormatKind::Unknown
}

fn quiet_payload(debug: String) -> SpectrumPayload {
    SpectrumPayload {
        bars: quiet_bars(),
        rms: 0.0,
        active: false,
        peak: 0.0,
        timestamp: timestamp_ms(),
        source: "quiet".to_string(),
        error: None,
        debug,
    }
}

fn error_payload(error: String) -> SpectrumPayload {
    SpectrumPayload {
        bars: quiet_bars(),
        rms: 0.0,
        active: false,
        peak: 0.0,
        timestamp: timestamp_ms(),
        source: "error".to_string(),
        error: Some(error),
        debug: "capture-error".to_string(),
    }
}

fn quiet_bars() -> Vec<f32> {
    vec![0.03; BAR_COUNT]
}

fn log_payload(payload: &SpectrumPayload) {
    let (min_bar, max_bar, avg_bar) = bar_stats(&payload.bars);
    let _ = append_audio_diagnostics_log(&format!(
        "emit source={} active={} rms={:.5} peak={:.5} minBar={:.5} maxBar={:.5} avgBar={:.5} bars={} debug={}",
        payload.source,
        payload.active,
        payload.rms,
        payload.peak,
        min_bar,
        max_bar,
        avg_bar,
        payload.bars.len(),
        payload.debug
    ));
}

fn bar_stats(bars: &[f32]) -> (f32, f32, f32) {
    if bars.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let mut min_bar = f32::MAX;
    let mut max_bar = f32::MIN;
    let mut total = 0.0_f32;
    for value in bars {
        min_bar = min_bar.min(*value);
        max_bar = max_bar.max(*value);
        total += *value;
    }

    (min_bar, max_bar, total / bars.len() as f32)
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn audio_diagnostics_log_path() -> PathBuf {
    if let Ok(appdata) = env::var("APPDATA") {
        PathBuf::from(appdata)
            .join("Jingzhuo")
            .join("audio-diagnostics.log")
    } else {
        PathBuf::from("audio-diagnostics.log")
    }
}

fn write_audio_diagnostics_log(text: &str) -> Result<(), String> {
    let path = audio_diagnostics_log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, text).map_err(|err| err.to_string())
}

fn append_audio_diagnostics_log(text: &str) -> Result<(), String> {
    let path = audio_diagnostics_log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    writeln!(file, "{text}").map_err(|err| err.to_string())
}

fn initialize_com_for_audio() -> Result<(), String> {
    unsafe {
        match CoInitializeEx(None, COINIT_MULTITHREADED).ok() {
            Ok(()) => Ok(()),
            Err(err) if err.code().0 == RPC_E_CHANGED_MODE => Ok(()),
            Err(err) => Err(format!("initialize audio COM failed: {err}")),
        }
    }
}

