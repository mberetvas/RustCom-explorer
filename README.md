# RustCOM Explorer

![RustCOM Explorer](rs-com-explorer.png)

> **A generic, lightning-fast TUI for exploring Windows COM/ActiveX objects.**

[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D4?logo=windows&logoColor=white)](https://www.microsoft.com/windows)
[![Rust](https://img.shields.io/badge/built_with-Rust-dca282.svg?logo=rust&logoColor=white)](https://www.rust-lang.org/)

**RustCOM Explorer** is a terminal-based utility designed for system integrators, SCADA engineers, and developers who need to interact with Windows COM (Component Object Model) objects.

Unlike heavy GUI tools like OLEView, this tool runs entirely in the terminal, offers real-time fuzzy search, and allows for safe inspection of method signatures and properties without accidental instantiation.

---

## ✨ Features

- **🚀 High Performance:** Instant startup and low memory footprint compared to traditional GUI inspectors.
- **🔍 Fuzzy Search:** Filter through thousands of registered COM objects in real-time using fuzzy matching algorithms.
- **🛡️ Safe Inspection:** Prioritizes reading Type Libraries (`LoadRegTypeLib`) to inspect objects without instantiation, preventing side effects.
- **📋 Developer Friendly:** Copy method signatures (`void Method(int ID)`) directly to your clipboard for use in C++, C#, or Rust.
- **🧵 Non-Blocking:** Inspection runs on background threads, ensuring the UI never freezes during heavy registry lookups.

## 🚀 Getting Started

### Prerequisites

- **Windows 10/11** (Required for COM API availability)
- **Rust Toolchain** (1.75 or later)

### Installation

**Option 1: Download Pre-built Executable**

Download the latest built executable for Windows from the [Releases](../../releases) section of this repository. Extract and run directly—no build required.

**Option 2: Build from Source**

Clone the repository and build with Cargo:

```bash
git clone https://github.com/Volvo/comm_browser.git
cd comm_browser
cargo build --release
```

The compiled executable will be available in `target/release/comm_browser.exe`.

> **Note:** Some COM objects require Administrator privileges to inspect. If you encounter permission errors, try running your terminal as Administrator.

## 📖 Usage Guide

The application interface is divided into two panes: the **Object List** (left) and **Details/Inspection** (right).

### Navigation & Browsing

COM objects are organized by category based on their ProgID prefix (e.g., "Excel", "Word", "MSXML"). Use arrow keys to browse categories and individual objects.

### Keyboard Shortcuts

| Key | Action |
| :--- | :--- |
| **Navigation** | |
| `↑` / `↓` | Scroll through categories, objects, or members list |
| `Enter` | **Expand/Collapse** category OR **Inspect** selected object members |
| `Esc` | Go back to browsing / Clear search query |
| `Ctrl+C` | Quit the application |
| **Search** | |
| `a-z` | Type to filter objects (by Name, CLSID, or Description) |
| `Backspace` | Delete character from filter |
| **Inspection Mode** | |
| `c` | Copy selected member signature to clipboard |
| `Shift+C` | Copy **all** members of the object to clipboard |

### View Structure

**Left Pane (Object List):**
- **Categories** (▼ expanded / ▶ collapsed): Grouped by ProgID prefix
- **Objects**: Indented under their category, showing Name and CLSID

**Right Pane (Details):**
- **Browsing Mode**: Shows metadata about the selected category or object
- **Inspection Mode**: Displays object metadata and a list of members (methods & properties)

### Member Types & Access Modes

Members are displayed with type indicators and access badges:

- **M** : **Method** (Function call)
- **P** : **Property** (Attribute)
  - `[R]`: Read-only
  - `[W]`: Write-only
  - `[RW]`: Read/Write

## 🏗 Architecture

This project uses a modular architecture to separate UI logic from low-level Windows APIs.

- **`scanner.rs`**: Handles the enumeration of `HKEY_CLASSES_ROOT` to find registered ProgIDs and CLSIDs.
- **`com_interop.rs`**: The core unsafe Rust layer. It manages COM initialization (RAII), attempts to load TypeInfos from the registry, and parses cryptic `VARDESC`/`FUNCDESC` structures into human-readable strings.
- **`app.rs`**: Manages the TUI state, event loop, and multithreaded inspection channel. Includes rendering logic with native [Ratatui](https://github.com/ratatui/ratatui) styling.
- **`error_handling.rs`**: Custom error handling and type conversions.
- **`UI Rendering`**: Built with [Ratatui](https://github.com/ratatui/ratatui) using native color themes and styling (no custom theme module).

### Safety Strategy

1. **Registry First:** The tool attempts to load `ITypeLib` directly from the registry using the object's GUID.
2. **Dynamic Fallback:** Only if the registry lookup fails does it attempt `CoCreateInstance` to query `IDispatch` dynamically.
3. **Error Handling:** All COM HRESULT failures are captured and displayed as TUI notifications or error panels, ensuring the app never crashes on a bad object.

## 🤝 Known Limitations

- **Windows Only:** This tool relies strictly on the Windows API (Win32) to interact with the Registry and COM Runtime. It **will not work** on Linux, macOS, or WSL (unless running the Windows `.exe` via interop).
- **Administrator Privileges:** Some COM objects (especially those in `HKEY_LOCAL_MACHINE`) require elevated permissions to inspect. If the list seems incomplete, try running as Administrator.
- **Type Libraries:** The safe inspection features rely on objects having registered Type Libraries. Objects without them must be instantiated to be inspected, which this tool performs as a fallback.

## 📄 License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.




