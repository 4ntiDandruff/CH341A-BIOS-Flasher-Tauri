fn main() {
    let data = std::fs::read("/tmp/bios_read_buffer.bin").expect("Failed to read test buffer");
    println!("Read buffer size: {} bytes", data.len());

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
            let re_key = regex::Regex::new(r"[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}-[A-Z0-9]{5}").unwrap();
            if let Some(mat) = re_key.find(&text) {
                win_key = mat.as_str().to_string();
            }
        }
    }

    // Convert printable ASCII chars, ignoring wide spaces/nulls
    let mut ascii_chars = Vec::new();
    for &b in &data {
        if b.is_ascii() && b >= 0x20 && b <= 0x7E {
            ascii_chars.push(b as char);
        } else if b == 0x00 || b == 0x0A || b == 0x0D {
            if ascii_chars.last() != Some(&' ') {
                ascii_chars.push(' ');
            }
        }
    }
    
    let raw_text: String = ascii_chars.into_iter().collect();
    
    // Compress multiple spaces into one single space
    let re_spaces = regex::Regex::new(r"\s+").unwrap();
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
    }

    // 3. Extract Board ID / BID (HP)
    // HP Board ID is typically $BID080C7 or BID080C7
    let re_bid = regex::Regex::new(r"(?i)BID([0-9A-F]{4,6})").unwrap();
    if let Some(cap) = re_bid.captures(&clean_text) {
        board_id = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
    }

    // 4. Extract Dell Service Tag (exactly 7 alphanumeric chars, validated with DELL signature)
    if brand == "Dell" {
        let re_svctag = regex::Regex::new(r"\b([A-Z0-9]{7})\b").unwrap();
        for cap in re_svctag.captures_iter(&clean_text) {
            let tag = cap.get(1).unwrap().as_str().to_string();
            // Simple validation: should not be common words
            if !tag.contains("SERVICE") && !tag.contains("VERSION") {
                service_tag = tag;
                break;
            }
        }
    }

    // 5. Extract Serial Number
    // Standard SN prefix check
    let re_sn = regex::Regex::new(r"(?i)(?:serial\s*number|s/n|system\s*serial|serial\s*no|prodn)\s*[:=]?\s*([A-Z0-9]{8,20})").unwrap();
    if let Some(cap) = re_sn.captures(&clean_text) {
        serial_num = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
    }

    // HP Specific Serial Number Fallback (Scan for HP patterns: 10 chars starting with 5CG, 5CD, CND, CNU, 5CB)
    if serial_num == "Not Found" && brand == "HP" {
        let re_hp_sn = regex::Regex::new(r"\b(5CG|5CD|CND|CNU|5CB)[A-Z0-9]{7}\b").unwrap();
        if let Some(cap) = re_hp_sn.find(&clean_text) {
            serial_num = cap.as_str().to_string();
        }
    }

    // 6. Extract Model Name based on Brand
    if brand == "Lenovo" {
        let re_lenovo = regex::Regex::new(r"(?i)(ThinkPad|IdeaPad|Yoga)\s+[A-Z0-9]{2,10}(?:\s+[A-Z0-9]{2,10})?").unwrap();
        if let Some(cap) = re_lenovo.find(&clean_text) {
            model = cap.as_str().trim().to_string();
        }
    } else if brand == "ASUS" {
        let re_asus = regex::Regex::new(r"\b(X\d{3}[A-Z]{1,2}|UX\d{3}[A-Z]{0,2}|A\d{3}[A-Z]{1,2}|K\d{3}[A-Z]{1,2}|GL\d{3}[A-Z]{0,2})\b").unwrap();
        if let Some(cap) = re_asus.find(&clean_text) {
            model = cap.as_str().to_string();
        }
    } else if brand == "HP" {
        let re_hp = regex::Regex::new(r"(?i)(?:ProBook|EliteBook|Pavilion|Spectre|Envy|HP\s+Notebook)\s+\d{3,4}(?:\s+G\d)?").unwrap();
        if let Some(cap) = re_hp.find(&clean_text) {
            model = cap.as_str().trim().to_string();
        }
    } else if brand == "Dell" {
        let re_dell = regex::Regex::new(r"(?i)(?:Latitude|Inspiron|Vostro|Precision|OptiPlex|XPS)\s+\d{4}").unwrap();
        if let Some(cap) = re_dell.find(&clean_text) {
            model = cap.as_str().trim().to_string();
        }
    }

    println!("Brand: {}", brand);
    println!("Model: {}", model);
    println!("Windows Key: {}", win_key);
    println!("Serial: {}", serial_num);
    println!("BID: {}", board_id);
    println!("SvcTag: {}", service_tag);
}
