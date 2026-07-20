# CH341A BIOS Flasher Professional Edition - PROJECT PLAN (Tauri)
## Build specification for Claude Opus rebuild (Clean Spec - No Code Blocks)

---

## 1. PROJECT OVERVIEW

- **Purpose:** Professional BIOS flash tool for laptop/PC service technicians.
- **User:** Hizam @ Megapass Intra Solusindo (Sidoarjo, GMT+7).
- **Hardware:** CH341A USB programmer (ID: 1a86:5512).
- **Backend:** flashrom 1.6.0 (command line tool).
- **Language:** Indonesian UI (menus/labels), English technical terms.
- **Target Framework:** **Tauri v1/v2** (Rust backend + React/Tailwind frontend).

---

## 2. TECHNICAL STACK & DEPENDENCIES

**Backend:**
- **Rust:** Manages subprocess calls to `flashrom` and filesystem operations.
- **Tauri Core:** Manages communication (IPC) between Rust and Webview.

**Frontend:**
- **React.js:** Component-based UI rendering.
- **Tailwind CSS + daisyUI (Dark Theme):** Modern, responsive UI widgets out-of-the-box.

**System Dependencies (Technician's PC):**
- Node.js (npm) & Rust toolchain (cargo).
- Build libraries: `build-essential`, `libssl-dev`, `libgtk-3-dev`, `libwebkit2gtk-4.0-dev`.
- Backend tool: `flashrom`.

---

## 3. UI/UX SPECIFICATIONS (ABSTRACT)

### Window Configuration
- Size: 900x700px, resizable, centered.
- Theme: Dark (Charcoal/Neutral Dark palette).

### Layout (Split Pane 40/60)
Use `QSplitter` style layout via CSS/Flexbox:
- **Header:** Full-width status bar.
  - Left: Chip identification label (badge colored: red if none, green if detected).
  - Right: Buffer status (size in KB, loaded file path if any).
- **Left Pane (40% - Operations Menu):**
  - Vertically stacked menu items with numerical prefixes and emoji icons.
  - Active selection highlighted in accent color (e.g., pink/salmon or neon blue).
  - Progress bar container visible at the bottom of this pane only during active processes (0-100% track).
- **Right Pane (60% - Hex Viewer):**
  - Read-only scrollable console container with monospace font.
  - Formatted text display.
  - Log/Status label below the hex viewer showing command feedback or raw logs.
- **Bottom:** Single prominent Apply button (`btn-primary` style, centered, wide).

---

## 4. OPERATION FLOWS (7 total)

### 1. 🔍 Detect Chip
- **Rust Action:** Run `sudo flashrom -p ch341a_spi`. Parse stdout for detected chip name.
- **Database Lookup:** Check the detected chip in `chips.json`.
- **UI Action:** Show modal dialog listing: Manufacturer, Model, Capacity (MB), Voltage (1.8V/3.3V), Package Type. Update header chip info.

### 2. 📖 Read
- **Rust Action:** Spawns background thread. Run `sudo flashrom -p ch341a_spi -c [CHIP_NAME] -r /tmp/read_buffer.bin`. Parse stderr output for percentage strings.
- **Bridge:** Emit progress (0-100) and status log lines to frontend.
- **UI Action:** Read binary file into memory buffer. Update hex viewer. Show notification on finish. Check if buffer has content or is empty (all `0xFF`).

### 3. 💾 Backup
- **UI Action:** Open native OS save file dialog. Filter: `*.bin`.
- **Rust Action:** Write memory buffer to selected path. Calculate MD5 checksum.
- **UI Action:** Show success dialog with file path, size (KB), and MD5 hash.

### 4. 📂 Open Backup
- **UI Action:** Open native OS file chooser dialog. Filter: `*.bin`.
- **Rust Action:** Read selected file into byte array.
- **UI Action:** Update memory buffer. Render hex preview. Update header buffer status.

### 5. ✍️ Write
- **UI Action:** Show warning confirmation dialog. Warning: Overwriting chip data.
- **Rust Action:** Write memory buffer to `/tmp/write_buffer.bin`, spawn thread, run `sudo flashrom -p ch341a_spi -c [CHIP_NAME] -w /tmp/write_buffer.bin`. Parse output for progress.
- **Bridge:** Emit progress updates to frontend progress bar.
- **UI Action:** Show success/failure message.

### 6. ✅ Verify
- **Rust Action:** Write memory buffer to `/tmp/verify_buffer.bin`, spawn thread, run `sudo flashrom -p ch341a_spi -c [CHIP_NAME] -v /tmp/verify_buffer.bin`. Parse output.
- **UI Action:** Display verification result (match percentage or byte mismatch details).

### 7. 🗑️ Erase (Destructive)
- **UI Action - Double Confirmation:**
  - Dialog 1: Warn user about complete data loss. Ask to proceed.
  - Dialog 2: Hard confirmation statement (default button: Cancel/No).
- **Rust Action:** Spawn thread, run `sudo flashrom -p ch341a_spi -c [CHIP_NAME] -E`.
- **UI Action:** Clear memory buffer (set all bytes to `0xFF`). Update hex viewer to empty display.

---

## 5. RUST-REACT BRIDGE CONTRACT (API Specification)

To prevent code conflicts, the interface between frontend and backend is defined below.

### Rust Commands to Implement:
1. `run_flashrom_operation(args: Vec<String>, op_type: String)` -> async command, spawns thread, runs command, parses output.
2. `read_binary_file(path: String)` -> returns raw byte array (`Vec<u8>`).
3. `write_binary_file(path: String, data: Vec<u8>)` -> writes byte array to disk.

### Tauri Events to Emit from Rust:
- `operation-progress` (Payload: `i32` 0-100) -> updates progress bar.
- `operation-log` (Payload: `String`) -> updates status log label.

### Frontend Dialog APIs to Use:
- `@tauri-apps/api/dialog/open` for file choosing.
- `@tauri-apps/api/dialog/save` for file saving.

---

## 6. HEX VIEWER FORMATTING LOGIC

The React frontend must format the binary buffer (first 64KB) for display:
1. Header line: `Offset   | 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F | ASCII`
2. Divider line.
3. Content lines (16 bytes per row):
   - Offset: 8-digit hexadecimal (e.g. `00000000`).
   - Hex values: 16 pairs of uppercase 2-digit hex separated by space (e.g. `FF FF ... FF`).
   - ASCII column: Print character if printable (ASCII code 32 to 126), print dot (`.`) if non-printable.
   - Separator between columns: pipe character (`|`).

---

## 7. CHIP DATABASE (chips.json)

- Location: `~/proyek/CH341A-programer/chips.json`
- Schema: Key is the chip identifier string. Values: `manufacturer`, `size_kb`, `voltage`, `package`.
- Common chips: Winbond (W25Q32/64/128/256), Macronix (MX25L series), GigaDevice (GD25Q series), EON, AMIC, SST.

---

## 8. DEPLOYMENT & CONSTRAINTS

### 1. Hardware Permissions (udev Rules)
To run `flashrom` without `sudo` password prompts:
- Create `/etc/udev/rules.d/99-ch341a.rules` containing:
  `SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="5512", MODE="0666"`
- Reload rules: `sudo udevadm control --reload-rules && sudo udevadm trigger`.
- This allows Tauri to call `flashrom` directly without root elevation wrapper.

### 2. Standalone Binary Compile
- Build command: `npm run tauri build`.
- Output: Standard `.AppImage` and compiled binary executable.

---

## 9. SUCCESS CRITERIA FOR OPUS

- [ ] Standalone compiled executable size is minimal (~10-15MB).
- [ ] Interface aligns with the 40/60 split layout.
- [ ] 7 menu options map correctly to frontend actions.
- [ ] Threading is implemented so that long operations do not lock up the GUI.
- [ ] Progress bar accurately tracks percentage from backend.
- [ ] Hex viewer displays binary contents in readable offset/hex/ASCII columns.
- [ ] Erase action prompts double confirmation dialog.
- [ ] Hardware access permissions bypass standard password prompt cleanly.

---

**END OF SPECIFICATION**
**Version: 2.1 (Tauri-Clean) | Updated: 2026-07-20**
