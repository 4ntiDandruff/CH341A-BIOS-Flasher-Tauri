use md5::{Digest, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tauri::{Emitter, Manager};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DiagnosticError {
    code: String,
    message: String,
    file: String,
    line: u32,
    context: String,
}

macro_rules! diagnose_err {
    ($code:expr, $msg:expr, $ctx:expr) => {
        DiagnosticError {
            code: $code.to_string(),
            message: $msg.to_string(),
            file: file!().to_string(),
            line: line!(),
            context: $ctx.to_string(),
        }
    };
}

#[derive(Serialize, Deserialize, Clone)]
struct ProgressPayload {
    percent: f64,
    stage: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct LogPayload {
    message: String,
}

fn emit_log(window: &tauri::Window, msg: &str) {
    let _ = window.emit(
        "operation-log",
        LogPayload {
            message: msg.to_string(),
        },
    );
}

fn emit_progress(window: &tauri::Window, percent: f64, stage: &str) {
    let _ = window.emit(
        "operation-progress",
        ProgressPayload {
            percent,
            stage: stage.to_string(),
        },
    );
}

fn parse_progress(line: &str) -> Option<f64> {
    let re = Regex::new(r"(\d+)%").ok()?;
    let caps = re.captures(line)?;
    caps.get(1)?.as_str().parse::<f64>().ok()
}

/// Sanitasi buffer biner → String 1 byte = 1 char.
/// JANGAN pakai from_utf8_lossy untuk hitung offset: byte invalid jadi U+FFFD
/// (3 byte UTF-8) → mat.start() meleset (terbukti skew +3 di audit v2.2.2).
fn ascii_map_1to1(data: &[u8]) -> String {
    data.iter()
        .map(|&b| {
            if (0x20..=0x7E).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

fn unique_tmp(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("/tmp/{}_{}_{}.bin", prefix, std::process::id(), nanos)
}

fn run_flashrom_with_progress(
    args: &[&str],
    window: &tauri::Window,
    stage: &str,
) -> Result<String, String> {
    emit_log(window, &format!("Running: flashrom {}", args.join(" ")));
    emit_progress(window, 0.0, stage);

    let mut child = Command::new("flashrom")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn flashrom: {}", e))?;

    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
    let reader = BufReader::new(stderr);

    let win = window.clone();
    let stage_owned = stage.to_string();
    let stderr_handle = std::thread::spawn(move || {
        let mut lines = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            if let Some(pct) = parse_progress(&line) {
                emit_progress(&win, pct, &stage_owned);
            }
            emit_log(&win, &line);
            lines.push(line);
        }
        lines.join("\n")
    });

    // FIX BUG-4: flashrom menulis output normal (termasuk progress) ke STDOUT.
    // Sebelumnya stdout hanya dikumpulkan di akhir tanpa parsing progress, jadi
    // bar diam di 0% lalu lompat 100%. Sekarang stdout di-stream & di-parse juga.
    let stdout_pipe = child.stdout.take().ok_or("Failed to capture stdout")?;
    let out_reader = BufReader::new(stdout_pipe);
    let win_out = window.clone();
    let stage_out = stage.to_string();
    let stdout_handle = std::thread::spawn(move || {
        let mut lines = Vec::new();
        for line in out_reader.lines().map_while(Result::ok) {
            if let Some(pct) = parse_progress(&line) {
                emit_progress(&win_out, pct, &stage_out);
            }
            emit_log(&win_out, &line);
            lines.push(line);
        }
        lines.join("\n")
    });

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for flashrom: {}", e))?;

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr_output = stderr_handle.join().unwrap_or_default();

    emit_progress(window, 100.0, stage);

    let combined = format!("{}\n{}", stdout, stderr_output);

    if status.success() {
        emit_log(window, &format!("{} completed successfully", stage));
        Ok(combined)
    } else {
        let msg = format!(
            "{} failed (exit code: {:?})\n{}",
            stage,
            status.code(),
            combined
        );
        emit_log(window, &msg);
        Err(msg)
    }
}

#[tauri::command]
fn check_usb() -> bool {
    if let Ok(output) = Command::new("lsusb").output() {
        let out = String::from_utf8_lossy(&output.stdout);
        return out.contains("1a86:5512");
    }
    false
}

#[tauri::command]
fn detect_chip() -> Result<serde_json::Value, String> {
    let output = Command::new("flashrom")
        .args(["-p", "ch341a_spi"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run flashrom: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);

    let re_found = Regex::new(r#"Found\s+.*?chip\s+"([^"]+)""#).unwrap();
    let re_multiple = Regex::new(r#"Multiple flash chip definitions match.*:\s*"([^"]+)""#).unwrap();

    let mut chips: Vec<String> = Vec::new();

    for cap in re_found.captures_iter(&combined) {
        if let Some(name) = cap.get(1) {
            let chip_name = name.as_str().to_string();
            if !chips.contains(&chip_name) {
                chips.push(chip_name);
            }
        }
    }

    if chips.is_empty() {
        for cap in re_multiple.captures_iter(&combined) {
            if let Some(name) = cap.get(1) {
                chips.push(name.as_str().to_string());
            }
        }
        // Parse all quoted chip names after multi-match sentence
        if let Some(pos) = combined.find("Multiple flash chip definitions match") {
            let slice = &combined[pos..];
            let re_all = Regex::new(r#""([^"]+)""#).unwrap();
            for cap in re_all.captures_iter(slice) {
                if let Some(name) = cap.get(1) {
                    let n = name.as_str().to_string();
                    if n.len() >= 4 && !n.contains(' ') && !chips.contains(&n) {
                        chips.push(n);
                    }
                }
            }
        }
    }

    if chips.is_empty() {
        let re_quotes = Regex::new(r#""([A-Z][A-Za-z0-9_]+\d+[A-Za-z0-9_]*)""#).unwrap();
        for cap in re_quotes.captures_iter(&combined) {
            if let Some(name) = cap.get(1) {
                let chip_name = name.as_str().to_string();
                if !chips.contains(&chip_name) {
                    chips.push(chip_name);
                }
            }
        }
    }

    Ok(serde_json::json!({
        "chips": chips,
        "raw_output": combined.trim(),
        "detected": !chips.is_empty(),
    }))
}

#[tauri::command]
async fn read_bios(chip: String, window: tauri::Window) -> Result<Vec<u8>, String> {
    let result = std::thread::spawn(move || {
        let output_path = unique_tmp("bios_read");

        let result = run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-r", &output_path],
            &window,
            "Reading",
        );

        match result {
            Ok(_) => {
                let data = fs::read(&output_path)
                    .map_err(|e| format!("Failed to read output file: {}", e));
                let _ = fs::remove_file(&output_path);
                data
            }
            Err(e) => {
                let _ = fs::remove_file(&output_path);
                Err(e)
            }
        }
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    Ok(result)
}

#[tauri::command]
fn backup_bios(path: String, data: Vec<u8>) -> Result<String, String> {
    fs::write(&path, &data).map_err(|e| format!("Failed to write backup: {}", e))?;

    let mut hasher = Md5::new();
    hasher.update(&data);
    let hash = hasher.finalize();
    let md5_hex = format!("{:x}", hash);

    Ok(md5_hex)
}

#[tauri::command]
fn open_backup(path: String) -> Result<Vec<u8>, String> {
    fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
fn inject_dmi(data_old: Vec<u8>, data_new: Vec<u8>) -> Result<Vec<u8>, DiagnosticError> {
    if data_old.is_empty() || data_new.is_empty() {
        return Err(diagnose_err!(
            "ERR_DMI_EMPTY_BUFFER_0x201",
            "BIOS buffer data is empty",
            format!("data_old_len: {}, data_new_len: {}", data_old.len(), data_new.len())
        ));
    }

    let mut output_data = data_new.clone();

    // FIX BUG-2: signature b"DMI" (3 byte) terlalu pendek -> cocok dengan string
    // sampah biasa di BIOS (mis. "DMI EDIT TOOL"), lalu 64KB di-copy BUTA dari
    // offset salah -> region acak ketimpa -> BIOS brick.
    // Sekarang: hanya terima anchor yang tervalidasi strukturnya.
    let mut dmi_offset: Option<usize> = None;

    // Anchor 1: MSDM (ACPI table). Validasi: length field masuk akal (>= 0x55).
    if let Some(pos) = data_old.windows(4).position(|w| w == b"MSDM") {
        if pos + 8 <= data_old.len() {
            let len_field = u32::from_le_bytes([
                data_old[pos + 4],
                data_old[pos + 5],
                data_old[pos + 6],
                data_old[pos + 7],
            ]);
            if (0x55..=0x1000).contains(&len_field) {
                dmi_offset = Some((pos / 0x1000) * 0x1000);
            }
        }
    }

    // Anchor 2: SMBIOS entry point "_SM_" / "_SM3_" — struktur nyata, bukan tebakan.
    if dmi_offset.is_none() {
        if let Some(pos) = data_old.windows(4).position(|w| w == b"_SM_") {
            // entry point diikuti checksum + length; length valid biasanya 0x1F/0x18
            if pos + 6 <= data_old.len() && data_old[pos + 5] >= 0x18 {
                dmi_offset = Some((pos / 0x1000) * 0x1000);
            }
        }
    }

    // Anchor 3: "_DMI_" (5 byte, SMBIOS intermediate anchor). Lebih spesifik dari "DMI".
    if dmi_offset.is_none() {
        if let Some(pos) = data_old.windows(5).position(|w| w == b"_DMI_") {
            dmi_offset = Some((pos / 0x1000) * 0x1000);
        }
    }

    if let Some(offset) = dmi_offset {
        let block_size = 0x10000;
        if offset + block_size <= data_old.len() && offset + block_size <= output_data.len() {
            output_data[offset..offset + block_size]
                .copy_from_slice(&data_old[offset..offset + block_size]);
            return Ok(output_data);
        }
    }

    if let Some(old_msdm_pos) = data_old.windows(4).position(|w| w == b"MSDM") {
        if let Some(new_msdm_pos) = output_data.windows(4).position(|w| w == b"MSDM") {
            let old_key_segment = &data_old[old_msdm_pos..std::cmp::min(data_old.len(), old_msdm_pos + 120)];
            let new_key_segment = &output_data[new_msdm_pos..std::cmp::min(output_data.len(), new_msdm_pos + 120)];

            // FIX BUG-9: jangan from_utf8_lossy — U+FFFD bikin mat.start() skew.
            // ascii_map_1to1 jaga panjang 1:1 dengan buffer biner.
            let old_text = ascii_map_1to1(old_key_segment);
            let new_text = ascii_map_1to1(new_key_segment);

            let re_key = Regex::new(r"[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}").unwrap();
            if let Some(old_mat) = re_key.find(&old_text) {
                let old_key_str = old_mat.as_str();
                if let Some(new_mat) = re_key.find(&new_text) {
                    let start_replace = new_msdm_pos + new_mat.start();
                    let end_replace = new_msdm_pos + new_mat.end();
                    if end_replace <= output_data.len()
                        && (end_replace - start_replace) == old_key_str.len()
                    {
                        output_data[start_replace..end_replace]
                            .copy_from_slice(old_key_str.as_bytes());
                        return Ok(output_data);
                    }
                }
            }
        }
    }

    Err(diagnose_err!(
        "ERR_DMI_SIGNATURE_NOT_FOUND_0x202",
        "Could not locate compatible DMI/MSDM block offsets to inject",
        format!("data_old_len: {}, data_new_len: {}", data_old.len(), data_new.len())
    ))
}

#[derive(Serialize, Deserialize, Clone)]
struct CompareResult {
    identical: bool,
    size_match: bool,
    size_a: usize,
    size_b: usize,
    hash_a: String,
    hash_b: String,
    diff_count: usize,
    /// First differing offsets (capped) for hex markers
    diff_offsets: Vec<usize>,
    first_offset: Option<usize>,
    sample_capped: bool,
    message: String,
}

fn md5_hex(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[tauri::command]
fn compare_bios_diff(data_a: Vec<u8>, data_b: Vec<u8>) -> Result<CompareResult, String> {
    let size_a = data_a.len();
    let size_b = data_b.len();
    let hash_a = md5_hex(&data_a);
    let hash_b = md5_hex(&data_b);

    if size_a == 0 || size_b == 0 {
        return Err("Salah satu file kosong - pilih file .bin yang valid".to_string());
    }

    if size_a != size_b {
        let msg = format!(
            "Size beda: {:.2}MB vs {:.2}MB - tidak bisa dibanding byte-per-byte. Pakai dump full size yang sama.",
            size_a as f64 / 1048576.0,
            size_b as f64 / 1048576.0
        );
        return Ok(CompareResult {
            identical: false,
            size_match: false,
            size_a,
            size_b,
            hash_a,
            hash_b,
            diff_count: 0,
            diff_offsets: vec![],
            first_offset: None,
            sample_capped: false,
            message: msg,
        });
    }

    // Fast path: same hash => identical
    if hash_a == hash_b {
        return Ok(CompareResult {
            identical: true,
            size_match: true,
            size_a,
            size_b,
            hash_a: hash_a.clone(),
            hash_b: hash_b.clone(),
            diff_count: 0,
            diff_offsets: vec![],
            first_offset: None,
            sample_capped: false,
            message: format!(
                "IDENTIK - {} bytes ({:.2}MB), MD5 {}",
                size_a,
                size_a as f64 / 1048576.0,
                hash_a
            ),
        });
    }

    let mut diff_offsets: Vec<usize> = Vec::new();
    let mut diff_count: usize = 0;
    let mut first_offset: Option<usize> = None;
    const SAMPLE_CAP: usize = 1000;

    for i in 0..size_a {
        if data_a[i] != data_b[i] {
            if first_offset.is_none() {
                first_offset = Some(i);
            }
            diff_count += 1;
            if diff_offsets.len() < SAMPLE_CAP {
                diff_offsets.push(i);
            }
        }
    }

    let sample_capped = diff_count > SAMPLE_CAP;
    let pct = (diff_count as f64 / size_a as f64) * 100.0;
    let first_txt = first_offset
        .map(|o| format!("0x{:08X}", o))
        .unwrap_or_else(|| "-".to_string());

    let msg = format!(
        "BEDA - {} byte beda ({:.4}%) | size {:.2}MB | offset pertama {} | MD5 A {} | MD5 B {}{}",
        diff_count,
        pct,
        size_a as f64 / 1048576.0,
        first_txt,
        hash_a,
        hash_b,
        if sample_capped {
            format!(" | hex menandai {} offset pertama", SAMPLE_CAP)
        } else {
            String::new()
        }
    );

    Ok(CompareResult {
        identical: false,
        size_match: true,
        size_a,
        size_b,
        hash_a,
        hash_b,
        diff_count,
        diff_offsets,
        first_offset,
        sample_capped,
        message: msg,
    })
}


#[tauri::command]
fn analyze_me_region(data: Vec<u8>) -> serde_json::Value {
    let mut found = false;
    let mut offset_hex = "Not Found".to_string();
    let mut size_kb = 0;
    let mut version = "Unknown".to_string();
    let mut status = "Unknown".to_string();

    // Intel Flash Partition Table ($FPT) signature
    if let Some(pos) = data.windows(4).position(|w| w == b"$FPT") {
        found = true;
        offset_hex = format!("0x{:08X}", pos);
        
        // Scan for version string nearby (e.g., 11.8.50, 12.0.20, etc.)
        let start = pos;
        let end = std::cmp::min(data.len(), pos + 512);
        let segment = &data[start..end];
        let ascii_segment: String = segment.iter()
            .map(|&b| if b.is_ascii() && (0x20..=0x7E).contains(&b) { b as char } else { ' ' })
            .collect();

        let re_ver = Regex::new(r"\b(\d{1,2}\.\d{1,2}\.\d{1,2}\.\d{4})\b").unwrap();
        if let Some(mat) = re_ver.find(&ascii_segment) {
            version = mat.as_str().to_string();
        } else {
            version = "Intel ME (Generic)".to_string();
        }

        // Determine rough size based on Intel Descriptor specs (standard ME size is 1.5MB to 5MB)
        size_kb = 2048; // Default estimate 2MB
        status = "Initialized (Dirty)".to_string();
    }

    serde_json::json!({
        "found": found,
        "offset": offset_hex,
        "size_kb": size_kb,
        "version": version,
        "status": status
    })
}

#[tauri::command]
fn clean_me_region(data: Vec<u8>, mode: String, app_handle: tauri::AppHandle) -> Result<Vec<u8>, DiagnosticError> {
    if data.is_empty() {
        return Err(diagnose_err!(
            "ERR_ME_EMPTY_BUFFER_0x301",
            "BIOS buffer is empty",
            "data_len: 0"
        ));
    }

    if mode == "python" {
        // Run me_cleaner.py script
        let temp_in = unique_tmp("me_cleaner_in");
        let temp_out = unique_tmp("me_cleaner_out");

        if let Err(e) = fs::write(&temp_in, &data) {
            // FIX #6 (audit ronde 4/LOW): dulu return langsung tanpa hapus temp_in.
            // Kalau write gagal separuh (disk penuh), file BIOS parsial (berisi
            // lisensi/serial customer) nyangkut di /tmp world-readable. Bersihkan.
            let _ = fs::remove_file(&temp_in);
            return Err(diagnose_err!(
                "ERR_ME_WRITE_TEMP_0x303",
                "Failed to write temp ME file",
                e.to_string()
            ));
        }

        // Find me_cleaner.py path
        let mut script_path = format!("{}/proyek/CH341A-programer/src-tauri/resources/me_cleaner.py", std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()));
        if let Ok(res_dir) = app_handle.path().resource_dir() {
            let p = res_dir.join("resources").join("me_cleaner.py");
            if p.exists() {
                script_path = p.to_string_lossy().to_string();
            } else {
                let p2 = res_dir.join("me_cleaner.py");
                if p2.exists() {
                    script_path = p2.to_string_lossy().to_string();
                }
            }
        }

        let output = Command::new("python3")
            .args([&script_path, &temp_in, "-O", &temp_out])
            .output();

        let _ = fs::remove_file(&temp_in);

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = format!("{}\n{}", stdout, stderr);

                if out.status.success() {
                    let cleaned = fs::read(&temp_out).map_err(|e| {
                        diagnose_err!(
                            "ERR_ME_READ_CLEANED_0x304",
                            "Failed to read cleaned ME file",
                            e.to_string()
                        )
                    });
                    let _ = fs::remove_file(&temp_out);
                    return cleaned;
                } else {
                    let _ = fs::remove_file(&temp_out);
                    return Err(diagnose_err!(
                        "ERR_ME_PYTHON_FAIL_0x305",
                        "me_cleaner.py failed execution",
                        combined.trim().to_string()
                    ));
                }
            }
            Err(e) => {
                let _ = fs::remove_file(&temp_out);
                return Err(diagnose_err!(
                    "ERR_ME_SPAWN_PYTHON_0x306",
                    "Failed to execute python3 for me_cleaner",
                    e.to_string()
                ));
            }
        }
    }

    // FIX #4 (audit ronde 4/MEDIUM): mode "flag" DIBUANG.
    // Dulu ia nulis output_data[$FPT+16]=0xFF. Offset +0x10 itu field UMASize di
    // header $FPT, BUKAN toggle status ME, dan checksum header (offset +0x0B)
    // TIDAK dihitung ulang -> hasilnya UMASize korup + checksum basi, tapi UI
    // lapor "cleaned successfully". Selain palsu, bisa bikin ME gagal init.
    // Sekarang hanya jalur "python" (me_cleaner.py, terbukti benar) yang sah.
    Err(diagnose_err!(
        "ERR_ME_MODE_UNSUPPORTED_0x307",
        "Mode ME clean tidak didukung. Gunakan me_cleaner.py.",
        format!("mode: {}", mode)
    ))
}


#[tauri::command]
fn extract_dmi_and_key(data: Vec<u8>) -> serde_json::Value {
    let mut win_key = "Not Found".to_string();
    let mut win_key_offset = 0;
    let mut brand = "Unknown".to_string();
    let mut model = "Unknown".to_string();
    let mut serial_num = "Not Found".to_string();
    let mut serial_offset = 0;
    let mut board_id = "Not Found".to_string();
    let mut board_id_offset = 0;
    let mut service_tag = "Not Found".to_string();
    let mut service_tag_offset = 0;

    if let Some(pos) = data.windows(4).position(|w| w == b"MSDM") {
        let start = pos;
        let end = std::cmp::min(data.len(), pos + 120);
        let segment = &data[start..end];
        // FIX BUG-1 + BUG-9: MSDM biner. from_utf8 gagal total; from_utf8_lossy
        // bikin key terbaca tapi mat.start() SKEW (U+FFFD = 3 byte). Pakai map 1:1.
        let text = ascii_map_1to1(segment);
        let re_key = Regex::new(r"[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}").unwrap();
        if let Some(mat) = re_key.find(&text) {
            win_key = mat.as_str().to_string();
            win_key_offset = pos + mat.start();
        }
    }

    let mut ascii_chars = Vec::new();
    for &b in &data {
        if b.is_ascii() && (0x20..=0x7E).contains(&b) {
            ascii_chars.push(b as char);
        } else if (b == 0x00 || b == 0x0A || b == 0x0D) && ascii_chars.last() != Some(&' ') {
            ascii_chars.push(' ');
        }
    }
    let raw_text: String = ascii_chars.into_iter().collect();
    let re_spaces = Regex::new(r"\s+").unwrap();
    let clean_text = re_spaces.replace_all(&raw_text, " ");
    let upper_text = clean_text.to_uppercase();

    if upper_text.contains("LENOVO") { brand = "Lenovo".to_string(); }
    else if upper_text.contains("ASUSTEK") || upper_text.contains("ASUS") { brand = "ASUS".to_string(); }
    else if upper_text.contains("HEWLETT-PACKARD") || upper_text.contains("HP ") || upper_text.contains("HP.") { brand = "HP".to_string(); }
    else if upper_text.contains("DELL") { brand = "Dell".to_string(); }
    else if upper_text.contains("ACER") { brand = "Acer".to_string(); }
    else if upper_text.contains("TOSHIBA") { brand = "Toshiba".to_string(); }
    else if upper_text.contains("GIGABYTE") { brand = "Gigabyte".to_string(); }
    else if upper_text.contains("MSI") { brand = "MSI".to_string(); }

    // FIX #2 (audit ronde 4): DULU ambil kecocokan byte PERTAMA di seluruh buffer.
    // Field pendek (Board ID 4-6 char, Service Tag 7 char) gampang nabrak byte
    // kembar di region lain -> offset nunjuk lokasi SALAH -> edit nulis ke tempat
    // acak -> BIOS brick. Guard overwrite_dmi_value TIDAK nolong (byte di offset
    // salah itu justru cocok dengan nilai lama, itu sebabnya kepilih).
    //
    // Perbaikan: offset hanya sah kalau nilai muncul TEPAT 1x di buffer. Kalau
    // ambigu (>1x) atau tidak ada -> 0. UI memblokir edit saat offset 0, jadi
    // teknisi disuruh cek manual daripada nekat nulis ke lokasi tebakan.
    let find_field_offset = |raw_data: &[u8], field_val: &str| -> usize {
        if field_val.is_empty() || field_val == "Not Found" { return 0; }
        let bytes = field_val.as_bytes();
        if bytes.is_empty() || bytes.len() > raw_data.len() { return 0; }
        let mut first = None;
        let mut count = 0usize;
        for (i, w) in raw_data.windows(bytes.len()).enumerate() {
            if w == bytes {
                if first.is_none() { first = Some(i); }
                count += 1;
                if count > 1 { break; }
            }
        }
        // Offset 0 tetap "ambigu/not found" secara konvensi (UI cek !offset).
        // Field DMI nyata tidak pernah di byte 0 (itu vector reset/descriptor).
        match (first, count) {
            (Some(pos), 1) if pos != 0 => pos,
            _ => 0,
        }
    };

    let re_bid = Regex::new(r"(?i)BID([0-9A-F]{4,6})").unwrap();
    if let Some(cap) = re_bid.captures(&clean_text) {
        board_id = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        board_id_offset = find_field_offset(&data, &board_id);
    }

    if brand == "Dell" {
        let re_svctag = Regex::new(r"\b([A-Z0-9]{7})\b").unwrap();
        for cap in re_svctag.captures_iter(&clean_text) {
            let tag = cap.get(1).unwrap().as_str().to_string();
            if !tag.contains("SERVICE") && !tag.contains("VERSION") {
                service_tag = tag;
                service_tag_offset = find_field_offset(&data, &service_tag);
                break;
            }
        }
    }

    let re_sn = Regex::new(r"(?i)(?:serial\s*number|s/n|system\s*serial|serial\s*no|prodn)\s*[:=]?\s*([A-Z0-9]{8,20})").unwrap();
    if let Some(cap) = re_sn.captures(&clean_text) {
        serial_num = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        serial_offset = find_field_offset(&data, &serial_num);
    }

    if serial_num == "Not Found" && brand == "HP" {
        let re_hp_sn = Regex::new(r"\b(5CG|5CD|CND|CNU|5CB)[A-Z0-9]{7}\b").unwrap();
        if let Some(cap) = re_hp_sn.find(&clean_text) {
            serial_num = cap.as_str().to_string();
            serial_offset = find_field_offset(&data, &serial_num);
        }
    }

    if brand == "Lenovo" {
        let re_lenovo = Regex::new(r"(?i)(ThinkPad|IdeaPad|Yoga)\s+[A-Z0-9]{2,10}(?:\s+[A-Z0-9]{2,10})?").unwrap();
        if let Some(cap) = re_lenovo.find(&clean_text) {
            model = cap.as_str().trim().to_string();
        }
    } else if brand == "ASUS" {
        let re_asus = Regex::new(r"\b(X\d{3}[A-Z]{1,2}|UX\d{3}[A-Z]{0,2}|A\d{3}[A-Z]{1,2}|K\d{3}[A-Z]{1,2}|GL\d{3}[A-Z]{0,2})\b").unwrap();
        if let Some(cap) = re_asus.find(&clean_text) {
            model = cap.as_str().to_string();
        }
    } else if brand == "HP" {
        let re_hp = Regex::new(r"(?i)(?:ProBook|EliteBook|Pavilion|Spectre|Envy|HP\s+Notebook)\s+\d{3,4}(?:\s+G\d)?").unwrap();
        if let Some(cap) = re_hp.find(&clean_text) {
            model = cap.as_str().trim().to_string();
        }
    } else if brand == "Dell" {
        let re_dell = Regex::new(r"(?i)(?:Latitude|Inspiron|Vostro|Precision|OptiPlex|XPS)\s+\d{4}").unwrap();
        if let Some(cap) = re_dell.find(&clean_text) {
            model = cap.as_str().trim().to_string();
        }
    }

    serde_json::json!({
        "brand": brand,
        "model": model,
        "windows_key": win_key,
        "windows_key_offset": win_key_offset,
        "serial_number": serial_num,
        "serial_number_offset": serial_offset,
        "board_id": board_id,
        "board_id_offset": board_id_offset,
        "service_tag": service_tag,
        "service_tag_offset": service_tag_offset
    })
}


#[tauri::command]
async fn write_bios(chip: String, data: Vec<u8>, window: tauri::Window) -> Result<String, String> {
    if chip.trim().is_empty() {
        return Err("Chip name empty - Detect chip first".to_string());
    }
    if data.is_empty() {
        return Err("Write buffer empty - Load File or Read first".to_string());
    }
    let buffer_path = unique_tmp("bios_write");
    fs::write(&buffer_path, &data).map_err(|e| format!("Failed to write buffer: {}", e))?;

    let result = std::thread::spawn(move || {
        let res = run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-w", &buffer_path],
            &window,
            "Writing",
        );
        let _ = fs::remove_file(&buffer_path);
        res
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    Ok(result)
}

#[tauri::command]
async fn verify_bios(
    chip: String,
    data: Vec<u8>,
    window: tauri::Window,
) -> Result<String, String> {
    if chip.trim().is_empty() {
        return Err("Chip name empty - Detect chip first".to_string());
    }
    if data.is_empty() {
        return Err("Verify buffer empty - Load File or Read first".to_string());
    }
    let buffer_path = unique_tmp("bios_verify");
    fs::write(&buffer_path, &data).map_err(|e| format!("Failed to write buffer: {}", e))?;

    let result = std::thread::spawn(move || {
        let res = run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-v", &buffer_path],
            &window,
            "Verifying",
        );
        let _ = fs::remove_file(&buffer_path);
        res
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    let upper = result.to_uppercase();
    if upper.contains("VERIFIED")
        || upper.contains("VERIFICATION SUCCESSFUL")
        || (upper.contains("VERIFY") && upper.contains("SUCCESS"))
    {
        Ok("VERIFIED".to_string())
    } else if result.to_lowercase().contains("failed") || result.to_lowercase().contains("error") {
        Err(format!("VERIFY FAILED\n{}", result))
    } else {
        Ok(format!("Verification result: {}", result.lines().last().unwrap_or("unknown")))
    }
}

#[tauri::command]
async fn erase_bios(chip: String, window: tauri::Window) -> Result<String, String> {
    // FIX BUG-8: command lain punya guard ini, erase_bios tidak -> bisa spawn
    // `flashrom -c ""` dengan chip kosong.
    if chip.trim().is_empty() {
        return Err("Chip name empty - Detect chip first".to_string());
    }

    let result = std::thread::spawn(move || {
        run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-E"],
            &window,
            "Erasing",
        )
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    Ok(result)
}

#[tauri::command]
fn load_chip_db(app_handle: tauri::AppHandle) -> Result<String, String> {
    if let Ok(resource_path) = app_handle.path().resource_dir() {
        let chips_path = resource_path.join("chips.json");
        if chips_path.exists() {
            return fs::read_to_string(&chips_path)
                .map_err(|e| format!("Failed to read chips.json: {}", e));
        }
    }

    let fallback = dirs_fallback();
    if let Ok(content) = fs::read_to_string(&fallback) {
        return Ok(content);
    }

    Ok("{}".to_string())
}

fn dirs_fallback() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    format!("{}/proyek/CH341A-programer/chips.json", home)
}

#[tauri::command]
fn get_chip_info(chip: String, app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let db_json = load_chip_db(app_handle)?;
    let chips: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&db_json).map_err(|e| format!("Failed to parse chip db: {}", e))?;

    if let Some(info) = chips.get(&chip) {
        return Ok(serde_json::json!({
            "found": true,
            "name": chip,
            "manufacturer": info.get("manufacturer").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            "size_kb": info.get("size_kb").and_then(|v| v.as_u64()).unwrap_or(0),
            "voltage": info.get("voltage").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            "package": info.get("package").and_then(|v| v.as_str()).unwrap_or("Unknown"),
        }));
    }

    for (key, info) in &chips {
        let parts: Vec<&str> = key.split('/').collect();
        for part in &parts {
            if part.eq_ignore_ascii_case(&chip) || chip.contains(part) || part.contains(chip.as_str()) {
                return Ok(serde_json::json!({
                    "found": true,
                    "name": key,
                    "manufacturer": info.get("manufacturer").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "size_kb": info.get("size_kb").and_then(|v| v.as_u64()).unwrap_or(0),
                    "voltage": info.get("voltage").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                    "package": info.get("package").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                }));
            }
        }
    }

    Ok(serde_json::json!({
        "found": false,
        "name": chip,
        "manufacturer": "Unknown",
        "size_kb": 0,
        "voltage": "Unknown",
        "package": "Unknown",
    }))
}

// 1. Overwrite DMI value in binary at exact offset
#[tauri::command]
fn overwrite_dmi_value(
    mut data: Vec<u8>,
    offset: usize,
    new_value: String,
    old_value: String,
) -> Result<Vec<u8>, String> {
    // FIX BUG-11: panjang slot DULU ditebak dengan nge-scan byte printable maju
    // dari offset. Kalau field (mis. Windows Key 29 char) nempel langsung ke
    // signature tabel ACPI berikutnya ("SSDT") TANPA null/spasi pemisah, scan
    // "makan" tetangga -> saat padding, tetangga ketimpa 0x00 -> BIOS korup.
    // (Terbukti via test regresi win_key_edit_corrupts_next_acpi_table.)
    //
    // Sumber kebenaran panjang field = nilai lama yang sudah tampil di UI, BUKAN
    // hasil scan. Slot = tepat old_value.len() byte, tidak pernah lebih.
    let slot_len = old_value.len();
    if slot_len == 0 {
        return Err("Nilai lama kosong - tidak bisa menentukan batas field DMI.".to_string());
    }
    if offset >= data.len() || offset + slot_len > data.len() {
        return Err("Invalid offset position".to_string());
    }

    // Verifikasi offset masih valid: byte di data HARUS sama dengan nilai lama.
    // Nangkis offset basi (buffer berubah sejak extraction) yang bisa bikin
    // tulisan mendarat di lokasi salah.
    if &data[offset..offset + slot_len] != old_value.as_bytes() {
        return Err(
            "Offset field sudah tidak cocok dengan data buffer (buffer berubah?). \
             Detect/Load ulang lalu coba lagi."
                .to_string(),
        );
    }

    let bytes = new_value.into_bytes();

    // FIX BUG-3: value lebih panjang dari slot asli akan menimpa struktur tetangga.
    if bytes.len() > slot_len {
        return Err(format!(
            "Value terlalu panjang: {} byte, field asli cuma muat {} byte. \
             Menulis lebih panjang akan merusak struktur BIOS setelahnya.",
            bytes.len(),
            slot_len
        ));
    }

    // Tulis nilai baru; sisa slot di-null-pad. Dibatasi KETAT ke slot_len,
    // jadi mustahil menyentuh byte di luar field lama.
    for (idx, &b) in bytes.iter().enumerate() {
        data[offset + idx] = b;
    }
    for idx in bytes.len()..slot_len {
        data[offset + idx] = 0x00;
    }

    Ok(data)
}

#[tauri::command]
async fn blank_check_bios(chip: String, window: tauri::Window) -> Result<String, String> {
    if chip.trim().is_empty() {
        return Err("Chip name empty - Detect chip first".to_string());
    }

    let result = std::thread::spawn(move || {
        let output_path = unique_tmp("bios_blank_check");

        let result = run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-r", &output_path],
            &window,
            "Reading",
        );

        match result {
            Ok(_) => {
                let data = fs::read(&output_path);
                let _ = fs::remove_file(&output_path);
                match data {
                    Ok(bytes) => {
                        if bytes.is_empty() {
                            return Err("Read zero bytes from chip".to_string());
                        }
                        
                        // Check if all bytes are 0xFF (standard blank flash state)
                        // Also check for all 0x00 just in case for some rare chips, but 0xFF is standard
                        let is_blank = bytes.iter().all(|&b| b == 0xFF);
                        
                        if is_blank {
                            Ok("BLANK_OK".to_string())
                        } else {
                            // Find first offset that is not 0xFF
                            let non_ff_offset = bytes.iter().position(|&b| b != 0xFF).unwrap_or(0);
                            let non_ff_val = bytes[non_ff_offset];
                            Err(format!("NOT_BLANK: Data found at offset 0x{:08X} (Value: 0x{:02X})", non_ff_offset, non_ff_val))
                        }
                    }
                    Err(e) => Err(format!("Failed to read temp buffer: {}", e)),
                }
            }
            Err(e) => {
                let _ = fs::remove_file(&output_path);
                Err(e)
            }
        }
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            check_usb,
            detect_chip,
            read_bios,
            backup_bios,
            open_backup,
            extract_dmi_and_key,
            write_bios,
            verify_bios,
            erase_bios,
            get_chip_info,
            inject_dmi,
            compare_bios_diff,
            analyze_me_region,
            clean_me_region,
            overwrite_dmi_value,
            blank_check_bios,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[cfg(test)]
mod compare_tests {
    use super::*;

    #[test]
    fn identical_files() {
        let a = vec![0u8; 1024];
        let b = vec![0u8; 1024];
        let r = compare_bios_diff(a, b).unwrap();
        assert!(r.identical);
        assert!(r.size_match);
        assert_eq!(r.diff_count, 0);
        assert!(r.message.contains("IDENTIK"));
        assert_eq!(r.hash_a, r.hash_b);
    }

    #[test]
    fn different_one_byte() {
        let a = vec![0u8; 256];
        let mut b = vec![0u8; 256];
        b[100] = 0xFF;
        let r = compare_bios_diff(a, b).unwrap();
        assert!(!r.identical);
        assert!(r.size_match);
        assert_eq!(r.diff_count, 1);
        assert_eq!(r.first_offset, Some(100));
        assert_eq!(r.diff_offsets, vec![100]);
        assert!(r.message.contains("BEDA"));
    }

    #[test]
    fn size_mismatch() {
        let a = vec![1u8; 100];
        let b = vec![1u8; 200];
        let r = compare_bios_diff(a, b).unwrap();
        assert!(!r.identical);
        assert!(!r.size_match);
        assert!(r.message.contains("Size beda"));
        assert!(r.diff_offsets.is_empty());
    }

    #[test]
    fn empty_errors() {
        let r = compare_bios_diff(vec![], vec![1, 2, 3]);
        assert!(r.is_err());
    }

    #[test]
    fn sample_cap() {
        let a = vec![0u8; 5000];
        let b = vec![1u8; 5000];
        let r = compare_bios_diff(a, b).unwrap();
        assert_eq!(r.diff_count, 5000);
        assert_eq!(r.diff_offsets.len(), 1000);
        assert!(r.sample_capped);
        assert_eq!(r.first_offset, Some(0));
    }

    #[test]
    fn test_overwrite_shorter_with_null_padding() {
        let data = vec![65, 66, 67, 68, 0, 17, 34]; // ABCD 
        let r = overwrite_dmi_value(data, 0, "XY".to_string(), "ABCD".to_string()).unwrap();
        assert_eq!(r, vec![88, 89, 0, 0, 0, 17, 34]); // XY   
    }

    #[test]
    fn test_overwrite_selalu_null_pad() {
        // Kontrak baru: padding SELALU 0x00 (standar DMI string terminator),
        // tidak lagi menebak spasi. old_value "ABCD" (4 slot), "XYZ" -> 1 null pad.
        let data = vec![65, 66, 67, 68, 17]; // ABCD + tetangga 0x11
        let r = overwrite_dmi_value(data, 0, "XYZ".to_string(), "ABCD".to_string()).unwrap();
        assert_eq!(r, vec![88, 89, 90, 0, 17]); // XYZ\0 + 0x11 utuh
    }

    #[test]
    fn test_overwrite_pas_slot_diterima_tetangga_utuh() {
        // old_value "AB" (2 slot). "XY" PAS muat -> boleh, tetangga 0x11 tak tersentuh.
        let data = vec![65, 66, 0, 17];
        let r = overwrite_dmi_value(data, 0, "XY".to_string(), "AB".to_string()).unwrap();
        assert_eq!(r, vec![88, 89, 0, 17]);
        assert_eq!(r[3], 17, "byte tetangga wajib utuh");
    }

    #[test]
    fn test_overwrite_lebih_panjang_dari_slot_ditolak() {
        // REGRESI BUG-3: slot 2 byte, value 5 byte -> harus DITOLAK.
        let data = vec![65, 66, 0, 17];
        let r = overwrite_dmi_value(data, 0, "XYZAB".to_string(), "AB".to_string());
        assert!(r.is_err(), "value melebihi slot harus ditolak");
        assert!(r.unwrap_err().contains("terlalu panjang"));
    }

    #[test]
    fn test_overwrite_tidak_rusak_tetangga() {
        // REGRESI BUG-3: struktur BIOS setelah field tidak boleh tersentuh.
        let data = vec![b'A', b'B', b'C', 0x00, 0xDE, 0xAD, 0xBE, 0xEF];
        let r = overwrite_dmi_value(data, 0, "ABCDEFGH".to_string(), "ABC".to_string());
        assert!(r.is_err(), "harus ditolak, bukan menimpa 0xDEADBEEF");
    }

    #[test]
    fn test_overwrite_pas_panjang_field_diterima() {
        // old_value "ABC" (3 slot). "XYZ" pas -> boleh, tetangga utuh.
        let data = vec![b'A', b'B', b'C', 0xDE, 0xAD];
        let r = overwrite_dmi_value(data, 0, "XYZ".to_string(), "ABC".to_string()).unwrap();
        assert_eq!(&r[0..3], b"XYZ");
        assert_eq!(&r[3..], &[0xDE, 0xAD], "byte tetangga wajib utuh");
    }

    #[test]
    fn test_overwrite_bug11_field_nempel_tabel_acpi() {
        // REGRESI BUG-11: Windows Key (29 char) nempel LANGSUNG ke signature tabel
        // ACPI berikutnya ("SSDT") tanpa null/spasi pemisah. Cara lama nge-scan
        // printable -> "makan" SSDT -> pad menimpanya jadi 0x00 -> ACPI korup.
        // Sekarang slot dibatasi tepat old_value.len() -> SSDT wajib utuh.
        let mut data = vec![0x11u8; 40];
        let old = b"VK7JG-NPHTM-C97JM-9MPGT-3V66T"; // 29 char
        data[0..29].copy_from_slice(old);
        data[29..33].copy_from_slice(b"SSDT"); // tabel tetangga, NEMPEL
        let newkey = "NEWKY-NPHTM-C97JM-9MPGT-3V66T".to_string(); // 29 char, pas slot
        let r = overwrite_dmi_value(
            data,
            0,
            newkey,
            String::from_utf8(old.to_vec()).unwrap(),
        )
        .unwrap();
        assert_eq!(&r[0..29], b"NEWKY-NPHTM-C97JM-9MPGT-3V66T", "key baru wajib tertulis");
        assert_eq!(&r[29..33], b"SSDT", "tabel ACPI tetangga WAJIB utuh (BUG-11)");
    }

    #[test]
    fn test_overwrite_offset_basi_ditolak() {
        // Byte di offset TIDAK cocok dengan old_value (buffer berubah) -> tolak.
        let data = vec![b'Z', b'Z', b'Z', 0x00];
        let r = overwrite_dmi_value(data, 0, "XY".to_string(), "AB".to_string());
        assert!(r.is_err(), "offset basi harus ditolak");
        assert!(r.unwrap_err().contains("tidak cocok"));
    }

    #[test]
    fn test_inject_dmi_tolak_signature_sampah() {
        // REGRESI BUG-2: string "DMI EDIT TOOL" TIDAK boleh dianggap anchor DMI.
        let mut old = vec![0x00u8; 0x40000];
        let junk = b"AMI BIOS SETUP UTILITY - DMI EDIT TOOL v2.1";
        old[0x2000..0x2000 + junk.len()].copy_from_slice(junk);
        let new = vec![0x11u8; 0x40000];

        let r = inject_dmi(old, new.clone());
        assert!(
            r.is_err(),
            "signature sampah 'DMI' harus ditolak, bukan copy 64KB buta"
        );
        assert_eq!(r.unwrap_err().code, "ERR_DMI_SIGNATURE_NOT_FOUND_0x202");
    }

    #[test]
    fn test_inject_dmi_terima_msdm_valid() {
        // MSDM dengan length field valid -> anchor diterima, blok 64KB disalin.
        let mut old = vec![0x00u8; 0x40000];
        let pos = 0x10000;
        old[pos..pos + 4].copy_from_slice(b"MSDM");
        old[pos + 4..pos + 8].copy_from_slice(&0x55u32.to_le_bytes());
        old[pos + 9] = 0xB3; // checksum biner (bikin from_utf8 gagal)
        old[pos + 20] = 0xAB; // penanda unik

        let new = vec![0x11u8; 0x40000];
        let out = inject_dmi(old, new).expect("MSDM valid harus diterima");
        assert_eq!(out[pos + 20], 0xAB, "blok DMI lama wajib tersalin");
    }

    #[test]
    fn test_extract_key_dari_msdm_biner() {
        // REGRESI BUG-1: MSDM biner bikin String::from_utf8 gagal -> key hilang.
        let mut bios = vec![0xFFu8; 8192];
        let pos = 1000;
        bios[pos..pos + 4].copy_from_slice(b"MSDM");
        bios[pos + 4..pos + 8].copy_from_slice(&0x55u32.to_le_bytes());
        bios[pos + 9] = 0xB3; // byte non-UTF8
        let key = b"VK7JG-NPHTM-C97JM-9MPGT-3V66T";
        bios[pos + 56..pos + 56 + key.len()].copy_from_slice(key);

        let info = extract_dmi_and_key(bios);
        assert_eq!(
            info["windows_key"].as_str().unwrap(),
            "VK7JG-NPHTM-C97JM-9MPGT-3V66T",
            "key wajib terbaca walau MSDM mengandung byte biner"
        );
    }

    #[test]
    fn test_key_offset_akurat_walau_byte_biner() {
        // REGRESI BUG-9: from_utf8_lossy bikin byte invalid jadi U+FFFD (3 byte),
        // sehingga mat.start() SKEW +3 dari posisi byte asli. Edit key via UI lalu
        // menulis di offset skew -> MSDM korup. ascii_map_1to1 harus akurat 1:1.
        let mut bios = vec![0xFFu8; 8192];
        let pos = 1000;
        bios[pos..pos + 4].copy_from_slice(b"MSDM");
        bios[pos + 4..pos + 8].copy_from_slice(&0x55u32.to_le_bytes());
        // beberapa byte biner (>0x7F) SEBELUM key -> pemicu skew lossy
        bios[pos + 9] = 0xB3;
        bios[pos + 10] = 0xE4;
        bios[pos + 11] = 0x91;
        let key = b"VK7JG-NPHTM-C97JM-9MPGT-3V66T";
        let true_off = pos + 56;
        bios[true_off..true_off + key.len()].copy_from_slice(key);

        let info = extract_dmi_and_key(bios.clone());
        let reported = info["windows_key_offset"].as_u64().unwrap() as usize;
        assert_eq!(
            reported, true_off,
            "offset harus == posisi byte asli (bukan skew lossy)"
        );
        // bukti kuat: byte di offset yang dilaporkan HARUS awal key sungguhan
        assert_eq!(&bios[reported..reported + 5], b"VK7JG",
            "offset yang dilaporkan harus menunjuk awal key sebenarnya");
    }

    #[test]
    fn test_find_offset_ambigu_ditolak() {
        // REGRESI FIX #2: nilai field yang muncul >1x di buffer = ambigu.
        // Offset harus 0 (UI blokir edit) supaya tidak nulis ke lokasi tebakan salah.
        // Dell service tag "ABC1234" ditaruh 2x: sekali "sampah" duluan, sekali di DMI.
        let mut bios = vec![0xFFu8; 8192];
        // brand marker Dell (huruf besar dicek via to_uppercase di extractor)
        let dell = b"DELL INC.";
        bios[100..100 + dell.len()].copy_from_slice(dell);
        let tag = b"ABC1234";
        // kemunculan 1 (duluan, region acak) + kemunculan 2 (belakangan)
        bios[500..500 + tag.len()].copy_from_slice(tag);
        bios[3000..3000 + tag.len()].copy_from_slice(tag);
        // pemisah non-alnum supaya \b regex mengenali sebagai token utuh
        bios[499] = 0x20; bios[507] = 0x20;
        bios[2999] = 0x20; bios[3007] = 0x20;

        let info = extract_dmi_and_key(bios);
        // tag boleh saja ke-parse, tapi offset WAJIB 0 karena ambigu -> UI tolak edit
        assert_eq!(
            info["service_tag_offset"].as_u64().unwrap(),
            0,
            "nilai ambigu (muncul 2x) harus offset 0, bukan nebak lokasi -> cegah brick"
        );
    }

    #[test]
    fn test_find_offset_unik_akurat() {
        // FIX #2: nilai yang muncul TEPAT 1x harus mengembalikan offset yang benar,
        // dan byte di offset itu harus persis nilainya (bukan lokasi lain).
        let mut bios = vec![0xFFu8; 8192];
        let dell = b"DELL INC.";
        bios[100..100 + dell.len()].copy_from_slice(dell);
        let tag = b"XYZ9876";
        let off = 2500;
        bios[off - 1] = 0x20;
        bios[off..off + tag.len()].copy_from_slice(tag);
        bios[off + tag.len()] = 0x20;

        let info = extract_dmi_and_key(bios.clone());
        let reported = info["service_tag_offset"].as_u64().unwrap() as usize;
        assert_eq!(reported, off, "nilai unik harus mengembalikan offset sebenarnya");
        assert_eq!(&bios[reported..reported + tag.len()], tag,
            "byte di offset yang dilaporkan harus == service tag");
    }

    #[test]
    fn test_ascii_map_panjang_1to1() {
        // ascii_map_1to1 wajib menjaga panjang == jumlah byte input (kunci akurasi offset)
        let data = vec![0x41, 0xB3, 0x00, 0x7E, 0xFF, 0x20];
        let s = ascii_map_1to1(&data);
        assert_eq!(s.len(), data.len(), "panjang string == jumlah byte");
        assert_eq!(s, "A..~. ");
    }
}
