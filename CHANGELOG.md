# 📜 Changelog — BIOS Flasher Professional

Semua catatan perubahan penting pada proyek **BIOS Flasher** milik Megapass Sidoarjo akan dicatat di file ini secara berkala.

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
