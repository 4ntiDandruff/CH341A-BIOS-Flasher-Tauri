# ⚡ BIOS Flasher Professional Edition v2.1.0
### 🔧 *The Ultimate SPI Flasher Tool for Megapass Sidoarjo*

[![Tauri](https://img.shields.io/badge/Framework-Tauri%20v2-blue?logo=tauri)](https://tauri.app/)
[![React](https://img.shields.io/badge/Frontend-React%20%2B%20Tailwind-61dafb?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Backend-Rust-red?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

Aplikasi GUI modern, super cepat, dan ringan berbasis **Tauri v2 (Rust Backend & React Frontend)** untuk mengontrol **flashrom** menggunakan programmer **CH341A USB SPI**. Didesain khusus untuk memenuhi kebutuhan *daily workflow* teknisi handphone, laptop, dan komputer profesional di **Megapass Sidoarjo**.

---

## 🚀 Fitur Unggulan

### 1. 🔍 Auto-Detect Chip (1-Click)
Mendeteksi IC EEPROM SPI secara instan. Menampilkan nama chip, pabrikan (Winbond, Macronix, Gigadevice, dll.), kapasitas memori (MB), voltase kerja (1.8V / 3.3V), dan tipe package (SOP8, DIP8, SOP16).

### 2. 📖 Fast Read & Buffer Analytics
Membaca firmware BIOS ke buffer memori dengan sangat cepat. Dilengkapi indikator persentase memori terpakai (*used data percentage*) agar teknisi langsung tahu apakah IC tersebut kosong (semua 0xFF) atau terisi data.

### 3. 📟 Smart DMI & License Auto-Extractor (Offline)
Fitur *killer* untuk servis motherboard laptop. Aplikasi akan memindai isi BIOS secara otomatis untuk mengekstrak:
*   🔑 **Windows Product Key (OEM):** Membaca ACPI MSDM table bawaan pabrik agar lisensi Windows pelanggan tidak hilang.
*   📋 **Serial Number (S/N):** Menampilkan nomor seri asli unit laptop.
*   🏷️ **Dell Service Tag:** Mengekstrak 7-digit kode Dell secara otomatis.
*   ⚙️ **HP Board ID (BID):** Menampilkan kode BID unik HP untuk mencegah salah flash (*no display*).
*   📋 **1-Click Copy:** Tombol copy clipboard di setiap baris data untuk menyalin data secara instan tanpa ribet.

### 4. ⚡ Instant Mode (Erase → Write → Verify)
Meningkatkan produktivitas servis dengan otomatisasi 3 langkah penting sekali klik. Sangat efisien untuk pengerjaan antrean servis yang menumpuk.

### 5. 🔍 Hex Viewer dengan Local Search
Hex Viewer bawaan yang responsif dengan fitur pencarian teks ASCII maupun string Hex (misal mencari string header `AMIBIOS` atau `MSDM`). Viewer secara cerdas akan otomatis melompati deretan kosong `0xFF` langsung menuju baris awal data pertama.

### 6. 🟢 Live USB Pulse Indicator
Lampu indikator koneksi hardware programmer CH341A secara real-time (Hijau berkedip = Terhubung, Merah berkedip = Terputus/Tidak Terdeteksi).

### 7. 🔔 Chime & Alert Sound
Menggunakan Web Audio API untuk notifikasi suara yang bersahabat:
*   🎵 **1x Chime Ringan (Sine Wave):** Notifikasi sukses proses.
*   🚨 **1x Buzz Berat (Triangle Wave):** Peringatan keras saat terjadi gagal proses/bad connection.

---

## 🖥️ Kebutuhan Sistem

*   **Sistem Operasi:** Linux (Wayland / X11) - Diuji optimal pada Kubuntu / Ubuntu.
*   **Perangkat Keras:** CH341A Programmer (USB ID `1a86:5512`).
*   **Dependencies:**
    *   `flashrom` (v1.6.0 atau versi terbaru)
    *   `Node.js` (v22.x) & `npm`
    *   `Rust` & `Cargo` compiler

---

## 🔧 Instalasi & Setup

### 1. Konfigurasi Udev Rules (Akses Non-Sudo)
Agar aplikasi dapat mendeteksi USB Programmer tanpa akses `root`/`sudo`:
```bash
echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="5512", MODE="0666"' | sudo tee /etc/udev/rules.d/40-ch341a.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

### 2. Clone & Install Dependencies
```bash
git clone https://github.com/4ntiDandruff/CH341A-BIOS-Flasher-Tauri.git
cd CH341A-BIOS-Flasher-Tauri
npm install
```

### 3. Mode Development (Debug)
```bash
npm run tauri dev
```

### 4. Build Native Binary (Release Produksi)
Kompilasi ke aplikasi native standalone (.AppImage, .deb):
```bash
npm run tauri build
```
Hasil build binary dapat ditemukan di: `src-tauri/target/release/bios-flasher`.

---

## ℹ️ Tentang Megapass Sidoarjo
**Megapass Sidoarjo** adalah pusat servis handphone, laptop, dan komputer terpercaya di Sidoarjo, Jawa Timur. Kami melayani perbaikan motherboard tingkat lanjut, pemrograman firmware BIOS, dan optimasi hardware.
