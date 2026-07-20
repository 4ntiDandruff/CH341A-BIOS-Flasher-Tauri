use md5::{Digest, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tauri::{Emitter, Manager};

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
    // flashrom outputs lines like "Reading flash... 50% complete."
    // or "Verifying flash... 100% complete."
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
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Some(pct) = parse_progress(&line) {
                    emit_progress(&win, pct, &stage_owned);
                }
                emit_log(&win, &line);
                lines.push(line);
            }
        }
        lines.join("\n")
    });

    let stdout = child
        .stdout
        .take()
        .map(|s| {
            let r = BufReader::new(s);
            r.lines().filter_map(|l| l.ok()).collect::<Vec<_>>().join("\n")
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
        // Don't treat non-zero exit as hard error for detect — flashrom exits 1 when it finds chips
        Err(msg)
    }
}

#[tauri::command]
fn check_usb() -> bool {
    // Check if CH341A (1a86:5512) is connected via lsusb
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

    // Parse chip name from flashrom output
    // Patterns: "Found ... chip \"CHIPNAME\"" or "chip "CHIPNAME" ... detected"
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

    // Also try to find chips listed in "Multiple flash chip definitions" lines
    if chips.is_empty() {
        for cap in re_multiple.captures_iter(&combined) {
            if let Some(name) = cap.get(1) {
                chips.push(name.as_str().to_string());
            }
        }
    }

    // Try another pattern: lines with chip names in quotes
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
fn extract_dmi_and_key(data: Vec<u8>) -> serde_json::Value {
    let mut win_key = "Not Found".to_string();
    
    // 1. Extract Windows Key from MSDM table
    // MSDM table signature is "MSDM" (0x4D, 0x53, 0x44, 0x4D)
    // Followed by table headers and the product key at the end (29 bytes or 25 chars)
    if let Some(pos) = data.windows(4).position(|w| w == b"MSDM") {
        // Table is typically small. Key is a 29-byte alfanumeric string (like XXXXX-XXXXX-XXXXX-XXXXX-XXXXX)
        // Let's search inside a window of 100 bytes from MSDM signature
        let start_search = pos;
        let end_search = std::cmp::min(data.len(), pos + 120);
        let segment = &data[start_search..end_search];
        
        // Find Windows Key pattern using Regex in segment
        if let Ok(text) = String::from_utf8(segment.to_vec()) {
            let re_key = Regex::new(r"[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}").unwrap();
            if let Some(mat) = re_key.find(&text) {
                win_key = mat.as_str().to_string();
            }
        }
    }

    // 2. Extract Serial Number & Board ID (DMI)
    let mut serial_num = "Not Found".to_string();
    let mut board_id = "Not Found".to_string();

    // Check ASCII strings in the bios
    if let Ok(text) = String::from_utf8(data.iter().map(|&b| if b.is_ascii() && b >= 0x20 && b <= 0x7E { b } else { b' ' }).collect()) {
        // Scan for HP Board ID (BID)
        let re_bid = Regex::new(r"(?i)BID\s*=\s*([A-Za-z0-9_#]+)").unwrap();
        if let Some(cap) = re_bid.captures(&text) {
            board_id = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        }

        // Scan for standard Serial Number / S/N patterns
        let re_sn = Regex::new(r"(?i)(?:serial\s*number|s/n|system\s*serial)\s*:?\s*([A-Z0-9]{8,20})").unwrap();
        if let Some(cap) = re_sn.captures(&text) {
            serial_num = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        }
    }

    serde_json::json!({
        "windows_key": win_key,
        "serial_number": serial_num,
        "board_id": board_id
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
    // Try resource dir first, then fallback path
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

    // Return empty array if no chip db found
    Ok("[]".to_string())
}

fn dirs_fallback() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    format!("{}/proyek/CH341A-programer/chips.json", home)
}

#[tauri::command]
fn get_chip_info(chip: String, app_handle: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let db_json = load_chip_db(app_handle)?;
    // chips.json is a Map: { "chipName": { manufacturer, size_kb, voltage, package }, ... }
    let chips: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&db_json).map_err(|e| format!("Failed to parse chip db: {}", e))?;

    // Try exact match first
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

    // Try partial match (chip name might be part of a key like "W25Q64BV/W25Q64CV/W25Q64FV")
    for (key, info) in &chips {
        let parts: Vec<&str> = key.split('/').collect();
        for part in &parts {
            if part.eq_ignore_ascii_case(&chip) || chip.contains(part) || part.contains(&chip.as_str()) {
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
            load_chip_db,
            get_chip_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
