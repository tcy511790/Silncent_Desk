# CLEANUP_PLAN

Scope note: the requested `legacy_raw/` directory does not exist in the current workspace. The actual legacy source directory is `legacy/`, so this plan maps `legacy/` as the legacy raw area. No files have been deleted or moved by this plan.

## Classification

### keep_reference

These projects are worth keeping as design or implementation references, but they should not be merged directly into the clean app.

| Original path | Reason |
|---|---|
| `legacy/ef/win11-widget` | Closest match to the `image_display` reference image. Contains the time/date/weather card, wallpaper switch icon, and music spectrum bar experiments. Valuable for understanding the old visual intent and possible module behavior. Not safe to merge directly because it uses raw Win32 windows, Direct2D, `static mut`, fixed pixel layout, and has garbled Chinese strings. |
| `legacy/d` | Most complete old desktop widget prototype. Contains config persistence, random wallpaper selection, wallpaper rotation, tray menu, draggable layered window, day/night mode, and animation experiments. Useful as a reference for Windows wallpaper behavior and config shape. Not safe to merge directly because it is a separate native widget architecture and contains hard-coded local wallpaper folders. |
| `legacy/image_display` | Reference screenshot describing the old desired visual blocks: time/date/weather area, wallpaper switch control, and music bar. Useful as product/design evidence, not source code. |

### extract_later

These projects may contain isolated logic worth extracting later, after the clean app structure is stable.

| Original path | Reason |
|---|---|
| `legacy/ef/wallpaper-switcher` | Contains folder scanning, random wallpaper selection, simple config text handling, and `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` usage. Useful for a future Tauri command implementation. The current structure and UI should not be reused. |
| `legacy/ef/rust-windows-app` | Contains Windows registry reads for wallpaper slideshow folders, shuffle state, and interval. Potentially useful for future diagnostics or settings import, but outside v0.1 scope. |
| `legacy/ef/liuli_effect` | Contains a small layered-window icon morph/breath animation. Possible future reference for subtle icon animation, but the visual direction is not core to the current quiet desktop overlay. |
| `legacy/ef/preprocess` | CLI image contour preprocessing tool. May be useful only if future icon/vector-mask generation is needed. Not needed for the current app. |
| `legacy/ef/spectrum_bar.rs` | Standalone music spectrum experiment. Its visual layout and FFT idea overlap with `win11-widget`, but it appears detached from a complete Cargo project at this level. Keep only as later reference. |
| `legacy/ef/main (2).rs` | Standalone time/date/weather widget draft. Useful only as a secondary reference for the old Direct2D text layout. |

### obsolete_drafts

These are clearly low-value drafts, generated files, incomplete tests, or local tool artifacts. They should not be used as implementation sources.

| Original path | Reason |
|---|---|
| `legacy/ef/test-project` | Minimal hello-world Rust project. No meaningful product logic. |
| `legacy/ef/tools` | Cargo project currently contains only a hello-world style entry point; root-level `extract_icon.rs` is detached and not integrated. Low value. |
| `legacy/ef/新建文件夹` | Contains generated Gemini draft code only. No stable project structure. |
| `legacy/ef/gemini-code-1776948105532.rs` | Generated standalone draft. Not integrated into a project. |
| `legacy/ef/preprocess_bin.rs` | Detached preprocessing draft outside the actual `preprocess` Cargo project. |
| `legacy/ef/prepare_icons.rs` | Detached one-off generator script. Only historical value. |
| `legacy/ef/analyze_icon.rs` | Detached analysis script. Not part of clean app. |
| `legacy/ef/enum_windows.rs` | Detached Win32 experiment. Not part of clean app. |
| `legacy/ef/rustup-init.exe` | Tool installer binary. Should not live in source history. |
| `legacy/ef/R.points` | Generated/intermediate point data. No clear source role. |
| `legacy/ef/Y.points` | Generated/intermediate point data. No clear source role. |
| `legacy/ef/**/target` | Rust build outputs. These are generated artifacts, not source. |

### unknown

No major project directories are currently unknown after the audit. If new directories appear under the legacy raw area, classify them here first before moving them into a stronger category.

| Original path | Reason |
|---|---|
| none | Current known legacy paths have enough context to classify. |

### assets

Images, icons, screenshots, and generated visual assets. These should be archived as assets even when they sit beside code.

| Original path | Reason |
|---|---|
| `legacy/d/assets` | Contains `moon_cloud.png` and `sun_cloud.png`, used by the old wallpaper switch widget. |
| `legacy/d/28.png` | Old source image/icon asset used by experiments. |
| `legacy/d/29.png` | Old source image/icon asset used by experiments. |
| `legacy/ef/28.png` | Duplicate or related old source image/icon asset. |
| `legacy/ef/29.png` | Duplicate or related old source image/icon asset. |
| `legacy/image_display/*.png` | Screenshot/reference image showing time/date/weather, wallpaper switching, and music bar layout. |

## Proposed Archive Layout

When approved, move folders only and preserve original directory names under:

```text
legacy_archive/
  keep_reference/
    README.md
    d/
    win11-widget/
    image_display/
  extract_later/
    README.md
    wallpaper-switcher/
    rust-windows-app/
    liuli_effect/
    preprocess/
  obsolete_drafts/
    README.md
    test-project/
    tools/
    新建文件夹/
  unknown/
    README.md
  assets/
    README.md
```

## Move Constraints

- Do not delete any file.
- Move folders only.
- Create `legacy_archive/` before moving.
- Preserve original directory names when moved.
- Add one `README.md` inside each category directory.
- Root-level loose files under `legacy/ef` are not folders. Because the requested operation is folder-only, leave those files in place unless a later instruction explicitly allows moving files.
- Asset files that are loose files, such as `legacy/d/28.png`, `legacy/d/29.png`, `legacy/ef/28.png`, and `legacy/ef/29.png`, cannot be moved under the current folder-only rule. Keep them in place unless later bundled by moving their parent folder.

## Recommended Migration Order

1. Archive reference screenshot and stable old projects first: `image_display`, `d`, `win11-widget`.
2. Archive extract-later projects: `wallpaper-switcher`, `rust-windows-app`, `liuli_effect`, `preprocess`.
3. Archive clearly obsolete folder projects: `test-project`, `tools`, `新建文件夹`.
4. Leave loose root files in `legacy/ef` untouched until file moves are explicitly allowed.
5. After archiving, continue clean-app development only in the current Tauri project, using archived code as read-only reference.
