use crate::config::AppConfig;
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const SLIDESHOW_REG_KEY: &str = r"HKCU\Control Panel\Personalization\Desktop Slideshow";
const SLIDESHOW_EXPLORER_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers\Slideshow";
const DEFAULT_INTERVAL_MS: u32 = 60_000;
const SPI_SETDESKWALLPAPER: u32 = 0x0014;
const SPIF_UPDATEINIFILE: u32 = 0x0001;
const SPIF_SENDCHANGE: u32 = 0x0002;

static STATE: OnceLock<Mutex<WallpaperState>> = OnceLock::new();

#[derive(Clone, Debug)]
struct WallpaperState {
    default_folder: PathBuf,
    alternate_folder: PathBuf,
    current_folder: PathBuf,
    current_images: Vec<PathBuf>,
    current_index: usize,
    interval_ms: u32,
}

impl WallpaperState {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            default_folder: PathBuf::from(&config.default_wallpaper_folder),
            alternate_folder: PathBuf::from(&config.alternate_wallpaper_folder),
            current_folder: PathBuf::from(&config.default_wallpaper_folder),
            current_images: Vec::new(),
            current_index: 0,
            interval_ms: if config.wallpaper_interval_ms == 0 {
                DEFAULT_INTERVAL_MS
            } else {
                config.wallpaper_interval_ms
            },
        }
    }

    fn current_label(&self) -> String {
        folder_label(&self.current_folder)
    }

    fn activate_folder(&mut self, folder: PathBuf) -> Result<(), String> {
        let images = scan_images(&folder)?;
        if images.is_empty() {
            return Err(format!("no wallpaper images found in {}", folder.display()));
        }

        self.current_folder = folder;
        self.current_images = images;
        self.current_index = random_index(self.current_images.len());

        set_slideshow_registry(&self.current_folder, self.interval_ms)?;
        apply_wallpaper(&self.current_images[self.current_index])?;
        Ok(())
    }

    fn next_wallpaper(&mut self) -> Result<String, String> {
        if self.current_images.is_empty() {
            self.current_images = scan_images(&self.current_folder)?;
        }

        if self.current_images.is_empty() {
            return Err(format!(
                "no wallpaper images found in {}",
                self.current_folder.display()
            ));
        }

        self.current_index = (self.current_index + 1) % self.current_images.len();
        apply_wallpaper(&self.current_images[self.current_index])?;
        Ok(self.current_images[self.current_index].display().to_string())
    }
}

fn state() -> &'static Mutex<WallpaperState> {
    STATE.get_or_init(|| {
        Mutex::new(WallpaperState {
            default_folder: PathBuf::new(),
            alternate_folder: PathBuf::new(),
            current_folder: PathBuf::new(),
            current_images: Vec::new(),
            current_index: 0,
            interval_ms: DEFAULT_INTERVAL_MS,
        })
    })
}

pub fn initialize_state(config: &AppConfig) -> Result<String, String> {
    let mut guard = state().lock().map_err(|err| err.to_string())?;
    *guard = WallpaperState::from_config(config);
    Ok(guard.current_label())
}

#[tauri::command]
pub fn next_wallpaper() -> Result<String, String> {
    let mut guard = state().lock().map_err(|err| err.to_string())?;
    guard.next_wallpaper()
}

#[tauri::command]
pub fn switch_wallpaper_folder() -> Result<String, String> {
    let mut guard = state().lock().map_err(|err| err.to_string())?;
    let target_folder = if guard.current_folder == guard.default_folder {
        guard.alternate_folder.clone()
    } else {
        guard.default_folder.clone()
    };
    guard.activate_folder(target_folder)?;
    Ok(guard.current_label())
}

fn scan_images(folder: &Path) -> Result<Vec<PathBuf>, String> {
    if !folder.exists() {
        return Err(format!("folder does not exist: {}", folder.display()));
    }

    let mut images = Vec::new();
    scan_images_recursive(folder, &mut images)?;
    images.sort();
    Ok(images)
}

fn scan_images_recursive(folder: &Path, images: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(folder).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let file_type = entry.file_type().map_err(|err| err.to_string())?;

        if file_type.is_dir() {
            scan_images_recursive(&entry.path(), images)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };

        let ext = ext.to_ascii_lowercase();
        if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp") {
            images.push(path);
        }
    }

    Ok(())
}

fn random_index(len: usize) -> usize {
    if len <= 1 {
        return 0;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    (nanos % len as u128) as usize
}

fn set_slideshow_registry(folder: &Path, interval_ms: u32) -> Result<(), String> {
    let folder_str = folder.to_string_lossy().to_string();
    run_reg_add(
        SLIDESHOW_REG_KEY,
        "SlideshowSourceFolder",
        "REG_SZ",
        &folder_str,
    )?;
    run_reg_add(
        SLIDESHOW_EXPLORER_KEY,
        "ImagesFolderPath",
        "REG_SZ",
        &folder_str,
    )?;
    run_reg_add(
        SLIDESHOW_EXPLORER_KEY,
        "Interval",
        "REG_DWORD",
        &interval_ms.to_string(),
    )?;
    run_reg_add(SLIDESHOW_EXPLORER_KEY, "Shuffle", "REG_DWORD", "1")?;
    Ok(())
}

fn run_reg_add(key: &str, value_name: &str, value_type: &str, value: &str) -> Result<(), String> {
    let output = Command::new("reg")
        .args([
            "add", key, "/v", value_name, "/t", value_type, "/d", value, "/f",
        ])
        .output()
        .map_err(|err| err.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn apply_wallpaper(path: &Path) -> Result<(), String> {
    let path_utf16: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            path_utf16.as_ptr() as *mut c_void,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };

    if ok == 0 {
        return Err(format!("failed to apply wallpaper: {}", path.display()));
    }

    Ok(())
}

fn folder_label(folder: &Path) -> String {
    folder
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| folder.display().to_string())
}

unsafe extern "system" {
    fn SystemParametersInfoW(
        ui_action: u32,
        ui_param: u32,
        pv_param: *mut c_void,
        win_ini: u32,
    ) -> i32;
}
