interface FunctionWheelProps {
  currentFolderLabel: string;
  onNextWallpaper: () => void;
  onSwitchWallpaperFolder: () => void;
  onClose: () => void;
  exiting?: boolean;
}

export function FunctionWheel({
  currentFolderLabel,
  onNextWallpaper,
  onSwitchWallpaperFolder,
  onClose,
  exiting = false
}: FunctionWheelProps) {
  return (
    <section
      className={`wallpaper-quick-panel ${exiting ? "is-exiting" : ""}`}
      aria-label="壁纸快控"
    >
      <div className="wallpaper-quick-header">
        <span className="wallpaper-quick-title">壁纸 · {currentFolderLabel}</span>
        <button className="wallpaper-quick-esc" type="button" onClick={onClose}>
          Esc
        </button>
      </div>

      <div className="wallpaper-quick-actions">
        <button className="wallpaper-quick-action" type="button" onClick={onNextWallpaper}>
          下一张壁纸
        </button>
        <button className="wallpaper-quick-action" type="button" onClick={onSwitchWallpaperFolder}>
          切换画卷
        </button>
      </div>
    </section>
  );
}
