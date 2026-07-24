# 📜 Changelog — BIOS Flasher Professional

Semua catatan perubahan penting pada proyek **BIOS Flasher** milik Megapass Sidoarjo akan dicatat di file ini secara berkala.

## [2.1.7] - 2026-07-24
### Changed
- **DMI Layout Polish:** Grid layout kartu Device Identity kanan-kiri proporsional, alignment teks kiri-kanan rapi, bounding box icon seragam.

## [2.1.6] - 2026-07-23
### Fixed
- Flash path hardening: unique temp files + cleanup for read/write/verify.
- Write/Verify guards for empty chip/buffer.
- Stricter verify success/fail parsing.
- Multi-chip detect parses all quoted names.
- Chip DB empty fallback object; W25Q64JV voltage corrected to 3.3V.
### Added
- Expanded chip database (~82 entries) + bundle chips.json in release.

## [2.1.5] - 2026-07-23
### Changed
- Rename menu **Open Backup** → **Load File** (lebih jelas: buka file .bin ke buffer, bukan hanya backup).

## [2.1.4] - 2026-07-23
### Changed
- **Compare (Diff) polish (MVP):**
  * Laporan ringkas **IDENTIK / BEDA** + MD5 kedua file
  * Jumlah byte beda + persen + offset pertama
  * Pesan jelas jika buffer kosong (Load BIOS dulu)
  * Pesan jelas jika **size file beda** (MB vs MB)
  * Hex marker `*XX*` tetap (sample max 1000 offset)
  * Fast-path: hash sama -> langsung IDENTIK tanpa scan penuh

## [2.1.3] - 2026-07-23
### Added
- **Pre-flight Gate:** Validasi wajib sebelum Write / Erase / Instant Mode:
  * CH341A USB connected (pulse hijau)
  * Chip sudah Detect
  * Buffer terisi (untuk Write/Instant)
  * Konfirmasi eksplisit jika chip 1.8V (adapter level shifter)
- **Udev rule permanen:** `/etc/udev/rules.d/40-ch341a.rules` (MODE 0666, GROUP plugdev) — flashrom non-sudo.

### Changed
- **Version sync:** `package.json`, `Cargo.toml`, `tauri.conf.json`, desktop entry, About modal → **2.1.3**.
- Write & Instant Mode minta konfirmasi ukuran data sebelum eksekusi.

## [2.1.2] - 2026-07-21
### Added
- **💉 DMI Injector (Identity Merger):** Fitur pemindah otomatis data DMI asli (Serial/Windows Key) dari BIOS rusak ke Clean BIOS dalam 1 klik.
- **🧹 Intel ME Region Cleaner & Detector:** Deteksi otomatis region Intel ME ($FPT) dan fitur pembersihan status inisialisasi (Dirty -> Clean Unconfigured State) untuk memperbaiki penyakit *late display / restart 30 menit*.
- **📊 Side-by-Side Hex Diff Viewer:** Pembanding visual file biner BIOS secara byte-per-byte langsung ditandai dengan highlight bintang merah `*XX*` di Hex Viewer.
- **🚨 BSOD Red Alert Diagnostic Boundary:** Penangkapan error sistem di level hardware/software dengan sajian data error terstruktur (Kode Error, File Source, Line, & Context) untuk menyalin log terformat bagi asisten AI tanpa halusinasi.

### Fixed
- Perbaikan Tauri IPC camelCase mapping pada command `inject_dmi` dan `compare_bios_diff` (pemetaan `data_old`/`data_new` ke `dataOld`/`dataNew` & `data_a`/`data_b` ke `dataA`/`dataB`).
- Penyempurnaan layout vertikal menu sidebar kiri tanpa scrollbar (mengecilkan padding vertikal tombol menu menjadi `py-2` dan memindahkan DMI Injector ke bagian bawah menu).

## [2.1.0] - 2026-07-21
### Added
- **📟 Smart DMI & License Auto-Extractor (Offline):**
  * Auto-extract **Windows OEM License Key** (tabel ACPI MSDM).
  * Auto-extract **Laptop Serial Number (S/N)**.
  * Auto-extract **Dell Service Tag** (khusus laptop Dell, dinonaktifkan otomatis untuk brand lain).
  * Auto-extract **HP Board ID (BID)** (khusus laptop HP, sangat krusial untuk mencegah blackscreen).
- **📋 1-Click Copy Clipboard:** Tombol copy instan di setiap kolom DMI dengan visual feedback checkmark hijau.
- **🔍 Hex Search Tool:** Input pencarian teks ASCII & Hex string langsung di atas Hex Viewer untuk memudahkan navigasi firmware.
- **🎨 Windows Title & Brand:**
  * Penambahan emoji `🔧` di window title bar OS: `🔧 BIOS Flasher - By Megapass Sidoarjo v2.1.0`.
  * Standardisasi nama brand menjadi **`Megapass`** secara konsisten di seluruh aplikasi.
- **🧹 Folder Flattening:** Merapikan struktur folder proyek. Menghapus folder bertumpuk `bios-flasher/` dan menaikkan seluruh file konfigurasi langsung ke tingkat root repository.
- **🚀 Native Production Build:** Kompilasi sukses menjadi native release binary mandiri berukuran ringan (18MB) dengan loading instan.

### Changed
- **🔊 Perbaikan Notifikasi Suara:** Mengubah frekuensi notifikasi suara ganda yang bising menjadi **1x Chime lembut** (Sine wave `587.33Hz`) untuk sukses, dan **1x Alert Bass tumpul** (Triangle wave `180Hz`) untuk gagal.

### Fixed
- Izin plugin Tauri (`dialog` & `fs` permissions) di `capabilities/default.json` agar fitur save dialog backup dan open backup berjalan lancar tanpa error sandboxing.
- Bug compile path Rust target akibat sisa folder nested lama.

## [2.0.0] - 2026-07-19
- Migrasi GUI dari bash YAD script ke custom Python GTK3 UI.
- Penambahan layout visual split 40/60.

## [1.1.0] - 2026-07-19
- Migrasi dari Zenity ke YAD untuk performa alignment kolom.

## [1.0.0] - 2026-07-19
- Rilis perdana menggunakan bash script + Zenity GUI.
