# SOP MEJA — BIOS FLASHER MEGAPASS
**Tool:** CH341A + BIOS Flasher v2.1.3  
**Print:** A4 portrait · tempel di meja servis  
**Tujuan:** Cegah brick 2x, cegah chip 1.8V hangus, cegah lupa backup

---

## SEBELUM MULAI (WAJIB)

| No | Cek | OK? |
|----|-----|-----|
| 1 | Laptop **mati total**: cabut baterai + adaptor | ☐ |
| 2 | Tunggu **30 detik** (kapasitor kosong) | ☐ |
| 3 | Clip CH341A duduk rata, 8 pin nempel | ☐ |
| 4 | App: pulse **HIJAU** (USB colok) | ☐ |
| 5 | Siapkan folder: `~/bios-backup/NAMA_CUSTOMER/` | ☐ |

> Analogi: kayak ganti IC power — **sumber listrik harus 0V** dulu, baru tembak.

---

## ALUR KERJA STANDAR (1 UNIT)

```
1 DETECT  →  2 READ  →  3 BACKUP  →  4 SIAPKAN FILE
                    ↓
5 (opsional) DMI INJECT / ME CLEAN
                    ↓
6 ERASE → WRITE → VERIFY   atau   INSTANT MODE
                    ↓
7 VERIFY LOLOS  →  cabut clip / solder balik  →  tes nyala
```

### Detail langkah

1. **Detect Chip**  
   - Catat: nama chip + voltage  
   - Kalau muncul **1.8V** → **STOP**. Pasang adapter level shifter dulu.

2. **Read**  
   - Hex viewer harus **bukan all-FF** (kecuali chip memang kosong)  
   - Kalau awal hex FF tapi ada data di offset tinggi → normal (padding)

3. **Backup** (WAJIB, walau chip rusak)  
   - Nama file: `MERK_MODEL_SNatauBID_YYYYMMDD.bin`  
   - Contoh: `HP_15s_5CG1234_BID080C7_20260723.bin`  
   - Unit mahal / brick kritis: backup **2–3x**, banding MD5 harus sama

4. **Siapkan file tulis**  
   - Load File clean BIOS, **atau**  
   - DMI Injector: BIOS rusak (serial/key) + clean BIOS → merge  
   - Gejala late display / mati 30 menit → **Clean ME Region**

5. **Write aman**  
   - Pre-flight harus lolos (USB + chip + buffer)  
   - Prefer **Instant Mode** (Erase → Write → Verify)  
   - **JANGAN** cabut USB / goyang clip selama proses

6. **Verify HARUS lolos**  
   - Belum VERIFIED = **belum selesai**  
   - Jangan solder balik / rakit dulu

7. **Tes unit**  
   - Power on → POST / logo → masuk BIOS setup  
   - Cek serial / Windows key bila relevan

---

## LARANGAN KERAS

| ❌ JANGAN | Kenapa |
|-----------|--------|
| Flash tanpa backup | Data asli hilang, gak bisa rollback |
| Cabut USB saat write/erase/verify | Chip setengah tulis = brick |
| 3.3V ke chip **1.8V** tanpa adapter | Chip hangus (irreversible) |
| Laptop masih standby power | Timeout / data korup |
| Anggap “hex FF di awal” = gagal write | Sering cuma padding; cek first data offset |
| Skip verify | Risiko rakit chip jelek |

---

## CHECKLIST CEPAT PRE-FLIGHT (di app v2.1.3)

App otomatis tolak Write/Erase/Instant Mode kalau:
- Pulse **merah** (CH341A gak kebaca)
- Chip belum Detect
- Buffer kosong (belum Read / Load File)
- Chip 1.8V belum dikonfirmasi adapter

Kalau ditolak: **perbaiki kondisi**, jangan dipaksa.

---

## NAMA FILE & SIMPANAN

```
~/bios-backup/
  └── NAMA_CUSTOMER/
        ├── ORIGINAL_YYYYMMDD.bin      ← dump asli
        ├── ORIGINAL_YYYYMMDD_md5.txt  ← opsional
        ├── CLEAN_atau_MERGED.bin      ← yang di-flash
        └── CATATAN.txt               ← keluhan + hasil
```

**Isi CATATAN.txt minimal:**
- Nama customer / no nota
- Merk-model-SN
- Chip + voltage
- Keluhan (brick / late display / no display)
- File yang di-flash
- Hasil verify + tes nyala

---

## TROUBLE CEPAT

| Gejala | Cek dulu |
|--------|----------|
| Detect gagal | Clip goyang, pin kotor, laptop masih bertegangan, pulse USB |
| Read timeout | Power total mati + 30 dtk; reseat clip |
| Multiple chip definition | Pilih nama chip pertama dari detect (biasanya benar) |
| Write/Verify mismatch | Clip longgar; ulang dari Read/Backup; jangan rakit |
| Pulse merah | Cabut-colok CH341A; cek kabel USB data (bukan charge-only) |
| Permission flashrom | Udev rule `40-ch341a.rules` harus ada; cabut-colok USB |

---

## ESTIMASI WAKTU

| Metode | Waktu tipikal |
|--------|----------------|
| In-circuit (clip) | 30–60 menit |
| Desolder + ZIF | 2–4 jam |
| Flash saja (chip sudah di socket) | 5–15 menit |

---

## SETELAH SELESAI

1. Simpan backup + catatan di folder customer  
2. Label fisik chip/board bila perlu  
3. Tes boot final di depan (kalau customer tunggu)  
4. Kalau gagal: export log/BSOD diagnostic dari app → kirim ke AI/teknisi senior

---

## VERSI TOOL

- App: **BIOS Flasher Megapass v2.1.3**
- Engine: flashrom + CH341A USB
- Repo: github.com/4ntiDandruff/CH341A-BIOS-Flasher-Tauri

**Megapass Sidoarjo — Servis Hardware**  
*1 poster = 1 SOP. Baca sebelum tembak chip.*
