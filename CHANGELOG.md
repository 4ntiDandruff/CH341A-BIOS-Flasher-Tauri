# 📜 Changelog — BIOS Flasher Professional

Semua catatan perubahan penting pada proyek **BIOS Flasher** milik Megapass Sidoarjo akan dicatat di file ini secara berkala.

## [2.2.5] - 2026-08-05
### Fixed — Audit ronde 4 lanjutan (1 MEDIUM + 3 LOW hardening)
- **[MEDIUM] ME Cleaner mode "flag" palsu dibuang:** Mode default `flag` (`clean_me_region`, `lib.rs`) menulis `output_data[$FPT+16] = 0xFF`. Offset +0x10 adalah field UMASize di header $FPT — BUKAN toggle status ME — dan checksum header (+0x0B) tidak pernah dihitung ulang. Hasilnya UMASize korup + checksum basi, tapi UI melaporkan "✅ cleaned successfully" (`analyze_me_region` malah hardcode status "Dirty" sehingga verifikasi tak pernah bisa lulus). Operator flash → ME bisa gagal init, penyakit late-display/restart 30 menit tetap/tambah parah, dengan laporan sukses palsu. Diperbaiki: mode `flag` dihapus total (backend menolak dengan `ERR_ME_MODE_UNSUPPORTED_0x307`); hanya jalur `me_cleaner.py` (teruji, hitung checksum benar) yang dipakai. Radio "Reset Flag Cepat" dihapus dari UI.
- **[LOW] Cek ukuran image vs chip sebelum Write/Instant Mode:** `runPreflight` dulu tak pernah membandingkan `buffer.length` dengan `chipInfo.size_kb*1024`. Instant Mode meng-erase chip DULU lalu flashrom menolak write kalau ukuran beda → chip terlanjur kosong. Sekarang preflight (saat `needBuffer`) memperingatkan operator kalau ukuran tidak cocok sebelum menyentuh hardware.
- **[LOW] Izin `fs:*` Tauri yang tidak terpakai dihapus:** `capabilities/default.json` dulu memberi webview `fs:allow-write/remove/rename/mkdir` + `fs:scope-home/temp/desktop` (blast radius: tulis/hapus seisi home). Frontend tidak pernah meng-import `@tauri-apps/plugin-fs` — semua IO lewat command Rust custom. Blok `fs:*` + registrasi `tauri_plugin_fs::init()` + dependency `tauri-plugin-fs` dihapus. Pure hardening, tanpa kehilangan fungsi.
- **[LOW] Temp file ME cleaner bocor saat write gagal:** Di `clean_me_region` mode python, kalau `fs::write(&temp_in)` error, fungsi dulu langsung return tanpa menghapus `temp_in` → file BIOS parsial (berisi lisensi/serial) nyangkut di `/tmp`. Ditambah `remove_file` di jalur error.

### Changed
- **Versi 2.2.4 → 2.2.5.** ME Clean modal sekarang single-method (me_cleaner.py). Dependency `tauri-plugin-fs` dilepas dari `Cargo.toml`.

### Catatan
Ronde ini menutup sisa temuan audit ronde 4 (MEDIUM + LOW). CH341A programmer terpasang fisik saat build — smoke test hardware (USB detect + chip detect) dijalankan, lihat bagian Verifikasi.

## [2.2.4] - 2026-08-05
### Fixed — Deep multi-agent audit ronde 4 (3 bug bahaya yang lolos audit v2.2.3)
- **[BAHAYA CHIP / HIGH] Gerbang 1.8V fail-open untuk chip di luar database:** Proteksi 1.8V (`App.jsx` preflight + badge) hanya nyala kalau `chipInfo.voltage === "1.8V"` — perbandingan string persis. `get_chip_info` (`lib.rs`) mengembalikan `voltage:"Unknown"` untuk setiap chip yang dikenali flashrom tapi tidak ada di `chips.json` (~90 entri kurasi vs ribuan chip flashrom). Chip 1.8V yang tak ada di DB → `"Unknown"` → dialog konfirmasi adapter TIDAK muncul, badge peringatan TIDAK render, operator tidak dapat indikasi apa pun → CH341A menyalurkan ~3.3V ke die 1.8V → chip hangus permanen. Diperbaiki **fail-safe**: kecuali voltase POSITIF diketahui `"3.3V"`/`"5V"`, preflight memaksa konfirmasi adapter; ditambah badge kuning "VOLTASE TAK DIKENAL - CEK 1.8V!".
- **[BAHAYA BRICK / HIGH] `find_field_offset` menulis ke kemunculan yang salah:** Fungsi ini mencari offset field DMI dengan scan seluruh buffer dan mengambil **kecocokan byte pertama**. Field pendek (Board ID 4-6 char, Dell service tag 7 char) sangat mungkin punya byte kembar di region lain, sehingga offset menunjuk lokasi non-SMBIOS. Guard `overwrite_dmi_value` (cek byte==nilai lama) TIDAK menolong — justru selalu cocok di offset salah itu (itu sebabnya kepilih). Edit field → tulisan + null-pad mendarat di region acak → BIOS korup/brick, sementara field asli tak berubah (kegagalan ganda diam-diam). Diperbaiki: offset hanya sah kalau nilai muncul **tepat 1x** di buffer; ambigu (>1x) atau tidak ada → 0 → UI memblokir edit (suruh cek manual) daripada nekat menulis ke lokasi tebakan.
- **[BAHAYA DATA / HIGH] Erase membuang satu-satunya copy BIOS di RAM:** `handleErase` sukses → langsung `setBuffer(null)`, tanpa cek backup. Buffer hasil Read chip mati sering satu-satunya copy BIOS customer (lisensi Windows/MSDM, DMI, serial). Skenario: teknisi baca chip untuk menangkap lisensi, belum save .bin, klik Erase untuk reflash → chip jadi 0xFF DAN copy RAM hilang bersamaan → lisensi hilang permanen (Write berikutnya malah keblok "Buffer kosong"). Diperbaiki: buffer TIDAK di-null saat erase (dipertahankan sebagai sumber re-write), dan sebelum Erase/Instant Mode aplikasi memaksa backup .bin dulu (`ensureBackupBeforeDestroy`) kalau buffer dari Read belum tersimpan.

### Changed
- **Unit test 18 → 20.** Ditambah `test_find_offset_ambigu_ditolak` (nilai muncul 2x → offset 0, cegah brick) dan `test_find_offset_unik_akurat` (nilai unik → offset benar). `handleBackup` sekarang mengembalikan path tersimpan supaya gate backup bisa cek sinkron (state React async).

### Catatan
Audit ronde 4 dijalankan sebagai deep multi-agent audit: 5 agent pencari (flash-safety, dmi-offset, error-data-loss, me-cleaner, tauri-security) + agent skeptis independen yang membantah tiap temuan. 11 temuan REAL terkonfirmasi (setelah dedupe = 8 bug: 3 HIGH di atas, 1 MEDIUM ME-cleaner "flag" mode, 4 LOW hardening). Ronde ini menutup 3 HIGH; sisanya (ME-cleaner flag mode + hardening /tmp & Tauri capability) menyusul.

## [2.2.3] - 2026-08-05
### Fixed — Audit ronde 3 (bug yang lolos dari audit v2.2.2)
- **[KRITIS] Edit DMI bisa merusak tabel BIOS tetangga (BUG-11):** `overwrite_dmi_value` menentukan panjang slot field dengan cara **nge-scan byte printable maju** dari offset. Kalau sebuah field (mis. Windows Key 29 char) menempel langsung ke signature tabel ACPI berikutnya (`SSDT`/`SSDT`/dll) **tanpa null atau spasi pemisah** — umum di dump BIOS mentah — scan "makan" byte tetangga dan menganggapnya bagian dari field. Saat padding, byte tetangga itu ketimpa `0x00` → struktur BIOS sebelahnya korup. Ini kelas bug yang sama dengan BUG-3, tapi lewat jalur yang belum ditutup (guard BUG-3 hanya membandingkan panjang value baru vs hasil scan yang sudah salah). Sekarang panjang slot diambil dari **nilai lama yang sudah tampil di UI** (`old_value.len()`), bukan hasil tebakan scan — batas field pasti akurat dan tetangga dijamin utuh. Terbukti via test regresi `test_overwrite_bug11_field_nempel_tabel_acpi`.
- **Penulisan di offset basi ditolak (hardening):** Edit DMI kini memverifikasi byte di offset masih sama persis dengan nilai lama sebelum menulis. Kalau buffer sudah berubah sejak extraction (offset basi), tulisan ditolak dengan pesan jelas — bukan mendarat di lokasi salah dan merusak data.

### Changed
- **Unit test 16 → 18.** Ditambah `test_overwrite_bug11_field_nempel_tabel_acpi` (mengunci BUG-11: field menempel tabel ACPI, tetangga wajib utuh) dan `test_overwrite_offset_basi_ditolak` (menolak tulis di offset yang tidak lagi cocok). Signature `overwrite_dmi_value` sekarang menerima parameter `old_value`.
- **Padding field DMI selalu `0x00`.** Dulu menebak antara spasi (`0x20`) dan null berdasarkan byte di sekitarnya. Sekarang selalu null terminator (standar string SMBIOS/DMI), lebih sederhana dan tidak bergantung pada scan.

### Catatan
Audit ronde 3 menemukan bahwa **fix BUG-3 di v2.2.1 tidak menutup seluruh jalur**. BUG-3 memasang guard "tolak kalau value baru lebih panjang dari field", tapi *panjang field* itu sendiri dihitung dengan scan yang bisa salah ketika tidak ada pemisah antar-field. Test regresi BUG-3 kebetulan selalu punya byte non-printable (`0x00`/`0xDEADBEEF`) tepat setelah field, jadi scan berhenti di tempat yang benar dan bug baru ini tidak ketahuan. Akar sebenarnya: panjang field seharusnya tidak pernah ditebak — sumber kebenarannya adalah nilai lama yang sudah diekstrak dan ditampilkan.

## [2.2.2] - 2026-08-03
### Fixed — Audit ronde 2 (bug yang lolos dari audit v2.2.1)
- **[KRITIS] Offset Windows Key meleset → edit key bisa merusak MSDM (BUG-9):** Fix v2.2.1 mengganti `String::from_utf8` jadi `from_utf8_lossy` supaya key terbaca. Tapi `from_utf8_lossy` mengubah setiap byte biner (>0x7F, umum di header MSDM sebelum key) jadi karakter U+FFFD yang **3 byte** di UTF-8. Akibatnya `mat.start()` menggeser `windows_key_offset` sejauh jumlah byte biner (terbukti skew +3). Ketika operator klik Edit → Save Windows Key, aplikasi menulis key baru di offset yang meleset → MSDM table korup. Diganti helper `ascii_map_1to1` yang menjaga panjang string 1:1 dengan buffer (1 byte = 1 char), jadi offset selalu akurat. Berlaku untuk DMI Extractor dan DMI Injector.
- **Progress bar salah skala setelah error (BUG-10):** `instantStageRef` tidak di-reset saat operasi gagal. Kalau Instant Mode error di tengah, operasi berikutnya (Read/Write biasa) progress bar-nya masih memakai skala Instant (erase 0-33%, write 33-66%, verify 66-100%) sehingga tampil salah. Sekarang di-reset di blok `finally`.
- **Potensi tabrakan file temp (hardening):** Nama file sementara di `/tmp` hanya pakai PID proses. Karena semua operasi jalan dalam satu proses aplikasi, dua operasi dalam sesi yang sama bisa memakai path identik. Ditambah timestamp nano-detik (`pid + nanos`) supaya unik.

### Changed
- **Unit test 14 → 16.** Ditambah `test_key_offset_akurat_walau_byte_biner` (mengunci BUG-9: offset yang dilaporkan harus menunjuk awal key sebenarnya) dan `test_ascii_map_panjang_1to1` (memastikan panjang string == jumlah byte).

### Catatan
Audit ronde 2 ini menemukan bahwa **fix BUG-1 di v2.2.1 sendiri mengandung bug baru (BUG-9)**. `from_utf8_lossy` benar membuat key *terbaca* (tujuan BUG-1 tercapai), tapi merusak *akurasi offset* — yang baru berdampak saat fitur Edit DMI dipakai. Test regresi v2.2.1 hanya mengecek key terbaca, tidak mengecek offset-nya, makanya lolos.

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
