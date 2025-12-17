# Copilot Instructions for `comm_browser`

## 1. Project Context & Principles
- **Core Domain:** A TUI tool for inspecting Windows COM/ActiveX objects (ProgIDs, CLSIDs, TypeLibs) without instantiation.
- **Platform Strategy:** Code is Windows-native. Non-Windows builds are stubs only.
- **Safety Philosophy:**
  - **Static over Dynamic:** Always prefer reading static registry data (`LoadRegTypeLib`) over instantiating objects (`CoCreateInstance`) to prevent side effects.
  - **No Panics:** The tool must never crash on malformed registry data or permission errors. Use `Result` and UI notifications.
  - **RAII:** All FFI/unsafe resources (COM initialization, raw pointers) must be wrapped in structs that handle cleanup in `Drop`.

## 2. Architectural Boundaries
The application follows a strict separation of concerns that must be preserved:

### A. The UI Layer (Presentation)
- **Role:** Handles rendering, input processing, and state management.
- **Key Pattern:** **Immediate Mode TUI**. The UI is a function of the central state (`App`).
- **Concurrency:** The main thread **must never block** on IO or Registry calls.
  - *Pattern:* Use `std::sync::mpsc` (or similar channels) to receive data from background worker threads.
  - *State:* The `App` struct checks the receiver channel once per tick to update the UI.

### B. The Interop Layer (Infrastructure)
- **Role:** Wraps `unsafe` OS APIs (Windows COM/OLE) into safe Rust abstractions.
- **Isolation:** Keep `unsafe` code restricted to specific interop modules. Do not leak raw Windows types (like `BSTR` or `HRESULT`) into the UI layer.
- **Data Conversion:** Parse cryptic OS structures (`FUNCDESC`, `VARDESC`) into human-readable Rust structs (`TypeDetails`, `Member`) immediately.

### C. The Data/Scanner Layer
- **Role:** Enumerates system resources (Registry keys).
- **Testability Pattern:** **Trait-based Abstraction**.
  - Logic accessing the system (e.g., Registry) must rely on traits (e.g., `RegistryReader`, `RegistryKey`).
  - *Reasoning:* This allows unit tests to run on non-Windows machines and without admin privileges by using Mock implementations.
  - **Rule:** Never write logic that directly instantiates a concrete system accessor (like `winreg::RegKey`) inside business logic; accept it as a dependency or trait object.

## 3. Developer Workflows & Conventions

### Testing
- **Unit Tests:** Focus on the Data Layer using Mocks. *Do not* write tests that require a real running Windows registry unless marked `#[cfg(windows)]` and strictly necessary.
- **TDD:** When adding new scanner features, extend the Mock struct first to represent the registry structure you expect to find.

### Error Handling
- **User-Facing:** COM errors are information, not failures. Display them in the UI (e.g., "Permission Denied").
- **Internal:** Use the `anyhow` or custom `InspectError` pattern. Map numeric HRESULTs to readable messages before bubbling up.

### Coding Style
- **Modules:** Keep `main.rs` minimal (setup only). Logic belongs in `lib.rs` modules.
- **Clarity:** When using FFI, explicitly document the `unsafe` invariants.

## 4. Key Libraries & ecosystem
(Check `Cargo.toml` for versions, but understand the roles)
- **UI:** `ratatui` (Rendering), `crossterm` (Input).
- **System:** `windows` (Official bindings), `winreg` (Registry).
- **Utilities:** `fuzzy-matcher` (Search), `arboard` (Clipboard).

## 5. Discoverability
- **Finding State:** Look for the central struct (usually `App`) that implements the run loop.
- **Finding System Calls:** Look for modules implementing `RegistryReader` or wrapping `CoCreateInstance`.