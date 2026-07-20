# 📜 Changelog — BIOS Flasher Professional

Semua catatan perubahan penting pada proyek **BIOS Flasher** milik Megapass Sidoarjo akan dicatat di file ini secara berkala.

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
