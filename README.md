# ⚡ BIOS Flasher Professional Edition v2.1.0
### 🔧 *The Ultimate SPI Flasher Tool for Megapass Sidoarjo*

[![Tauri](https://img.shields.io/badge/Framework-Tauri%20v2-blue?logo=tauri)](https://tauri.app/)
[![React](https://img.shields.io/badge/Frontend-React%20%2B%20Tailwind-61dafb?logo=react)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Backend-Rust-red?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)

Aplikasi GUI modern, super cepat, dan ringan berbasis **Tauri v2 (Rust Backend & React Frontend)** untuk mengontrol **flashrom** menggunakan programmer **CH341A USB SPI**. Didesain khusus untuk memenuhi kebutuhan *daily workflow* teknisi handphone, laptop, dan komputer profesional di **Megapass Sidoarjo**.

---

## 🚀 Fitur Unggulan

1.  **🔍 Auto-Detect Chip (1-Click):** Mendeteksi IC EEPROM SPI secara instan beserta informasi voltage (1.8V / 3.3V) dan tipe package.
2.  **📖 Fast Read & Buffer Analytics:** Membaca firmware BIOS ke buffer memori RAM dengan analisis persentase data terpakai (non-FF bytes).
3.  **📟 Smart DMI & License Auto-Extractor (Offline):** Ekstraksi Windows Product Key (MSDM), Serial Number (S/N), Dell Service Tag, dan HP Board ID (BID) secara otomatis dilengkapi tombol copy clipboard 📋.
4.  **💉 DMI Injector (Identity Merger):** Memindahkan data DMI asli (Serial/Windows Key) dari BIOS lama (korup) ke BIOS baru (Clean) secara aman dan instan dengan 1 klik sebelum di-flash.
5.  **🧹 Intel ME Region Cleaner & Detector:** Deteksi otomatis region Intel ME (Management Engine) dan pembersihan status initialization (Dirty -> Clean Unconfigured State) untuk mengatasi problem laptop *late display / restart 30 menit*.
6.  **📊 Side-by-Side Hex Diff Viewer:** Fitur pembanding dua file biner BIOS secara byte-per-byte dengan penandaan (highlight) visual bintang merah `*XX*` di Hex Viewer untuk byte yang memiliki perbedaan.
7.  **🚨 BSOD Red Alert Diagnostic Boundary:** Menangkap crash/error sistem di level hardware/software dan menyajikan data error terstruktur (Kode Error, File Source, Line, & Context) untuk memudahkan perbaikan instan oleh AI tanpa halusinasi.
8.  **⚡ Instant Mode:** Pipeline otomatisasi sekali jalan: Erase → Write → Verify.
9.  **🟢 Live USB Pulse Indicator:** Indikator pendeteksi status koneksi hardware programmer CH341A secara real-time.
10. **🔊 Chime & Alert Sound:** Web Audio API untuk nada status (Chime sukses, Buzz gagal).

---

## 📦 Dokumentasi Dependencies

### A. System Level (Linux OS)
*   **`flashrom` (v1.6.0+):** Utility backend utama untuk interaksi chip SPI Flash.
*   **`lsusb`:** Utilitas sistem untuk deteksi koneksi USB programmer.
*   **`WebKit2GTK`:** Engine rendering browser bawaan Linux untuk GUI Tauri.

### B. Backend Level (Rust / Cargo)
*   **`tauri` (v2.x):** Framework inti integrasi desktop dan security sandbox.
*   **`tauri-plugin-dialog` & `tauri-plugin-fs`:** Plugin dialog buka/simpan file native OS serta read/write file lokal.
*   **`regex`:** Engine pencarian pola teks berkecepatan tinggi untuk ekstraksi data DMI.
*   **`md5`:** Library checksum MD5 untuk validasi file backup.
*   **`serde` & `serde_json`:** Library serialisasi data pertukaran format JSON antara Rust & React.

### C. Frontend Level (React / Node.js)
*   **`React` & `Vite`:** React library dan build tool super cepat.
*   **`Tailwind CSS v4` & `daisyUI v5`:** Framework styling bertema gelap (*Dark Mode default*).

---

## ⚙️ Cara Kerja Logika & Alur Data (Logic Workflow)

Aplikasi ini menggunakan pola komunikasi **IPC (Inter-Process Communication)** asinkron antara Frontend dan Backend:

```
+------------------+    IPC (Invoke)     +--------------------+
|  React Frontend  | ------------------> |    Tauri Backend   |
|     (UI-UX)      | <------------------ |  (Rust Executable) |
+------------------+    Event (Emit)     +--------------------+
         |                                         |
         v (Web Audio API)                         v (Command Line wrapper)
     Sound Out                                 flashrom / lsusb
```

### A. Alur Kerja Deteksi & Pembacaan Chip (Detect & Read)
1. User memicu aksi **Detect** / **Read** di frontend React.
2. React memanggil fungsi Tauri IPC: `invoke("detect_chip")` atau `invoke("read_bios")`.
3. Di Rust backend, thread baru dibuat (`std::thread::spawn`) agar proses panjang `flashrom` tidak membekukan (freeze) GUI.
4. Rust menjalankan proses CLI `flashrom` di background dan menyaring output progress (`\d+%`) menggunakan regex untuk dipancarkan (*emit*) kembali ke React guna memperbarui progress bar di UI secara real-time.
5. Selesai proses, biner BIOS dibaca ke array byte (`Vec<u8>`) di RAM dan dikirim ke React.

### B. Logika Ekstraksi DMI & Windows Key (DMI Parser)
Saat file BIOS selesai dimuat ke RAM, fungsi `extract_dmi_and_key` memproses data mentah biner:
1.  **Pencarian Windows Key:** Memindai buffer biner untuk mencari header string `MSDM` (Microsoft Data Table) via `.windows(4).position(...)`. Jika ketemu, data 120-byte setelahnya diekstraksi menggunakan regex lisensi 25-digit.
2.  **Identifikasi Brand & Model:** Mengonversi data biner ke string ASCII bersih dengan kompresi spasi ganda. Mencari signature produsen (`ASUS`, `LENOVO`, `DELL`, `HP`, `ACER`).
3.  **HP & Dell Fallback:** Menggunakan regex pola khusus untuk mencari HP Serial Number (10 digit diawali `5CG/5CD/etc.`), HP Board ID (BID), dan Dell Service Tag (7 digit alfanumerik).

### C. Logika Smart Hex Viewer (Auto-Skip 0xFF)
*   **Cara kerja:** Fungsi `formatHex` di `App.jsx` memindai buffer. Jika area depan berisi data kosong (`0xFF` / `0x00`) beruntun melampaui `64KB`, viewer otomatis melompati area kosong tersebut (*auto-skip*) dan langsung menampilkan baris awal data pertama.

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
