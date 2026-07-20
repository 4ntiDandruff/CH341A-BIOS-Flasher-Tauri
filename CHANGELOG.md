# Changelog - BIOS Flasher

Semua perubahan penting pada proyek **BIOS Flasher** akan dicatat di file ini.

## [2.1.0] - 2026-07-20
### Added
- Rebuild total arsitektur clean-slate dari Python ke **Tauri (Rust + React + Tailwind CSS v4 + daisyUI)**.
- Fitur **⚡ Instant Mode** (Erase → Write → Verify otomatis sekali klik).
- Indikator deteksi USB CH341A (`USB Connected / Disconnected`) dengan efek pulse visual.
- Notifikasi suara keras (1x chime untuk sukses, 1x buzz untuk error hardware/IC rusak).
- Logger dengan durasi presisi dalam format `MM.SS` (Menit.Detik).
- Dialog pop-up "About Developer" Megapass Sidoarjo.
- Database chip `chips.json` (34+ IC Winbond, Macronix, GigaDevice, dll.) dengan info tegangan (1.8V/3.3V) dan tipe package.
- Hex Viewer pintar (otomatis skip area kosong `0xFF` ke awal data offset pertama).
- Window size dinaikkan ke `1100x800px` untuk layout visual yang lebih lega.

### Fixed
- Error JSON Parse saat proses deteksi chip.
- Penambahan izin plugin Tauri (`dialog` & `fs` permissions) di `capabilities/default.json` agar fitur backup dan open backup berjalan sukses.
- Perbaikan teks brand konsisten "Megapass" dan perbaikan string Window Title.

## [2.0.0] - 2026-07-19
- Migrasi GUI dari YAD script ke custom Python GTK3 UI.
- Penambahan layout visual split 40/60.

## [1.1.0] - 2026-07-19
- Migrasi dari Zenity ke YAD untuk performa alignment kolom.

## [1.0.0] - 2026-07-19
- Rilis perdana menggunakan bash script + Zenity GUI.
