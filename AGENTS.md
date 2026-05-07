# AGENTS.md

## Project

This project is a Windows desktop overlay prototype named "静桌".

It is not a full shell replacement. It should not replace Windows Explorer or the taskbar. It should provide a quiet no-icon desktop layer with time, weather, focus timer, command palette, and AI chat panel.

## Product Principles

* Default state must be quiet.
* No desktop icons in the main visual layer.
* Use Windows auto-hide taskbar instead of replacing the taskbar.
* Keyboard-first interaction.
* Ctrl + Space opens command palette.
* Ctrl + T opens AI panel.
* Esc closes the active panel.
* Visual style: dark, calm, restrained, minimal, subtle East Asian study-room feeling.
* Avoid cyber neon, cute style, heavy HUD, excessive glassmorphism, and fake ancient decoration.

## v0.1 Scope

Implement only:

* Desktop overlay shell
* Static wallpaper
* Time display
* Weather text with fake data
* Focus countdown
* Subtle weather animation
* Command palette with mock actions
* AI panel with mock replies
* Basic local settings

Do not implement:

* Taskbar replacement
* Window management
* Plugin marketplace
* Real file automation
* AI system control
* OpenClaw integration
* Complex weather API
* Complex theme system

## Engineering Expectations

* Keep modules small and readable.
* Prefer simple working prototype over over-engineering.
* Add TODO comments for future features instead of implementing them prematurely.
* Do not introduce unnecessary dependencies.
* Update README when changing setup or run commands.
* Run available lint, typecheck, build, or tests before finishing.
