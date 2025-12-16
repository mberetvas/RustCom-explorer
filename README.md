# 🔍 RustCOM Explorer

> A blazingly fast TUI for browsing, filtering, and inspecting Windows COM/ActiveX objects—designed for system integrators and SCADA engineers.

[![Rust](https://img.shields.io/badge/rust-%23CE422B?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Windows](https://img.shields.io/badge/platform-Windows-0078D4?style=flat&logo=windows&logoColor=white)](https://www.microsoft.com/windows)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?style=flat)](LICENSE)

---

## 🚀 Quick Start

### Prerequisites
- **Windows 10+** (COM APIs are Windows-only)
- **Rust 1.70+** ([Install Rust](https://rustup.rs/))

### Installation & Run

```bash
# Clone and navigate to the project
git clone <repository>
cd comm_browser

# Build and run
cargo run
```

The app will scan your system's COM registry and launch an interactive TUI. Use arrow keys to navigate and press `Enter` to inspect details.

---

## 🎯 What is RustCOM Explorer?

RustCOM Explorer (RCE) is a lightweight **Text User Interface (TUI)** that lets you:

- 🔎 **Browse** all registered COM/ActiveX objects on your Windows machine
- ⚡ **Filter** thousands of objects in real-time with fuzzy search
- 🔬 **Inspect** methods and properties without instantiating objects (safe-by-default)
- 📋 **Copy** function signatures to your clipboard
- 🖥️ **Work remotely** over SSH/PowerShell without heavy GUI overhead

Perfect for exploring WinCC providers, Excel COM interfaces, and custom ActiveX components.

---

## 💡 Why RustCOM Explorer?

| Tool | GUI | Performance | Remote-Friendly | Safe Inspection |
|------|-----|-------------|-----------------|-----------------|
| **OLEView** | ✅ | ⚠️ Slow | ❌ GUI-only | ❌ May instantiate |
| **Excel VBA** | ✅ | ⚠️ Slow | ❌ Requires Office | ⚠️ Risky |
| **RustCOM Explorer** | ✅ TUI | ✅ Fast | ✅ SSH/CLI | ✅ Type-lib first |

---

## 🎮 Usage Guide

### Navigation

```
↑ / ↓       Scroll through COM objects
Enter       Inspect selected object (view methods & properties)
Esc         Exit inspection mode or clear search filter
Ctrl+C      Quit the application
```

### Searching & Filtering

Start typing to filter objects in real-time:

```
Type: wincc     → Shows only objects matching "wincc"
Type: alarm     → Shows only objects matching "alarm"
Backspace       → Remove last character from filter
Esc             → Clear entire filter
```

### Inspecting Objects

1. **Navigate** to a COM object (e.g., `WinCC-Runtime-Project`)
2. **Press Enter** to inspect its type information
3. **View members:**
   - 🔵 **P** = Property (Read/Write/ReadWrite)
   - 🟦 **M** = Method

Example inspection output:
```
Name: WinCC.CCAlarmList.1
Description: WinCC Alarm List Object

Members:
P [RO] ServerName: String
P [RW] Filter: String
P [RO] Count: Long
M GetAlarm(ID: Long) -> IDispatch
M AckAlarm(ID: Long) -> Void
M Quit() -> Void
```

---

## 🏗️ Architecture

### Module Overview

- **`scanner.rs`** – Registry scanning engine
  - Scans `HKEY_CLASSES_ROOT` for COM objects
  - Extracts ProgID, CLSID, and descriptions

- **`com_interop.rs`** – COM interaction layer
  - Initializes COM library safely (RAII guard)
  - Loads type libraries via `LoadRegTypeLib`
  - Parses `ITypeInfo` into human-readable signatures
  - Fallback to dynamic instantiation if type library unavailable

- **`app.rs`** – Application state & TUI logic
  - Central state management (`App` struct)
  - Event handling (keyboard input, mode transitions)
  - UI rendering (two-pane split layout)

- **`error_handling.rs`** – Error management
  - Unified error type (`Result<T>`)
  - Graceful error display in UI

### Data Flow

```
┌─────────────────────────────────────────┐
│         Windows COM Registry            │
│   HKEY_CLASSES_ROOT + TypeLibraries     │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│       scanner.rs (Registry Scan)        │
│   Extracts: ProgID, CLSID, Description  │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│          app.rs (State & Events)        │
│   Filtering, Selection, Mode Management │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│    com_interop.rs (Type Inspection)     │
│   Parses ITypeInfo → Readable Signatures│
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│    ratatui (TUI Rendering & Display)    │
│         Two-Pane Split Layout           │
└─────────────────────────────────────────┘
```

---

## 🛠️ Development

### Project Structure

```
comm_browser/
├── src/
│   ├── main.rs              # Entry point & TUI setup
│   ├── lib.rs               # Module exports
│   ├── app.rs               # State & event handling
│   ├── scanner.rs           # Registry scanning
│   ├── com_interop.rs       # COM initialization & type parsing
│   └── error_handling.rs    # Error types
├── Docs/
│   ├── prd.md               # Product requirements
│   ├── tasks_and_lists.md   # Development roadmap
│   └── crates_and_setup.md  # Dependency guide
├── Cargo.toml               # Project manifest
└── README.md                # This file
```

### Building

```bash
# Development build
cargo build

# Optimized release build
cargo build --release

# Run with logging
RUST_LOG=debug cargo run
```

### Code Quality

Always run clippy before finalizing changes:

```bash
cargo clippy
```

### Testing

Unit tests are included for critical logic:

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

---

## 📚 Key Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | Terminal UI rendering |
| `crossterm` | Terminal input/output handling |
| `windows` | Windows API bindings (COM, Registry) |
| `winreg` | High-level registry access |
| `anyhow` | Error handling & context |
| `fuzzy-matcher` | Real-time search filtering |
| `arboard` | Clipboard integration |

---

## ⚙️ Technical Highlights

### Safety-First Design

RustCOM Explorer prioritizes **safety** over convenience:

1. **Type Library First** – Uses `LoadRegTypeLib` to inspect objects without instantiation
2. **Graceful Fallback** – Only instantiates objects if type library is unavailable
3. **No Panics** – All errors caught and displayed in the UI; app never crashes on COM failures
4. **RAII Guards** – COM initialization uses RAII pattern for automatic cleanup

### Type Parsing

Converts cryptic COM variant types into readable strings:

```rust
VT_BSTR         → String
VT_I4           → Long
VT_DISPATCH     → IDispatch
VT_BOOL         → Boolean
VT_SAFEARRAY    → SafeArray
```

### Real-Time Filtering

Fuzzy search updates the object list instantly as you type, with substring matching on:
- Program ID (ProgID)
- Class ID (CLSID)
- Description

---

## 🐛 Known Limitations

- **Windows Only** – Requires Windows 10+ for full COM API support
- **Registry Access** – Some corporate environments may restrict registry read access
- **Type Library Dependencies** – Objects without registered type libraries require dynamic instantiation
- **No Clipboard on WSL** – Limited clipboard support in WSL1; WSL2 supported

---

## 📖 Examples

### Finding WinCC Objects

```
1. Run: cargo run
2. Type: "wincc"
3. Browse filtered results
4. Press Enter on "WinCC-Runtime-Project"
5. View all available methods and properties
```

### Inspecting Custom COM Objects

```
1. Navigate to your custom ProgID
2. Press Enter to load type information
3. Copy function signatures to clipboard for use in scripts
4. Use parameter types to write correct calls
```

---

## 🤝 Contributing

Contributions are welcome! Please:

1. Follow the [Copilot Instructions](.github/copilot-instructions.md)
2. Run `cargo clippy` before submitting
3. Add tests for new functionality
4. Keep code clean and well-documented

See [tasks_and_lists.md](Docs/tasks_and_lists.md) for the development roadmap.

---

## 📄 License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

---

## 📞 Support

For issues, feature requests, or questions:

- 📌 Check [prd.md](Docs/prd.md) for feature overview
- 🔧 Review [crates_and_setup.md](Docs/crates_and_setup.md) for setup details
- 💻 Examine [tasks_and_lists.md](Docs/tasks_and_lists.md) for development context

---

**Built with ❤️ in Rust** — making COM inspection fast, safe, and accessible.
