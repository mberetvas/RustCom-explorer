# Copilot Instructions for `comm_browser` (RustCOM Explorer)

## Project Overview

- **Purpose:** TUI tool for Windows to browse, filter, and inspect registered COM/ActiveX objects (e.g., WinCC providers) and their type information, optimized for remote/CLI use.
- **Key Features:** Registry scanning, fuzzy search/filter, safe type inspection (without instantiating objects), and clipboard integration.

## Architecture & Key Modules

- **Data Layer:**  
  - `scanner.rs`: Scans `HKEY_CLASSES_ROOT` for COM objects (`ProgID`, `CLSID`, `Description`).  
  - `com_interop.rs` (planned): Handles COM initialization and safe type info retrieval (`ITypeInfo`), prioritizing `LoadRegTypeLib` for safety.
- **View Layer:**  
  - TUI built with `ratatui` and `crossterm`.  
  - Split layout: Left pane (object list), right pane (member details), bottom bar (help/status).
- **State Management:**  
  - Central `App` struct holds object list, search query, selected object, and app mode (`Scanning`, `Browsing`, `Inspecting`).

## Developer Workflows

- **Build:**  
  - Standard: `cargo build`  
  - Run: `cargo run`
- **Linting & Quality:**  
  - **Always run `cargo clippy` before finalizing a task** to catch common mistakes and ensure code quality.
- **Testing:**  
  - **Use Test-Driven Development (TDD)** unless explicitly specified otherwise. Write tests before implementation to define expected behavior.
  - Currently no formal test suite; focus on manual verification (see Docs/prd.md, tasks_and_lists.md) for acceptance testing.
- **Debugging:**  
  - Use `println!` or logging for diagnostics.  
  - Handle errors gracefully—never crash on COM/registry failures; display errors in the TUI.

## Patterns & Conventions

- **Registry/COM Access:**  
  - Use `winreg` for registry, `windows` crate for COM APIs.  
  - Always prefer type library inspection over direct instantiation for safety.
- **Fuzzy Search:**  
  - Use `fuzzy-matcher` for real-time filtering of the object list.
- **Clipboard:**  
  - Use `arboard` to copy function signatures on keypress (`c`).
- **Error Handling:**  
  - All COM/registry errors must be caught and shown in the UI, not as panics.
- **UI Layout:**  
  - Follow the split-pane pattern described in Docs/tasks_and_lists.md.

## Integration Points

- **External Crates:**  
  - `ratatui`, `crossterm`, `windows`, `winreg`, `fuzzy-matcher`, `arboard`, `anyhow`.
- **Windows Only:**  
  - All COM/registry logic assumes a Windows environment.

## References

- See `Docs/prd.md` for product goals and user stories.
- See `Docs/tasks_and_lists.md` for implementation phases and module responsibilities.
- See `Cargo.toml` for dependency versions and features.
