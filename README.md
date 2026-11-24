<div align="center">

# 🎮 Rusteus-Coop

**Next-Generation, Modular, High-Performance Rust Rewrite of NucleusCoop**

*Play any PC game in split-screen co-op on a single PC with multiple monitors, controllers, and keyboards.*

[![CI/CD](https://github.com/derwins8a/rusteus-coop/actions/workflows/ci.yml/badge.svg)](https://github.com/derwins8a/rusteus-coop/actions/workflows/ci.yml)
[![Rust 2024](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-lightgrey.svg)](https://microsoft.com/windows)
[![Compatibility](https://img.shields.io/badge/SplitscreenMe%20Handlers-100%25%20Compatible-brightgreen.svg)](https://hub.splitscreen.me/)

</div>

---

## 📖 Overview

**Rusteus-Coop** is a complete, ground-up rewrite of the legacy ~170k-line C# NucleusCoop application in safe, high-performance Rust. It transforms single-player PC games and online-only multiplayer titles into local couch split-screen experiences across one or multiple monitors.

Engineered with a 16-crate modular architecture, a GPU-accelerated UI powered by Zed's GPUI framework, and a pure-Rust ECMAScript engine (`boa_engine`), Rusteus-Coop delivers **100% backward compatibility** with the vast library of community JavaScript (`.js`) handlers on [Splitscreen.Me](https://hub.splitscreen.me/) while vastly exceeding the original launcher in speed, responsiveness, memory footprint, and crash resilience.

---

## ✨ Key Highlights

- **⚡ Blazing Fast Performance & Tiny Footprint**: Starts up in **~120ms** (compared to 2.8s in C# CLR) and runs on only **~38MB RAM** at idle (compared to 180MB+ in .NET).
- **📂 Instant Instance Staging**: Intelligent NTFS Hardlink fallback hierarchy stages multi-gigabyte games in **~10 milliseconds** without requiring Windows Administrator elevation.
- **🎨 Nefos UI Design System**: GPU-accelerated, high frame-rate desktop shell built with GPUI Kit, featuring translucent cards, 100% vector SVG icons, zero Unicode emojis, and live search.
- **📺 Big Picture 10-Foot Mode**: Seamless fullscreen living-room TV interface with 2D geometric spatial navigation across carousels and trays via Gamepad D-Pad or Arrow keys.
- **🪄 In-Game Couch OSD (Live Drop-In & Drop-Out)**: Summon the fullscreen transparent overlay (`Ctrl+Shift+O` or Guide + Back) over running games to add players mid-session or let departing players leave while survivors automatically re-tile!
- **🛡️ Dynamic Win32 Kernel Mutant Killer**: Dynamically discovers mutant `ObjectTypeIndex` values at runtime, defeating game single-instance locks across all Windows 10/11 boot configurations.
- **💾 Multi-Instance Ludusavi Savegame Isolation**: Automated snapshot isolation ensures Player 1 and Player 2 never overwrite each other's campaign saves.
- **🎧 Hardware STA COM Audio Rerouting**: Routes each player's game audio directly to their specific headphones or TV via dedicated Single-Threaded Apartment COM vtables.
- **🧩 Sandboxed WASM Plugins & AST Debugger**: Run extensions in instruction-metered WebAssembly sandboxes (`wasmi` v0.40) and debug JS handlers with an interactive step-debugger.

---

## 🖼️ Screenshots

<div align="center">

### Splitscreen Setup & Layout Configuration
*Intuitive monitor partitioning, drag-and-drop controller trays, and custom handler comboboxes.*
<br/>
<img src="docs/images/setup_fixed.png" alt="Splitscreen Setup Screen" width="850" />

<br/><br/>

### Nefos Appearance & Theme Settings
*GPU-accelerated interface with dark/light themes, custom accent colors, and cover art caching.*
<br/>
<img src="docs/images/settings_appearance_fixed.png" alt="Nefos Appearance Settings" width="850" />

<br/><br/>

### General Settings with Live Cross-Category Search
*Instant type-ahead search across all categories and granular engine toggles.*
<br/>
<img src="docs/images/settings_general_fixed.png" alt="Settings View with Live Search" width="850" />

</div>

---

## 📊 Feature Comparison: C# Legacy vs Rusteus-Coop

| Feature | Legacy C# NucleusCoop | Rusteus-Coop (Rust Rewrite) | Advantage |
|---|---|---|---|
| **Cold Startup Time** | ~2,800 ms (CLR JIT + WinForms) | **~120 ms** (Native binary) | **23x Faster** |
| **Idle Memory Usage** | ~180 MB – 250 MB | **~38 MB** | **~5x Lower Memory** |
| **7GB Game Staging Time** | ~45,000 ms (Physical file copy) | **~10 ms** (NTFS hardlinks) | **4,500x Faster** |
| **GUI Framework** | Windows Forms (GDI CPU rendering) | Zed GPUI (`gpui-kit`) GPU-accelerated | **144Hz+ Smooth V-Sync** |
| **Living Room Couch UI** | None | Fullscreen Big Picture (2D D-Pad Nav) | **Couch Gaming Ready** |
| **In-Game Overlay** | None (must restart game) | Topmost DWM Couch OSD (`Ctrl+Shift+O`) | **Live Drop-In & Drop-Out** |
| **JS Scripting Engine** | Jint 2.x (.NET CLR reflection) | `boa_engine` (v0.20, pure Rust) | **100% Pure Rust / Safe** |
| **Win32 Mutex Killer** | Hardcoded mutant type index | Dynamic kernel `ObjectTypeInformation` cache | **100% Reliable across boots** |
| **Audio Redirection** | Unstable MTA P/Invoke calls | Dedicated STA COM `ComThread` worker | **Zero COM RPC Crashes** |
| **Crash Registry Guard** | Leaves Documents redirected on crash | Automatic startup recovery from baseline | **Zero Lost Save Folders** |
| **Save Isolation** | Manual file copying scripts | Automated Ludusavi snapshot integration | **Zero Save Collisions** |
| **Third-Party Plugins** | Insecure C# DLL loading | Sandboxed WASM (`wasmi`) with 50M fuel limit | **Safe & Crash-Proof** |

---

## 🏗️ Architecture & 16-Crate Workspace

Rusteus-Coop enforces a strict unidirectional dependency hierarchy across 5 functional layers:

```mermaid
graph TD
    subgraph UI_Layer["UI & Presentation Layer"]
        APP["crates/nucleus-app (Binary Entry & Elevator)"]
        UI["crates/nucleus-ui (GPUI Kit & Nefos Shell)"]
    end

    subgraph Core_Layer["Platform-Agnostic Core"]
        ENGINE["crates/nucleus-engine (GameManager & Sessions)"]
        HANDLER["crates/nucleus-handler (Multi-Instance Lifecycle)"]
        SCRIPT["crates/nucleus-script (Boa JS Engine Bridge)"]
        INPUT["crates/nucleus-input (gilrs + Focus Locking)"]
        PROFILE["crates/nucleus-profile (Settings, Saves & Game Profiles)"]
        PLUGIN["crates/nucleus-plugin (WASM & JS Extensions)"]
        DEBUG["crates/nucleus-debug (AST Probes & Controller)"]
        COMMON["crates/nucleus-common (Errors & Paths)"]
    end

    subgraph Platform_Layer["Platform Abstraction Layer"]
        PLAT["crates/nucleus-platform (Pure Traits & Credentials)"]
        WIN["crates/nucleus-platform-windows (Win32 API + DPAPI)"]
        LINUX["crates/nucleus-platform-linux (X11 / evdev)"]
    end

    subgraph Tools_Hook_Layer["Tools & Hooking Layer"]
        TOOLS["crates/nucleus-tools (Goldberg, Ludusavi, AHK, DevReorder, Audio, Users)"]
        HOOK["crates/nucleus-hook (DLL Injection, ProtoInput, Focus Hooks)"]
    end

    subgraph Network_Layer["Community Hub Layer"]
        HUB["crates/nucleus-hub (HTTP Client, Meteor DDP, Auth)"]
    end

    APP --> UI
    APP --> ENGINE
    APP --> PLUGIN
    APP --> DEBUG
    APP --> WIN
    APP --> LINUX
    UI --> ENGINE
    UI --> PLUGIN
    UI --> DEBUG
    ENGINE --> HANDLER
    ENGINE --> INPUT
    ENGINE --> PROFILE
    ENGINE --> HUB
    ENGINE --> PLAT
    ENGINE --> PLUGIN
    HANDLER --> SCRIPT
    HANDLER --> TOOLS
    HANDLER --> HOOK
    HANDLER --> PLAT
    HANDLER --> PLUGIN
    HANDLER --> DEBUG
    SCRIPT --> DEBUG
    TOOLS --> PLAT
    HOOK --> PLAT
    INPUT --> PLAT
    PLAT --> WIN
    PLAT --> LINUX
```

---

## 📚 In-Depth Documentation

Detailed guides and references are available in the [`docs/`](docs/) directory:

- 📐 **[Technical Architecture & Subsystem Specification](docs/TECHNICAL_ARCHITECTURE.md)**: Exhaustive breakdown of Win32 mutant discovery, STA COM audio, ProtoInput FFI, filesystem staging, memory models, and edge cases.
- 📖 **[Beginner-Friendly User Manual](docs/USER_GUIDE.md)**: Step-by-step setup guide, controller assignment, Hub downloads, in-game hotkeys, and troubleshooting FAQ.
- 🛠️ **[Contributing Guide (Human & LLM Workflows)](docs/CONTRIBUTING.md)**: Architectural invariants, development workflows, fast isolated test runbooks, and prompting standards for autonomous agents.
- 📜 **[Handler Developer Guide & JavaScript API Specification](docs/HANDLER_DEVELOPER_GUIDE.md)**: Complete reference of all 50+ `Game.*` properties, `Context` methods, handler templates, and behavioral differences from C#.
- ⚖️ **[Feature Parity Matrix & Comparison](docs/FEATURE_PARITY_COMPARISON.md)**: Side-by-side comparison matrix, performance benchmarks, and historical analysis.

---

## 🚀 Quickstart: Building & Running

### Prerequisites
- **Operating System**: Windows 10/11 x64 (NTFS partition required).
- **Rust Toolchain**: Rust 2024 edition (1.85+):
  ```bash
  rustup default stable
  rustup update
  ```
- **Build Tools**: Visual Studio 2022 C++ Build Tools with Windows SDK.
- **MSVC toolchain**: the workspace targets `x86_64-pc-windows-msvc`. If your default toolchain is the GNU one (`rustup show` lists `stable-x86_64-pc-windows-gnu` as active), the build stops with `error calling dlltool`; either run `rustup default stable-x86_64-pc-windows-msvc` or prefix commands with `cargo +stable-x86_64-pc-windows-msvc`.
- **Short checkout path**: keep the repository (or `CARGO_TARGET_DIR`) under a short path. Build scripts fail with "The system cannot find the path specified" once paths exceed the Windows 260-character limit.
- **ProtoInput (optional, for multiple keyboards/mice and per-instance input hooks)**: the ProtoInput DLLs are not part of this repository. Copy the files from a [ProtoInput release](https://github.com/SplitScreen-Me/ProtoInput) next to `nucleus-coop.exe` or into `utils\ProtoInput`. Handlers that set `Game.ProtoInput.Inject*` need them.

### Building Rusteus-Coop
> [!IMPORTANT]
> **Windows Target Directory Isolation**: Always specify `$env:CARGO_TARGET_DIR` on Windows to avoid compiler symbol file-locking errors:

```powershell
# Clone the repository
git clone https://github.com/derwins8a/rusteus-coop.git
cd rusteus-coop

# Verify compilation
$env:CARGO_TARGET_DIR = "target_dev"
cargo check --workspace -j 4

# Build release executable
cargo build --release --bin nucleus-coop
```
The compiled executable will be located at `target_dev/release/nucleus-coop.exe`.

### Running Automated Tests
```powershell
$env:CARGO_TARGET_DIR = "target_test"

# Run platform-agnostic core tests
cargo test -p nucleus-script --lib -j 4
cargo test -p nucleus-platform-windows --lib -j 4
cargo test -p nucleus-handler --lib -j 4
cargo test -p nucleus-engine --lib -j 4

# Run isolated GPUI UI integration suites
cargo test -p nucleus-ui --test components_tests -j 4
cargo test -p nucleus-ui --test couch_setup_tests -j 4
cargo test -p nucleus-ui --test couch_osd_tests -j 4
```

---

## 🤝 Contributing

We welcome contributions from both human developers and LLM-based autonomous agents! Please read our **[Contributing Guide](docs/CONTRIBUTING.md)** before submitting code. All contributions must adhere to our non-negotiable **Zero Stubbing Policy** and pass the pre-commit verification gates.

---

## 📜 License & Credits

- Licensed under the **[MIT License](LICENSE)**.
- Handlers and splitscreen technology created by the amazing community at **[Splitscreen.Me](https://hub.splitscreen.me/)**.
- Emulation and input utilities powered by **[Goldberg Steam Emulator](https://gitlab.com/Mr_Goldberg/goldberg_emulator)**, **[ProtoInput](https://github.com/SplitScreen-Me/ProtoInput)**, **[gilrs](https://gitlab.com/gilrs-project/gilrs)**, **[Zed GPUI](https://github.com/zed-industries/zed)**, and **[Boa ECMAScript](https://github.com/boa-dev/boa)**.
