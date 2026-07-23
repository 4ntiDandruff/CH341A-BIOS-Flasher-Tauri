use md5::{Digest, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tauri::{Emitter, Manager};

#[derive(Serialize, Deserialize, Clone)]
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

    let stdout = child
        .stdout
        .take()
        .map(|s| {
            let r = BufReader::new(s);
            r.lines().map_while(Result::ok).collect::<Vec<_>>().join("\n")
        })
        .unwrap_or_default();

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for flashrom: {}", e))?;

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
        let output_path = "/tmp/bios_read_buffer.bin";

        let result = run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-r", output_path],
            &window,
            "Reading",
        );

        match result {
            Ok(_) => {
                fs::read(output_path).map_err(|e| format!("Failed to read output file: {}", e))
            }
            Err(e) => Err(e),
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
    let mut dmi_offset: Option<usize> = None;
    if let Some(pos) = data_old.windows(4).position(|w| w == b"MSDM") {
        dmi_offset = Some((pos / 0x1000) * 0x1000);
    }
    
    if dmi_offset.is_none() {
        if let Some(pos) = data_old.windows(3).position(|w| w == b"NCB" || w == b"DMI") {
            dmi_offset = Some((pos / 0x1000) * 0x1000);
        }
    }

    if let Some(offset) = dmi_offset {
        let block_size = 0x10000;
        if offset + block_size <= data_old.len() && offset + block_size <= output_data.len() {
            output_data[offset..offset + block_size].copy_from_slice(&data_old[offset..offset + block_size]);
            return Ok(output_data);
        }
    }

    if let Some(old_msdm_pos) = data_old.windows(4).position(|w| w == b"MSDM") {
        if let Some(new_msdm_pos) = output_data.windows(4).position(|w| w == b"MSDM") {
            let old_key_segment = &data_old[old_msdm_pos..std::cmp::min(data_old.len(), old_msdm_pos + 120)];
            let new_key_segment = &output_data[new_msdm_pos..std::cmp::min(output_data.len(), new_msdm_pos + 120)];
            
            if let Ok(old_text) = String::from_utf8(old_key_segment.to_vec()) {
                let re_key = Regex::new(r"[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}").unwrap();
                if let Some(old_mat) = re_key.find(&old_text) {
                    let old_key_str = old_mat.as_str();
                    
                    if let Ok(new_text) = String::from_utf8(new_key_segment.to_vec()) {
                        if let Some(new_mat) = re_key.find(&new_text) {
                            let start_replace = new_msdm_pos + new_mat.start();
                            let end_replace = new_msdm_pos + new_mat.end();
                            output_data[start_replace..end_replace].copy_from_slice(old_key_str.as_bytes());
                            return Ok(output_data);
                        }
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
fn clean_me_region(data: Vec<u8>) -> Result<Vec<u8>, DiagnosticError> {
    if data.is_empty() {
        return Err(diagnose_err!(
            "ERR_ME_EMPTY_BUFFER_0x301",
            "BIOS buffer is empty",
            "data_len: 0"
        ));
    }

    let mut output_data = data.clone();
    if let Some(pos) = output_data.windows(4).position(|w| w == b"$FPT") {
        let state_offset = pos + 16;
        if state_offset < output_data.len() {
            output_data[state_offset] = 0xFF;
            return Ok(output_data);
        }
    }

    Err(diagnose_err!(
        "ERR_ME_FPT_HEADER_NOT_FOUND_0x302",
        "Could not locate Intel ME $FPT header in buffer to clean",
        format!("buffer_len: {}", data.len())
    ))
}

#[tauri::command]
fn extract_dmi_and_key(data: Vec<u8>) -> serde_json::Value {
    let mut win_key = "Not Found".to_string();
    let mut brand = "Unknown".to_string();
    let mut model = "Unknown".to_string();
    let mut serial_num = "Not Found".to_string();
    let mut board_id = "Not Found".to_string();
    let mut service_tag = "Not Found".to_string();

    // 1. Extract Windows Key from MSDM
    if let Some(pos) = data.windows(4).position(|w| w == b"MSDM") {
        let start = pos;
        let end = std::cmp::min(data.len(), pos + 120);
        let segment = &data[start..end];
        if let Ok(text) = String::from_utf8(segment.to_vec()) {
            let re_key = Regex::new(r"[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}").unwrap();
            if let Some(mat) = re_key.find(&text) {
                win_key = mat.as_str().to_string();
            }
        }
    }

    // Convert printable ASCII chars, ignoring wide spaces/nulls
    let mut ascii_chars = Vec::new();
    for &b in &data {
        if b.is_ascii() && (0x20..=0x7E).contains(&b) {
            ascii_chars.push(b as char);
        } else if (b == 0x00 || b == 0x0A || b == 0x0D) && ascii_chars.last() != Some(&' ') {
            ascii_chars.push(' ');
        }
    }
    
    let raw_text: String = ascii_chars.into_iter().collect();
    
    // Compress multiple spaces into one single space
    let re_spaces = Regex::new(r"\s+").unwrap();
    let clean_text = re_spaces.replace_all(&raw_text, " ");
    let upper_text = clean_text.to_uppercase();

    // 2. Identify Brand
    if upper_text.contains("LENOVO") {
        brand = "Lenovo".to_string();
    } else if upper_text.contains("ASUSTEK") || upper_text.contains("ASUS") {
        brand = "ASUS".to_string();
    } else if upper_text.contains("HEWLETT-PACKARD") || upper_text.contains("HP ") || upper_text.contains("HP.") {
        brand = "HP".to_string();
    } else if upper_text.contains("DELL") {
        brand = "Dell".to_string();
    } else if upper_text.contains("ACER") {
        brand = "Acer".to_string();
    } else if upper_text.contains("TOSHIBA") {
        brand = "Toshiba".to_string();
    } else if upper_text.contains("GIGABYTE") {
        brand = "Gigabyte".to_string();
    } else if upper_text.contains("MSI") {
        brand = "MSI".to_string();
    }

    // 3. Extract Board ID / BID (HP)
    let re_bid = Regex::new(r"(?i)BID([0-9A-F]{4,6})").unwrap();
    if let Some(cap) = re_bid.captures(&clean_text) {
        board_id = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
    }

    // 4. Extract Dell Service Tag (exactly 7 alphanumeric chars, validated with DELL signature)
    if brand == "Dell" {
        let re_svctag = Regex::new(r"\b([A-Z0-9]{7})\b").unwrap();
        for cap in re_svctag.captures_iter(&clean_text) {
            let tag = cap.get(1).unwrap().as_str().to_string();
            if !tag.contains("SERVICE") && !tag.contains("VERSION") {
                service_tag = tag;
                break;
            }
        }
    }

    // 5. Extract Serial Number
    let re_sn = Regex::new(r"(?i)(?:serial\s*number|s/n|system\s*serial|serial\s*no|prodn)\s*[:=]?\s*([A-Z0-9]{8,20})").unwrap();
    if let Some(cap) = re_sn.captures(&clean_text) {
        serial_num = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
    }

    // HP Specific Serial Number Fallback
    if serial_num == "Not Found" && brand == "HP" {
        let re_hp_sn = Regex::new(r"\b(5CG|5CD|CND|CNU|5CB)[A-Z0-9]{7}\b").unwrap();
        if let Some(cap) = re_hp_sn.find(&clean_text) {
            serial_num = cap.as_str().to_string();
        }
    }

    // 6. Extract Model Name based on Brand
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
        "serial_number": serial_num,
        "board_id": board_id,
        "service_tag": service_tag
    })
}

#[tauri::command]
async fn write_bios(chip: String, data: Vec<u8>, window: tauri::Window) -> Result<String, String> {
    let buffer_path = "/tmp/bios_write_buffer.bin";
    fs::write(buffer_path, &data).map_err(|e| format!("Failed to write buffer: {}", e))?;

    let result = std::thread::spawn(move || {
        run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-w", buffer_path],
            &window,
            "Writing",
        )
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
    let buffer_path = "/tmp/bios_verify_buffer.bin";
    fs::write(buffer_path, &data).map_err(|e| format!("Failed to write buffer: {}", e))?;

    let result = std::thread::spawn(move || {
        run_flashrom_with_progress(
            &["-p", "ch341a_spi", "-c", &chip, "-v", buffer_path],
            &window,
            "Verifying",
        )
    })
    .join()
    .map_err(|_| "Thread panicked".to_string())??;

    if result.contains("VERIFIED") {
        Ok("VERIFIED".to_string())
    } else {
        Ok(format!("Verification result: {}", result.lines().last().unwrap_or("unknown")))
    }
}

#[tauri::command]
async fn erase_bios(chip: String, window: tauri::Window) -> Result<String, String> {
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

    Ok("[]".to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
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
}
