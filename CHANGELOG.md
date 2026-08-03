# 📜 Changelog — BIOS Flasher Professional

Semua catatan perubahan penting pada proyek **BIOS Flasher** milik Megapass Sidoarjo akan dicatat di file ini secara berkala.

## [2.2.1] - 2026-08-03
### Fixed — Deep Audit (8 logic bug, 2 di antaranya berisiko merusak hardware)
- **[KRITIS] DMI Injector bisa brick BIOS:** Signature pencarian `"DMI"` cuma 3 byte, jadi cocok dengan string sampah biasa di BIOS (contoh nyata: `"DMI EDIT TOOL"`). Offset hasil salah itu dipakai untuk menyalin 64KB secara buta, sehingga region acak (bisa NVRAM/ME/descriptor) ketimpa. Sekarang hanya menerima anchor yang divalidasi strukturnya: MSDM (cek length field `0x55..0x1000`), SMBIOS `_SM_` (cek length ≥ `0x18`), dan `_DMI_` 5-byte. Kalau tidak yakin → tolak, bukan hajar.
- **[KRITIS] Chip 1.8V berisiko hangus:** `Read`, `Verify`, dan `Blank Check` melewati pre-flight check padahal ketiganya menyalurkan tegangan ke chip. Chip 1.8V bisa terbakar kena 3.3V tanpa level shifter. Ketiganya sekarang wajib lewat `runPreflight()`.
- **Windows Key tidak pernah terbaca:** Segment MSDM adalah ACPI table biner (ada byte checksum >0x7F), sehingga `String::from_utf8` selalu gagal dan blok pembacaan key di-skip diam-diam — hasilnya `"Not Found"` padahal key ada di buffer. Diganti `from_utf8_lossy`. Berlaku untuk DMI Extractor dan DMI Injector.
- **Edit DMI merusak data tetangga:** `overwrite_dmi_value` menulis tanpa cek panjang, sehingga value yang lebih panjang dari field aslinya menimpa struktur BIOS setelahnya. Sekarang ditolak dengan pesan jelas berapa byte yang muat.
- **Progress bar mati:** Progress hanya di-parse dari stderr, padahal flashrom menulis output normal ke stdout. Akibatnya bar diam di 0% lalu lompat 100% — operator tidak tahu proses jalan atau nyantol saat flash 8MB. Kedua stream sekarang di-stream & di-parse paralel.
- **ME Cleaner mode Python selalu gagal di user:** `me_cleaner.py` tidak ikut ter-bundle, dan path fallback-nya hardcode ke folder PC developer. Sudah ditambahkan ke `resources` bundle.
- **UI freeze di Diff Mode:** `Array.includes()` dipanggil di dalam loop 32KB × 1000 offset (~32 juta operasi tiap render). Diganti `Set.has()` — terukur **54x lebih cepat**, hasil identik.
- **Erase tanpa guard:** `erase_bios` bisa menjalankan `flashrom -c ""` saat chip belum terdeteksi. Guard ditambahkan menyamakan dengan command lain.

### Changed
- **Unit test 8 → 14.** Test lama `test_overwrite_longer` ternyata *mengesahkan* bug penimpaan data tetangga, makanya bug itu lolos 8/8 selama berbulan-bulan. Diganti test regresi yang memisahkan kasus "pas muat" vs "overflow", plus test regresi untuk DMI signature sampah dan pembacaan key dari MSDM biner.

### Verifikasi
`cargo clippy` clean · `cargo test` 14/14 pass · `vite build` OK · `tauri build` 3 bundle OK · `me_cleaner.py` terkonfirmasi ada di dalam paket `.deb` hasil build.

> ⚠️ Belum diuji dengan hardware CH341A fisik (programmer tidak tercolok saat audit). Disarankan tes Read → Backup → Compare di chip bekas sebelum dipakai ke BIOS customer.

## [2.2.0] - 2026-07-24
### Added
- **Blank Check Feature:** Cek apakah isi chip 100% kosong (0xFF) secara biner di backend Rust untuk memvalidasi proteksi write-protect / kegagalan erase.

## [2.1.9] - 2026-07-24
### Fixed
- **DMI Card Alignment:** Unified grid system forcing row height (h-7) parity between columns. No more independent column height drift.

## [2.1.8] - 2026-07-24
### Fixed
- **Right Pane Layout:** Lock layout atas (Search, DMI Card, ME Panel) agar  sehingga tidak tertekan (squeezed) atau tidak proporsional saat data BIOS termuat ke buffer.

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
