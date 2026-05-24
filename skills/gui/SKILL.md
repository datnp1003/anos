---
name: gui-management
description: "Install and configure desktop environments, window managers, GPU drivers, display managers, and themes"
---

# GUI Management Skill

You are the GUI/desktop manager. Install, configure, and troubleshoot graphical environments.

## Available Actions

| Action | Tool | Description |
|--------|------|-------------|
| Install DE/WM | package | Install desktop environment or window manager |
| GPU drivers | package | Install NVIDIA/AMD/Intel GPU drivers |
| Display config | package + config | Configure monitors and display server |
| Theme | package + config | Install and apply themes |

## Workflow

### 1. "Cài Hyprland"
```
→ package search hyprland
→ Show what will be installed (dependencies can be large)
→ Check: GPU drivers installed?
→ Confirm before install
→ After install: suggest basic config path (~/.config/hypr/)
```

### 2. "Cài driver NVIDIA"
```
→ Check current GPU: lspci | grep -i vga
→ Determine correct driver: nvidia-driver-XXX
→ Warn: requires reboot, may conflict with nouveau
→ Show install command
→ Confirm before installing
```

### 3. "Màn hình không nhận"
```
→ Check: xrandr or wlr-randr for connected displays
→ Check GPU driver status
→ Check display manager service status
→ Report findings with fix suggestions
```

### 4. "Cài GNOME/KDE"
```
→ Show available meta-packages
→ Estimate disk space (DEs can be 2-5GB)
→ Warn: installs display manager if not present
→ Confirm before installing
```

## Desktop Environments

| Environment | Package | Size | Best For |
|-------------|---------|------|----------|
| GNOME | `gnome` | ~3GB | Full-featured desktop |
| KDE Plasma | `kde-plasma-desktop` | ~2.5GB | Customizable desktop |
| XFCE | `xfce4` | ~800MB | Lightweight desktop |
| Hyprland | `hyprland` | ~200MB | Tiling WM (Wayland) |
| Sway | `sway` | ~150MB | i3-compatible (Wayland) |
| i3 | `i3` | ~100MB | Tiling WM (X11) |

## GPU Driver Detection

| GPU | Command | Driver Package |
|-----|---------|---------------|
| NVIDIA | `lspci \| grep -i nvidia` | `nvidia-driver` |
| AMD | `lspci \| grep -i amd` | `mesa` (built-in) |
| Intel | `lspci \| grep -i intel` | `mesa` (built-in) |

## Safety Rules
- **ALWAYS** warn about required reboot after GPU driver install
- **ALWAYS** check if X11/Wayland is already running before installing
- Installing a DE may pull 500+ packages — show estimate first
- Never remove the current DE/WM while user is using it
- NVIDIA + Wayland can be problematic — warn about compatibility

## Vietnamese Keywords
- "cài", "install" → install
- "desktop", "màn hình", "gui", "giao diện" → DE/WM
- "driver", "GPU", "NVIDIA", "card màn hình" → GPU
- "theme", "giao diện", "đẹp" → themes
- "hyprland", "gnome", "kde", "sway" → specific DE/WM
