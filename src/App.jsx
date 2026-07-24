import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save, message } from "@tauri-apps/plugin-dialog";

const MENU_ITEMS = [
  { id: 1, icon: "🔍", label: "Detect Chip", direct: true },
  { id: 2, icon: "📖", label: "Read", direct: false },
  { id: 3, icon: "💾", label: "Backup", direct: true },
  { id: 4, icon: "📂", label: "Load File", direct: true },
  { id: 7, icon: "🗑️", label: "Erase", direct: false },
  { id: 5, icon: "✍️", label: "Write", direct: false },
  { id: 6, icon: "✅", label: "Verify", direct: false },
  { id: 8, icon: "💉", label: "DMI Injector", direct: true },
];

const APP_VERSION = "2.1.7";

const INDO_CHANGELOG = [
  {
    version: "v2.1.7",
    date: "2026-07-24",
    items: [
      "Perbaikan visual kartu DMI: grid layout proporsional, alignment teks kiri-kanan rapi, space ikon seragam."
    ]
  },
  {
    version: "v2.1.6",
    date: "2026-07-23",
    items: [
      "Audit flash path: temp file unik + cleanup, validasi buffer kosong, verify lebih ketat.",
      "Chip database diperluas (Boya/Fudan/Puya/Atmel/dll) + fix voltage W25Q64JV 3.3V.",
      "chips.json di-bundle ke release agar get_chip_info tetap jalan di luar folder project."
    ]
  },
  {
    version: "v2.1.5",
    date: "2026-07-23",
    items: [
      "Rename menu Open Backup menjadi Load File agar lebih jelas."
    ]
  },
  {
    version: "v2.1.4",
    date: "2026-07-23",
    items: [
      "Compare (Diff) poles: laporan IDENTIK/BEDA + MD5 + jumlah byte beda + offset pertama.",
      "Pesan jelas jika buffer kosong atau size file beda; hex *XX* tetap untuk detail."
    ]
  },
  {
    version: "v2.1.3",
    date: "2026-07-23",
    items: [
      "Pre-flight Gate: cek USB + chip + buffer + konfirmasi adapter 1.8V sebelum Write/Erase/Instant Mode.",
      "Udev rule CH341A permanen: flashrom non-sudo (MODE 0666, group plugdev).",
      "Version sync: package/Cargo/tauri/desktop/About diseragamkan ke 2.1.3."
    ]
  },
  {
    version: "v2.1.2",
    date: "2026-07-21",
    items: [
      "💉 Fitur DMI Injector: Pemindahan otomatis lisensi Windows Key & Serial dari BIOS rusak ke Clean BIOS.",
      "🧹 Intel ME Cleaner: Pembersihan ME region untuk mengatasi problem laptop late display / mati setelah 30 menit.",
      "📊 Side-by-Side Hex Diff: Pembanding visual byte-per-byte langsung ditandai bintang merah (*XX*) pada Hex Viewer.",
      "🚨 Diagnostik Error Terstruktur (BSOD Red Modal): Laporan crash mendalam & tombol ekspor file log (.log)."
    ]
  },
  {
    version: "v2.1.0",
    date: "2026-07-21",
    items: [
      "📟 Fitur DMI & License Auto-Extractor: Ekstraksi Windows Product Key (MSDM), Serial Number, HP BID, & Dell Service Tag.",
      "📋 Salin Clipboard Instan: Tombol copy cepat di setiap field DMI.",
      "🔍 Hex Search: Input pencarian teks ASCII & Hex string langsung di atas viewer.",
      "⚡ Instant Mode: Erase → Write → Verify otomatis dalam sekali jalan."
    ]
  },
  {
    version: "v2.0.0",
    date: "2026-07-19",
    items: [
      "🎨 Migrasi GUI Python GTK3 ke Tauri (React + Tailwind CSS + DaisyUI).",
      "🖥️ Tampilan Modern Split Layout (Menu 40% | Hex Viewer 60%) dan Dark Mode bawaan."
    ]
  }
];

function formatDuration(ms) {
  const totalSecs = Math.floor(ms / 1000);
  const mins = Math.floor(totalSecs / 60).toString().padStart(2, "0");
  const secs = (totalSecs % 60).toString().padStart(2, "0");
  return `${mins}.${secs}`;
}

function playSound(type) {
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    if (type === 'success') {
      const playBeep = (delay) => {
        setTimeout(() => {
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();
          osc.connect(gain);
          gain.connect(ctx.destination);
          osc.type = 'sine';
          osc.frequency.setValueAtTime(2500, ctx.currentTime);
          gain.gain.setValueAtTime(0.3, ctx.currentTime);
          osc.start();
          gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.15);
          osc.stop(ctx.currentTime + 0.18);
        }, delay);
      };
      playBeep(0);
      playBeep(250);
    } else {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.type = 'sawtooth';
      osc.frequency.setValueAtTime(130, ctx.currentTime);
      gain.gain.setValueAtTime(0.4, ctx.currentTime);
      osc.start();
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.55);
      osc.stop(ctx.currentTime + 0.6);
    }
  } catch (e) {
    console.error("Audio notification error:", e);
  }
}

function formatHex(bytes, highlightOffset = -1, searchLen = 0, diffOffsets = []) {
  if (!bytes || bytes.length === 0) return "No data loaded";
  const totalSize = bytes.length;
  const totalKB = (totalSize / 1024).toFixed(0);

  let nonFFcount = 0;
  let firstDataOffset = -1;
  for (let i = 0; i < totalSize; i++) {
    if (bytes[i] !== 0xFF) {
      nonFFcount++;
      if (firstDataOffset === -1) firstDataOffset = i;
      if (nonFFcount > 100) break;
    }
  }
  const isEmpty = nonFFcount === 0;

  const lines = [];
  lines.push(`Total: ${totalKB} KB (${totalSize.toLocaleString()} bytes)`);
  if (isEmpty) {
    lines.push("⚠️  CHIP IS EMPTY — All bytes are 0xFF");
  } else {
    lines.push(`✅ Chip has data | First data at 0x${firstDataOffset.toString(16).toUpperCase().padStart(8, "0")}`);
  }
  if (diffOffsets.length > 0) {
    lines.push(`🔴 Diff Mode Active: ${diffOffsets.length} byte differences detected (marked with *XX*)`);
  }
  lines.push("");
  lines.push("Offset    00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F  |ASCII|");
  lines.push("─".repeat(79));

  let startOffset = 0;
  if (highlightOffset !== -1) {
    startOffset = Math.max(0, Math.floor(highlightOffset / 16) * 16 - 256);
  } else if (!isEmpty && firstDataOffset > 65536) {
    startOffset = Math.floor(firstDataOffset / 16) * 16;
  }
  
  if (startOffset > 0) {
    lines.push(`... skipping ${(startOffset/1024).toFixed(0)}KB of 0xFF ...`);
    lines.push("");
  }

  const showBytes = 32768;
  const endOffset = Math.min(totalSize, startOffset + showBytes);

  for (let i = startOffset; i < endOffset; i += 16) {
    const offset = i.toString(16).toUpperCase().padStart(8, "0");
    const hexParts = [];
    let ascii = "";
    
    for (let j = 0; j < 16; j++) {
      const idx = i + j;
      if (idx < endOffset) {
        const b = bytes[idx];
        const hexStr = b.toString(16).toUpperCase().padStart(2, "0");
        
        const isMatch = highlightOffset !== -1 && idx >= highlightOffset && idx < (highlightOffset + searchLen);
        const isDiff = diffOffsets.includes(idx);
        
        if (isDiff) {
          hexParts.push(`*${hexStr}*`);
        } else if (isMatch) {
          hexParts.push(`>${hexStr}<`);
        } else {
          hexParts.push(` ${hexStr} `);
        }
        
        ascii += b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : ".";
      } else {
        hexParts.push("    ");
        ascii += " ";
      }
    }
    lines.push(`${offset}  ${hexParts.join("")} |${ascii}|`);
  }

  if (endOffset < totalSize) {
    lines.push("");
    lines.push(`... showing 32KB from offset 0x${startOffset.toString(16).toUpperCase()} of ${totalKB}KB total`);
  }
  return lines.join("\n");
}

export default function App() {
  const [chip, setChip] = useState("");
  const [chipInfo, setChipInfo] = useState(null);
  const [buffer, setBuffer] = useState(null);
  const [fileName, setFileName] = useState("");
  const [activeMenu, setActiveMenu] = useState(1);
  const [progress, setProgress] = useState(0);
  const [isProcessing, setIsProcessing] = useState(false);
  const [statusLog, setStatusLog] = useState("Ready. Select an operation and click Apply.");
  const [hexText, setHexText] = useState("");
  const [usbConnected, setUsbConnected] = useState(false);
  const [instantMode, setInstantMode] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [showChangelog, setShowChangelog] = useState(false);
  
  // DMI & Search states
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResultIdx, setSearchResultIdx] = useState(-1);
  const [searchLen, setSearchLen] = useState(0);
  const [dmiInfo, setDmiInfo] = useState({
    brand: "Unknown",
    model: "Unknown",
    windows_key: "Not Found",
    serial_number: "Not Found",
    board_id: "Not Found",
    service_tag: "Not Found"
  });
  
  const [copiedField, setCopiedField] = useState("");

  // DMI Injector & Diff states
  const [showInjector, setShowInjector] = useState(false);
  const [oldBiosData, setOldBiosData] = useState(null);
  const [oldBiosName, setOldBiosName] = useState("");
  const [newBiosData, setNewBiosData] = useState(null);
  const [newBiosName, setNewBiosName] = useState("");
  
  const [diffOffsets, setDiffOffsets] = useState([]);
  // DMI inline manual edit mode states & handle
  const [editingField, setEditingField] = useState(""); // "sn" | "winKey" | "bid" | "svctag"
  const [editValue, setEditValue] = useState("");
  const [meCleanMode, setMeCleanMode] = useState("flag"); // "flag" | "python"
  const [showMeCleanModal, setShowMeCleanModal] = useState(false);

  const startEditField = (field, currentVal) => {
    setEditingField(field);
    setEditValue(currentVal);
  };

  const cancelEditField = () => {
    setEditingField("");
    setEditValue("");
  };

  const saveEditField = async (field, offset) => {
    if (!offset || offset === 0) {
      appendLog("⚠️ Gagal edit: Lokasi offset data tidak ditemukan di file BIOS.");
      setEditingField("");
      return;
    }
    if (!editValue || editValue.trim() === "") {
      appendLog("⚠️ Gagal: Nilai baru tidak boleh kosong.");
      return;
    }

    try {
      appendLog(`✍️ Mengubah byte DMI [${field}] pada offset 0x${offset.toString(16).toUpperCase()} ke: "${editValue.trim()}"`);
      const result = await invoke("overwrite_dmi_value", {
        data: Array.from(buffer),
        offset: offset,
        newValue: editValue.trim()
      });
      const bytes = new Uint8Array(result);
      setBuffer(bytes);
      
      // Update local state details
      setDmiInfo(prev => {
        const next = { ...prev };
        if (field === "sn") next.serial_number = editValue.trim();
        if (field === "winKey") next.windows_key = editValue.trim();
        if (field === "bid") next.board_id = editValue.trim();
        if (field === "svctag") next.service_tag = editValue.trim();
        return next;
      });

      // Jump Hex Viewer directly to modified offset for verification
      setSearchResultIdx(offset);
      setSearchLen(editValue.trim().length);

      appendLog(`✅ DMI [${field}] berhasil diubah di buffer RAM!`);
      playSound("success");
      setEditingField("");
    } catch (e) {
      appendLog(`❌ Gagal edit DMI: ${e}`);
      playSound("error");
    }
  };

  const [comparisonTargetName, setComparisonTargetName] = useState("");

  // Intel ME Region Cleaner states
  const [meInfo, setMeInfo] = useState({
    found: false,
    offset: "Not Found",
    size_kb: 0,
    version: "Unknown",
    status: "Unknown"
  });

  // Diagnostic BSOD Error states
  const [diagnosticError, setDiagnosticError] = useState(null);
  const [copiedDiagnostic, setCopiedDiagnostic] = useState(false);

  const searchInputRef = useRef(null);
  const instantStageRef = useRef(null); 
  const logRef = useRef(null);

  const appendLog = useCallback((msg) => {
    setStatusLog((prev) => prev + "\n" + msg);
  }, []);

  async function runPreflight({ needBuffer = false, opLabel = "operasi" } = {}) {
    const blockers = [];
    if (!usbConnected) blockers.push("CH341A tidak terdeteksi di USB (pulse merah)");
    if (!chip) blockers.push("Chip belum di-Detect");
    if (needBuffer && (!buffer || buffer.length === 0)) {
      blockers.push("Buffer kosong - Read chip atau Load File dulu");
    }

    if (blockers.length > 0) {
      const msg = `Pre-flight GAGAL untuk ${opLabel}:\n- ${blockers.join("\n- ")}`;
      appendLog(`⛔ ${msg}`);
      playSound("error");
      try {
        await message(msg, { title: "Pre-flight Check", kind: "error" });
      } catch (_) {}
      return false;
    }

    if (chipInfo?.voltage === "1.8V") {
      const ok = window.confirm(
        `Chip ${chip} = 1.8V\n\nWAJIB pakai adapter level shifter 1.8V.\nTanpa adapter chip bisa hangus.\n\nAdapter 1.8V sudah terpasang?`
      );
      if (!ok) {
        appendLog("⛔ Dibatalkan - konfirmasi adapter 1.8V ditolak operator");
        return false;
      }
      appendLog("✅ Pre-flight: adapter 1.8V dikonfirmasi operator");
    }

    appendLog(
      `✅ Pre-flight OK untuk ${opLabel} (USB + chip${needBuffer ? " + buffer" : ""})`
    );
    return true;
  }


  // Keyboard shortcut Ctrl+F to focus search input
  useEffect(() => {
    const handleKeyDown = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        if (searchInputRef.current) {
          searchInputRef.current.focus();
          searchInputRef.current.select();
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    const checkUsb = async () => {
      try {
        const result = await invoke("check_usb");
        setUsbConnected(result);
      } catch (_) {
        setUsbConnected(false);
      }
    };
    checkUsb();
    const interval = setInterval(checkUsb, 3000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const unsubs = [];
    listen("operation-progress", (e) => {
      const p = typeof e.payload === "object" ? e.payload.percent : e.payload;
      const val = typeof p === "number" ? p : 0;
      
      const stage = instantStageRef.current;
      if (stage === 'erase') {
        setProgress(Math.round(val * 0.33));
      } else if (stage === 'write') {
        setProgress(Math.round(33 + val * 0.33));
      } else if (stage === 'verify') {
        setProgress(Math.round(66 + val * 0.34));
      } else {
        setProgress(val);
      }
    }).then((u) => unsubs.push(u));
    listen("operation-log", (e) => {
      const msg = typeof e.payload === "object" ? e.payload.message : e.payload;
      if (msg) appendLog(msg);
    }).then((u) => unsubs.push(u));
    return () => unsubs.forEach((u) => u());
  }, [appendLog]);

  useEffect(() => {
    setHexText(buffer ? formatHex(buffer, searchResultIdx, searchLen, diffOffsets) : "");
  }, [buffer, searchResultIdx, searchLen, diffOffsets]);

  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [statusLog]);

  const copyToClipboard = (text, fieldName) => {
    if (text === "Not Found" || text === "Unknown") return;
    navigator.clipboard.writeText(text).then(() => {
      setCopiedField(fieldName);
      appendLog(`📋 Copied ${fieldName}: ${text}`);
      setTimeout(() => setCopiedField(""), 1500);
    });
  };

  const handleCopyDiagnostic = () => {
    if (!diagnosticError) return;
    const errText = JSON.stringify(diagnosticError, null, 2);
    navigator.clipboard.writeText(errText).then(() => {
      setCopiedDiagnostic(true);
      setTimeout(() => setCopiedDiagnostic(false), 2000);
    });
  };

  const handleSaveDiagnosticLog = async () => {
    if (!diagnosticError) return;
    try {
      const path = await save({
        title: "Save Debug Crash Log File",
        defaultPath: `error_log_${diagnosticError.code || "crash"}_${new Date().toISOString().slice(0,10)}.log`,
        filters: [{ name: "Log File", extensions: ["log", "txt", "json"] }],
      });
      if (!path) return;
      const logContent = `====================================================
MEGAPASS BIOS FLASHER - ERROR DIAGNOSTIC REPORT
Timestamp: ${new Date().toISOString()}
====================================================
ERROR CODE : ${diagnosticError.code}
MESSAGE    : ${diagnosticError.message}
LOCATION   : ${diagnosticError.file}:${diagnosticError.line}
CHIP TARGET: ${chip || "None Detected"}
====================================================
RAW CONTEXT:
${diagnosticError.context || "No raw context"}
====================================================
`;
      const encoder = new TextEncoder();
      await invoke("backup_bios", { path, data: Array.from(encoder.encode(logContent)) });
      appendLog(`💾 Diagnostic crash log saved to: ${path}`);
      playSound('success');
    } catch (e) {
      console.error("Save log error failed:", e);
    }
  };

  const handleSearch = () => {
    if (!buffer || buffer.length === 0 || !searchQuery) return;
    
    const query = searchQuery.trim();
    let queryBytes = [];

    const isHexPattern = /^(0x)?[0-9a-fA-F\s]+$/.test(query) && query.length >= 2;
    if (isHexPattern) {
      const cleanedHex = query.replace(/0x/g, "").replace(/\s+/g, "");
      if (cleanedHex.length % 2 === 0) {
        for (let i = 0; i < cleanedHex.length; i += 2) {
          queryBytes.push(parseInt(cleanedHex.substr(i, 2), 16));
        }
      }
    }
    
    if (queryBytes.length === 0) {
      const encoder = new TextEncoder();
      queryBytes = Array.from(encoder.encode(query));
    }

    setSearchLen(queryBytes.length);

    let matchOffset = -1;
    for (let i = 0; i <= buffer.length - queryBytes.length; i++) {
      let match = true;
      for (let j = 0; j < queryBytes.length; j++) {
        if (buffer[i + j] !== queryBytes[j]) {
          match = false;
          break;
        }
      }
      if (match) {
        matchOffset = i;
        break;
      }
    }

    if (matchOffset !== -1) {
      setSearchResultIdx(matchOffset);
      appendLog(`🔍 Match found at offset 0x${matchOffset.toString(16).toUpperCase()}`);
    } else {
      setSearchResultIdx(-1);
      appendLog(`⚠️ Query "${query}" not found in buffer.`);
    }
  };

  const triggerDmiExtraction = async (bytes) => {
    try {
      const info = await invoke("extract_dmi_and_key", { data: Array.from(bytes) });
      setDmiInfo(info);
      if (info.windows_key !== "Not Found") {
        appendLog(`🔑 Windows Product Key Extracted: ${info.windows_key}`);
      }
      if (info.brand !== "Unknown") {
        appendLog(`📋 Identified Device: ${info.brand} ${info.model}`);
      }

      // Trigger Intel ME Region analysis
      const me = await invoke("analyze_me_region", { data: Array.from(bytes) });
      setMeInfo(me);
      if (me.found) {
        appendLog(`⚙️ Intel ME Region Detected at ${me.offset} (Ver: ${me.version})`);
      }
    } catch (e) {
      console.error("Extraction/Analysis failed:", e);
    }
  };

  async function handleMenuClick(menuId) {
    if (isProcessing) return;
    setActiveMenu(menuId);
    const item = MENU_ITEMS.find((m) => m.id === menuId);
    if (item?.direct) {
      if (menuId === 3) { await wrapAction(handleBackup); return; }
      if (menuId === 4) { await wrapAction(handleOpenBackup); return; }
      if (menuId === 8) { setShowInjector(true); return; }
    }
  }

  async function wrapAction(fn) {
    if (isProcessing) return;
    setIsProcessing(true);
    setProgress(0);
    try { 
      await fn(); 
    } catch (err) { 
      // Capture diagnostic structure if available
      if (err && typeof err === 'object' && err.code) {
        setDiagnosticError(err);
      } else {
        // Fallback for raw command/hardware crashes
        setDiagnosticError({
          code: "ERR_SYSTEM_FATAL_0x901",
          message: String(err),
          file: "src-tauri/src/lib.rs",
          line: 0,
          context: `CH341A connected state: ${usbConnected}`
        });
      }
      appendLog(`❌ Error: ${err.message || err}`); 
      playSound('error');
    } finally { 
      setIsProcessing(false); 
    }
  }

  async function executeAction(menuId) {
    if (instantMode) {
      await wrapAction(handleInstantMode);
      return;
    }

    if (menuId === 3 || menuId === 4 || menuId === 8) {
      await handleMenuClick(menuId);
      return;
    }

    await wrapAction(async () => {
      switch (menuId) {
        case 1: await handleDetect(); break;
        case 2: await handleRead(); break;
        case 5: await handleWrite(); break;
        case 6: await handleVerify(); break;
        case 7: await handleErase(); break;
      }
    });
  }

  async function handleDetect() {
    appendLog("🔍 Detecting chip...");
    const result = await invoke("detect_chip");
    if (result.detected && result.chips && result.chips.length > 0) {
      const detectedChip = result.chips[0];
      setChip(detectedChip);
      appendLog(`✅ Chip detected: ${detectedChip}`);
      if (result.chips.length > 1) {
        appendLog(`📋 Multiple matches: ${result.chips.join(", ")}`);
      }
      try {
        const info = await invoke("get_chip_info", { chip: detectedChip });
        setChipInfo(info);
        if (info.found) {
          appendLog(`📋 ${info.manufacturer}, ${info.size_kb / 1024}MB, ${info.voltage}, ${info.package}`);
        }
      } catch (_) {}
    } else {
      setChip("");
      setChipInfo(null);
      throw {
        code: "ERR_HW_NO_CHIP_0x102",
        message: "No chip detected by programmer.",
        file: "src-tauri/src/lib.rs",
        line: 145,
        context: result.raw_output || "Check physical pins and clip connections."
      };
    }
    setProgress(100);
  }

  async function handleRead() {
    if (!chip) { appendLog("⚠️ Detect chip first!"); return; }
    appendLog(`📖 Reading from ${chip}...`);
    const start = performance.now();
    const data = await invoke("read_bios", { chip });
    const bytes = new Uint8Array(data);
    setBuffer(bytes);
    setSearchResultIdx(-1);
    setDiffOffsets([]);
    setComparisonTargetName("");
    const duration = formatDuration(performance.now() - start);
    let nonFF = 0;
    for (let i = 0; i < bytes.length; i++) { if (bytes[i] !== 0xFF) nonFF++; }
    const pctUsed = ((nonFF / bytes.length) * 100).toFixed(1);
    appendLog(`✅ Read completed in ${duration} | ${(bytes.length / 1024).toFixed(0)}KB | ${pctUsed}% used (${nonFF.toLocaleString()} non-FF bytes)`);
    setProgress(100);
    playSound('success');
    await triggerDmiExtraction(bytes);
  }

  async function handleBackup() {
    if (!buffer || buffer.length === 0) { appendLog("⚠️ No data in buffer. Read chip first!"); return; }
    const path = await save({
      title: "Save BIOS Backup",
      defaultPath: `backup_${chip || "bios"}_${new Date().toISOString().slice(0,10)}.bin`,
      filters: [{ name: "Binary", extensions: ["bin", "rom"] }],
    });
    if (!path) { appendLog("Backup cancelled."); return; }
    appendLog(`💾 Saving to ${path}...`);
    const md5 = await invoke("backup_bios", { path, data: Array.from(buffer) });
    setFileName(path);
    appendLog(`✅ Backup saved. MD5: ${md5}`);
    setProgress(100);
    playSound('success');
  }

  async function handleOpenBackup() {
    const path = await open({
      title: "Load File BIOS (.bin)",
      filters: [{ name: "Binary", extensions: ["bin", "rom"] }],
      multiple: false,
    });
    if (!path) { appendLog("Load File dibatalkan."); return; }
    appendLog(`📂 Loading ${path}...`);
    const data = await invoke("open_backup", { path });
    const bytes = new Uint8Array(data);
    setBuffer(bytes);
    setFileName(path);
    setSearchResultIdx(-1);
    setDiffOffsets([]);
    setComparisonTargetName("");
    let nonFF = 0;
    for (let i = 0; i < bytes.length; i++) { if (bytes[i] !== 0xFF) nonFF++; }
    const pctUsed = ((nonFF / bytes.length) * 100).toFixed(1);
    appendLog(`✅ Loaded ${(bytes.length / 1024).toFixed(0)}KB | ${pctUsed}% used | from ${path.split(/[/\\]/).pop()}`);
    setProgress(100);
    playSound('success');
    await triggerDmiExtraction(bytes);
  }

  // Compare (Diff): buffer A vs file B - ringkas IDENTIK/BEDA + hex markers
  const handleLoadDiffTarget = async () => {
    if (!buffer || buffer.length === 0) {
      const msg = "Load file dulu (Read chip atau Load File) sebelum Compare.";
      appendLog("⚠️ " + msg);
      try {
        await message(msg, { title: "Compare (Diff)", kind: "warning" });
      } catch (_) {}
      return;
    }
    const path = await open({
      title: "Pilih file .bin untuk dibandingkan dengan buffer",
      filters: [{ name: "Binary", extensions: ["bin", "rom"] }],
      multiple: false,
    });
    if (!path) return;
    try {
      const nameB = path.split(/[/\\]/).pop();
      appendLog(`🔍 Compare: buffer (${(buffer.length / 1024).toFixed(0)}KB) vs ${nameB}...`);
      const targetData = await invoke("open_backup", { path });
      const result = await invoke("compare_bios_diff", {
        dataA: Array.from(buffer),
        dataB: Array.from(targetData),
      });

      setComparisonTargetName(nameB);

      // Defensive: accept snake_case (serde default) or camelCase (if renamed)
      const sizeMatch = result.size_match ?? result.sizeMatch;
      const identical = result.identical;
      const msg = result.message || "";
      const offsets = result.diff_offsets || result.diffOffsets || [];
      const firstOff = result.first_offset ?? result.firstOffset;
      const diffCount = result.diff_count ?? result.diffCount ?? offsets.length;

      if (sizeMatch === false) {
        setDiffOffsets([]);
        appendLog(`❌ ${msg}`);
        try {
          await message(msg, { title: "Size beda", kind: "error" });
        } catch (_) {}
        playSound("error");
        return;
      }

      if (identical === true || diffCount === 0) {
        setDiffOffsets([]);
        appendLog(`✅ ${msg || "IDENTIK"}`);
        playSound("success");
        return;
      }

      // BEDA: mark sample offsets in hex viewer
      setDiffOffsets(Array.isArray(offsets) ? offsets : []);
      appendLog(`❌ ${msg}`);
      if (firstOff != null && firstOff !== undefined) {
        const off = Number(firstOff).toString(16).toUpperCase().padStart(8, "0");
        appendLog(`📌 Cek hex di sekitar offset 0x${off}`);
      }
      playSound("error");
    } catch (e) {
      appendLog(`❌ Comparison failed: ${e.message || e}`);
      playSound("error");
    }
  };

  // DMI Injector triggers
  const handleSelectOldBios = async () => {
    const path = await open({
      title: "Select CUSTOMER Original BIOS (Old)",
      filters: [{ name: "Binary", extensions: ["bin", "rom"] }],
    });
    if (!path) return;
    const data = await invoke("open_backup", { path });
    setOldBiosData(data);
    setOldBiosName(path.split(/[/\\]/).pop());
  };

  const handleSelectNewBios = async () => {
    const path = await open({
      title: "Select CLEAN / WORKING BIOS (New)",
      filters: [{ name: "Binary", extensions: ["bin", "rom"] }],
    });
    if (!path) return;
    const data = await invoke("open_backup", { path });
    setNewBiosData(data);
    setNewBiosName(path.split(/[/\\]/).pop());
  };

  const handleRunInjection = async () => {
    if (!oldBiosData || !newBiosData) return;
    try {
      appendLog("💉 Injecting original DMI data into Clean BIOS...");
      const result = await invoke("inject_dmi", { 
        dataOld: Array.from(oldBiosData), 
        dataNew: Array.from(newBiosData) 
      });
      const bytes = new Uint8Array(result);
      setBuffer(bytes);
      setFileName("merged_ready_to_flash.bin");
      setSearchResultIdx(-1);
      setDiffOffsets([]);
      setComparisonTargetName("");
      appendLog("✅ DMI Injection completed successfully! Loaded merged data to main buffer.");
      setShowInjector(false);
      playSound('success');
      await triggerDmiExtraction(bytes);
    } catch (e) {
      setShowInjector(false);
      // Capture detailed Rust injector error object
      setDiagnosticError(e);
      appendLog(`❌ DMI Injection failed: ${e.message || e}`);
      playSound('error');
    }
  };

  // Intel ME Region Clean with option selection
  const handleCleanMeRegion = async () => {
    if (!buffer || buffer.length === 0) return;
    setShowMeCleanModal(true);
  };

  const executeMeClean = async () => {
    setShowMeCleanModal(false);
    try {
      appendLog(`🧹 Resetting/Cleaning Intel ME Region via [${meCleanMode === "python" ? "me_cleaner.py" : "Reset Flag Cepat"}]...`);
      const result = await invoke("clean_me_region", {
        data: Array.from(buffer),
        mode: meCleanMode
      });
      const bytes = new Uint8Array(result);
      setBuffer(bytes);
      appendLog(`✅ Intel ME Region cleaned successfully via [${meCleanMode === "python" ? "me_cleaner.py" : "Reset Flag"}]!`);
      playSound("success");

      const me = await invoke("analyze_me_region", { data: Array.from(bytes) });
      setMeInfo(me);
    } catch (e) {
      setDiagnosticError(e);
      appendLog(`❌ ME Region Clean failed: ${e.message || e}`);
      playSound("error");
    }
  };


  async function handleWrite() {
    if (!(await runPreflight({ needBuffer: true, opLabel: "Write" }))) return;
    const sizeKb = (buffer.length / 1024).toFixed(0);
    if (!window.confirm(`Write ${sizeKb}KB ke ${chip}?\n\nPastikan backup sudah disimpan.`)) {
      appendLog("Write dibatalkan operator.");
      return;
    }
    appendLog(`✍️ Writing ${sizeKb}KB to ${chip}...`);
    const start = performance.now();
    await invoke("write_bios", { chip, data: Array.from(buffer) });
    const duration = formatDuration(performance.now() - start);
    appendLog(`✅ Write completed in ${duration}`);
    setProgress(100);
    playSound('success');
  }

  async function handleVerify() {
    if (!chip) { appendLog("⚠️ Detect chip first!"); return; }
    if (!buffer || buffer.length === 0) { appendLog("⚠️ No data in buffer!"); return; }
    appendLog(`🔄 Verifying ${chip}...`);
    const start = performance.now();
    const result = await invoke("verify_bios", { chip, data: Array.from(buffer) });
    const duration = formatDuration(performance.now() - start);
    appendLog(`📋 Verify result: ${result} (in ${duration})`);
    setProgress(100);
    if (result.includes("VERIFIED") || result.includes("successful")) {
      playSound('success');
    } else {
      throw {
        code: "ERR_VERIFICATION_MISMATCH_0x103",
        message: "Verify validation failed. Data mismatch.",
        file: "src-tauri/src/lib.rs",
        line: 520,
        context: result
      };
    }
  }

  async function handleErase() {
    if (!(await runPreflight({ needBuffer: false, opLabel: "Erase" }))) return;
    if (!window.confirm(`⚠️ ERASE chip ${chip}?\nThis will permanently destroy all data!`)) return;
    if (!window.confirm(`🚨 FINAL WARNING!\nAre you ABSOLUTELY SURE you want to erase ${chip}?`)) return;
    appendLog(`🗑️ Erasing ${chip}...`);
    const start = performance.now();
    await invoke("erase_bios", { chip });
    const duration = formatDuration(performance.now() - start);
    appendLog(`✅ Erase completed in ${duration}`);
    setBuffer(null);
    setProgress(100);
    playSound('success');
  }

  async function handleInstantMode() {
    if (!(await runPreflight({ needBuffer: true, opLabel: "Instant Mode" }))) return;
    const sizeKb = (buffer.length / 1024).toFixed(0);
    if (!window.confirm(`Instant Mode\nErase -> Write -> Verify\n\nTarget: ${chip}\nData: ${sizeKb}KB\n\nLanjut?`)) {
      appendLog("Instant Mode dibatalkan operator.");
      return;
    }

    appendLog("⚡ Starting Instant Mode (Erase → Write → Verify)...");
    const totalStart = performance.now();

    instantStageRef.current = 'erase';
    appendLog("[1/3] 🗑️ Erasing chip...");
    const eraseStart = performance.now();
    await invoke("erase_bios", { chip });
    const eraseTime = formatDuration(performance.now() - eraseStart);
    appendLog(`✅ Erase done in ${eraseTime}`);
    setProgress(33);

    instantStageRef.current = 'write';
    appendLog("[2/3] ✍️ Writing firmware...");
    const writeStart = performance.now();
    await invoke("write_bios", { chip, data: Array.from(buffer) });
    const writeTime = formatDuration(performance.now() - writeStart);
    appendLog(`✅ Write done in ${writeTime}`);
    setProgress(66);

    instantStageRef.current = 'verify';
    appendLog("[3/3] 🔄 Verifying...");
    const verifyStart = performance.now();
    const verifyResult = await invoke("verify_bios", { chip, data: Array.from(buffer) });
    const verifyTime = formatDuration(performance.now() - verifyStart);
    
    if (!verifyResult.includes("VERIFIED") && !verifyResult.includes("successful")) {
      throw {
        code: "ERR_INSTANT_MODE_VERIFY_0x109",
        message: "Verification failed inside Instant Mode pipeline",
        file: "src-tauri/src/lib.rs",
        line: 520,
        context: verifyResult
      };
    }
    
    appendLog(`📋 Verify result: ${verifyResult} (in ${verifyTime})`);
    setProgress(100);

    instantStageRef.current = null;
    const totalTime = formatDuration(performance.now() - totalStart);
    appendLog(`🎉 Instant Mode finished successfully in ${totalTime}!`);
    playSound('success');
  }

  const bufferSize = buffer ? `${(buffer.length / 1024).toFixed(0)}KB` : "empty";

  return (
    <div className="flex flex-col h-screen bg-base-100 text-base-content select-none font-sans">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 bg-base-300 border-b border-base-content/10">
        <div className="flex items-center gap-4">
          <div className="flex flex-col leading-tight">
            <span className="text-lg font-bold flex items-center gap-1.5">BIOS Flasher</span>
            <span className="text-[10px] opacity-60">By Teknisi Megapass Sidoarjo</span>
          </div>
          {/* USB Pulse Indicator */}
          <div className="flex items-center gap-1.5" title={usbConnected ? "CH341A Connected" : "CH341A Not Connected"}>
            <span className={`inline-block w-2.5 h-2.5 rounded-full ${usbConnected ? "bg-green-500 animate-pulse shadow-[0_0_8px_rgba(34,197,94,0.7)]" : "bg-red-500 animate-pulse shadow-[0_0_8px_rgba(239,68,68,0.5)]"}`} />
            <span className="text-[10px] opacity-75 uppercase font-semibold tracking-wider">{usbConnected ? "USB" : "NO USB"}</span>
          </div>
          {chip ? (
            <span className="badge badge-success font-mono text-xs">{chip}</span>
          ) : (
            <span className="badge badge-error badge-outline text-xs">No chip</span>
          )}
          {chipInfo?.manufacturer && (
            <span className={`badge text-xs font-bold ${chipInfo.voltage === "1.8V" ? "badge-warning animate-bounce" : "badge-ghost"}`}>
              {chipInfo.manufacturer} {chipInfo.size_kb ? `${chipInfo.size_kb/1024}MB` : ""} {chipInfo.voltage || ""}
            </span>
          )}
          {chipInfo?.voltage === "1.8V" && (
            <span className="badge badge-error font-bold text-xs animate-pulse shadow-[0_0_8px_rgba(239,68,68,0.7)]">
              ⚠️ WAJIB PAKAI ADAPTER 1.8V!
            </span>
          )}
        </div>
        <div className="flex items-center gap-3 text-sm opacity-70">
          <span>Buffer: <strong>{bufferSize}</strong></span>
          {fileName && <span className="truncate max-w-48" title={fileName}>📁 {fileName.split(/[/\\]/).pop()}</span>}
          <button 
            className="btn btn-xs btn-outline btn-primary font-mono text-[10px]"
            onClick={() => setShowChangelog(true)}
          >
            📜 Catatan Rilis
          </button>
        </div>
      </div>

      {/* Main area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left pane */}
        <div className="w-2/5 flex flex-col bg-base-200 border-r border-base-content/10 justify-between">
          <div className="flex flex-col flex-1 overflow-y-auto pt-2">
            <ul className="menu px-2 space-y-0.5">
              {MENU_ITEMS.map((item) => (
                <li key={item.id}>
                  <button
                    className={`flex items-center gap-2.5 text-sm py-2 ${
                      activeMenu === item.id ? "active bg-primary/20 text-primary font-semibold" : ""
                    } ${isProcessing ? "pointer-events-none opacity-50" : ""}`}
                    onClick={() => handleMenuClick(item.id)}
                  >
                    <span className="text-lg">{item.icon}</span>
                    {item.label}
                  </button>
                </li>
              ))}
            </ul>
          </div>

          <div className="flex flex-col">
            {/* Instant Mode Toggle */}
            <div className="px-4 py-3 border-t border-base-content/10 flex items-center gap-2">
              <input
                type="checkbox"
                id="instant-mode"
                className="checkbox checkbox-primary checkbox-sm"
                checked={instantMode}
                onChange={(e) => setInstantMode(e.target.checked)}
                disabled={isProcessing}
              />
              <label htmlFor="instant-mode" className="text-xs font-semibold c‍ursor-pointer select-none">
                ⚡ Instant Mode (Erase → Write → Verify)
              </label>
            </div>

            {/* About Developer Link */}
            <div className="p-2.5 text-center border-t border-base-content/10 bg-base-300/40">
              <button 
                className="text-xs link link-hover opacity-50 hover:opacity-100 font-medium"
                onClick={() => setShowAbout(true)}
              >
                ℹ️ About Developer
              </button>
            </div>

            {/* Progress bar */}
            <div className="p-3 border-t border-base-content/10">
              {isProcessing ? (
                <progress className="progress progress-primary w-full animate-pulse" value={progress > 0 ? progress : undefined} max="100" />
              ) : (
                <progress className={`progress w-full ${progress === 100 ? "progress-success" : "progress-primary"}`} value={progress} max="100" />
              )}
              <div className="text-xs text-center mt-1 opacity-60">
                {isProcessing ? `Processing... ${progress > 0 ? progress + "%" : ""}` : progress === 100 ? "Complete" : "Idle"}
              </div>
            </div>
          </div>
        </div>

        {/* Right pane */}
        <div className="w-3/5 flex flex-col overflow-hidden">
          {/* Opsi B Search Panel */}
          <div className="p-2 bg-base-300/80 border-b border-base-content/10 flex items-center gap-2">
            <input
              ref={searchInputRef}
              type="text"
              placeholder="Search Text or Hex (e.g. AMIBIOS or 0x4D53444D)... (Ctrl+F)"
              className="input input-bordered input-sm flex-1 font-mono text-xs focus:input-primary"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            />
            <button className="btn btn-primary btn-sm px-4" onClick={handleSearch}>
              🔍 Find
            </button>
            <button 
              className={`btn btn-sm px-3 ${diffOffsets.length > 0 ? "btn-success" : "btn-outline btn-accent"}`}
              onClick={diffOffsets.length > 0 ? () => { setDiffOffsets([]); setComparisonTargetName(""); appendLog("Diff di-reset."); } : handleLoadDiffTarget}
              title={!buffer ? "Load file dulu (Read / Load File)" : (diffOffsets.length > 0 ? "Reset tanda beda di hex" : "Bandingkan buffer dengan file .bin lain")}
            >
              📊 {diffOffsets.length > 0 ? "Reset Diff" : "Compare (Diff)"}
            </button>
          </div>

          {/* Smart Card DMI & Info */}
          <div className="p-3.5 bg-base-300/40 border-b border-base-content/10 text-xs grid grid-cols-2 gap-6 items-start">
            {/* Column 1: Device Model & Serial */}
            <div className="space-y-2.5">
              <div className="font-semibold opacity-60 uppercase tracking-wider text-[9px] flex items-center gap-2 pb-1 border-b border-base-content/10">
                <span className="w-4 text-center">📟</span>
                <span>Device Identity</span>
                {dmiInfo.brand !== "Unknown" && (
                  <span className="badge badge-accent badge-outline text-[8px] font-bold uppercase py-0 px-1.5">{dmiInfo.brand}</span>
                )}
              </div>
              
              <div className="flex justify-between items-center border-b border-base-content/5 pb-2 pt-0.5">
                <div className="flex items-center gap-2">
                  <span className="w-4 text-center opacity-70">💻</span>
                  <span className="opacity-70">Model:</span>
                </div>
                <span className="font-bold text-base-content select-text text-right truncate max-w-44" title={dmiInfo.brand !== "Unknown" ? `${dmiInfo.brand} ${dmiInfo.model}` : "Unknown"}>
                  {dmiInfo.brand !== "Unknown" ? `${dmiInfo.brand} ${dmiInfo.model}` : "Unknown"}
                </span>
              </div>
              
              <div className="flex justify-between items-center border-b border-base-content/5 pb-1.5">
                <div className="flex items-center gap-2">
                  <span className="w-4 text-center opacity-70">📋</span>
                  <span className="opacity-70">Serial Number:</span>
                </div>
                {editingField === "sn" ? (
                  <div className="flex items-center gap-1">
                    <input 
                      type="text" 
                      className="input input-bordered input-xs font-mono w-24 text-right focus:input-primary"
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                    />
                    <button className="btn btn-xs btn-success px-1" onClick={() => saveEditField("sn", dmiInfo.serial_number_offset)}>💾</button>
                    <button className="btn btn-xs btn-ghost px-1" onClick={cancelEditField}>✕</button>
                  </div>
                ) : (
                  <div className="flex items-center gap-1.5 justify-end">
                    <span className="font-mono font-semibold select-text text-right">{dmiInfo.serial_number}</span>
                    {dmiInfo.serial_number !== "Not Found" && (
                      <div className="flex gap-0.5">
                        <button 
                          className="btn btn-xs btn-ghost p-1 opacity-50 hover:opacity-100 text-info" 
                          onClick={() => startEditField("sn", dmiInfo.serial_number)}
                          title="Edit Serial Number manual"
                        >
                          ✏️
                        </button>
                        <button 
                          className={`btn btn-xs btn-ghost p-1 ${copiedField === "sn" ? "text-success" : "opacity-50 hover:opacity-100"}`} 
                          onClick={() => copyToClipboard(dmiInfo.serial_number, "sn")}
                          title="Copy to clipboard"
                        >
                          {copiedField === "sn" ? "✓" : "📋"}
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>

            {/* Column 2: Windows Key & Special Brand DMI */}
            <div className="space-y-2.5">
              <div className="font-semibold opacity-60 uppercase tracking-wider text-[9px] flex justify-between items-center pb-1 border-b border-base-content/10">
                <div className="flex items-center gap-2">
                  <span className="w-4 text-center">🔑</span>
                  <span>Security & Specs</span>
                </div>
                {comparisonTargetName && (
                  <span className="text-[8px] text-success tracking-tight bg-success/10 px-1.5 py-0.5 rounded font-mono">vs {comparisonTargetName}</span>
                )}
              </div>
              
              <div className="flex justify-between items-center border-b border-base-content/5 pb-1.5 pt-0.5">
                <div className="flex items-center gap-2">
                  <span className="w-4 text-center opacity-70">🔑</span>
                  <span className="opacity-70">Windows Key:</span>
                </div>
                {editingField === "winKey" ? (
                  <div className="flex items-center gap-1">
                    <input 
                      type="text" 
                      className="input input-bordered input-xs font-mono w-24 text-right focus:input-primary"
                      value={editValue}
                      onChange={(e) => setEditValue(e.target.value)}
                    />
                    <button className="btn btn-xs btn-success px-1" onClick={() => saveEditField("winKey", dmiInfo.windows_key_offset)}>💾</button>
                    <button className="btn btn-xs btn-ghost px-1" onClick={cancelEditField}>✕</button>
                  </div>
                ) : (
                  <div className="flex items-center gap-1.5 justify-end">
                    <span className="font-mono font-bold text-primary select-text text-right">{dmiInfo.windows_key}</span>
                    {dmiInfo.windows_key !== "Not Found" && (
                      <div className="flex gap-0.5">
                        <button 
                          className="btn btn-xs btn-ghost p-1 opacity-50 hover:opacity-100 text-info" 
                          onClick={() => startEditField("winKey", dmiInfo.windows_key)}
                          title="Edit Windows Key manual"
                        >
                          ✏️
                        </button>
                        <button 
                          className={`btn btn-xs btn-ghost p-1 ${copiedField === "winKey" ? "text-success" : "opacity-50 hover:opacity-100"}`} 
                          onClick={() => copyToClipboard(dmiInfo.windows_key, "winKey")}
                          title="Copy to clipboard"
                        >
                          {copiedField === "winKey" ? "✓" : "📋"}
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </div>

              {/* HP Specific: Board ID */}
              {dmiInfo.brand === "HP" && dmiInfo.board_id !== "Not Found" && (
                <div className="flex justify-between items-center border-b border-base-content/5 pb-1.5">
                  <div className="flex items-center gap-2">
                    <span className="w-4 text-center opacity-70">⚙️</span>
                    <span className="opacity-70 text-warning font-semibold">HP Board ID (BID):</span>
                  </div>
                  {editingField === "bid" ? (
                    <div className="flex items-center gap-1">
                      <input 
                        type="text" 
                        className="input input-bordered input-xs font-mono w-20 text-right focus:input-primary"
                        value={editValue}
                        onChange={(e) => setEditValue(e.target.value)}
                      />
                      <button className="btn btn-xs btn-success px-1" onClick={() => saveEditField("bid", dmiInfo.board_id_offset)}>💾</button>
                      <button className="btn btn-xs btn-ghost px-1" onClick={cancelEditField}>✕</button>
                    </div>
                  ) : (
                    <div className="flex items-center gap-1.5 justify-end">
                      <span className="font-mono font-bold text-warning select-text text-right">{dmiInfo.board_id}</span>
                      <div className="flex gap-0.5">
                        <button 
                          className="btn btn-xs btn-ghost p-1 opacity-50 hover:opacity-100 text-info" 
                          onClick={() => startEditField("bid", dmiInfo.board_id)}
                          title="Edit HP Board ID manual"
                        >
                          ✏️
                        </button>
                        <button 
                          className={`btn btn-xs btn-ghost p-1 ${copiedField === "bid" ? "text-success" : "opacity-50 hover:opacity-100"}`} 
                          onClick={() => copyToClipboard(dmiInfo.board_id, "bid")}
                          title="Copy to clipboard"
                        >
                          {copiedField === "bid" ? "✓" : "📋"}
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {/* Dell Specific: Service Tag */}
              {dmiInfo.brand === "Dell" && dmiInfo.service_tag !== "Not Found" && (
                <div className="flex justify-between items-center border-b border-base-content/5 pb-1.5">
                  <div className="flex items-center gap-2">
                    <span className="w-4 text-center opacity-70">🏷️</span>
                    <span className="opacity-70 text-info font-semibold">Dell Service Tag:</span>
                  </div>
                  {editingField === "svctag" ? (
                    <div className="flex items-center gap-1">
                      <input 
                        type="text" 
                        className="input input-bordered input-xs font-mono w-20 text-right focus:input-primary"
                        value={editValue}
                        onChange={(e) => setEditValue(e.target.value)}
                      />
                      <button className="btn btn-xs btn-success px-1" onClick={() => saveEditField("svctag", dmiInfo.service_tag_offset)}>💾</button>
                      <button className="btn btn-xs btn-ghost px-1" onClick={cancelEditField}>✕</button>
                    </div>
                  ) : (
                    <div className="flex items-center gap-1.5 justify-end">
                      <span className="font-mono font-bold text-info select-text text-right">{dmiInfo.service_tag}</span>
                      <div className="flex gap-0.5">
                        <button 
                          className="btn btn-xs btn-ghost p-1 opacity-50 hover:opacity-100 text-info" 
                          onClick={() => startEditField("svctag", dmiInfo.service_tag)}
                          title="Edit Dell Service Tag manual"
                        >
                          ✏️
                        </button>
                        <button 
                          className={`btn btn-xs btn-ghost p-1 ${copiedField === "svctag" ? "text-success" : "opacity-50 hover:opacity-100"}`} 
                          onClick={() => copyToClipboard(dmiInfo.service_tag, "svctag")}
                          title="Copy to clipboard"
                        >
                          {copiedField === "svctag" ? "✓" : "📋"}
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {dmiInfo.brand !== "HP" && dmiInfo.brand !== "Dell" && (
                <div className="flex justify-between items-center border-b border-base-content/5 pb-1.5">
                  <div className="flex items-center gap-2">
                    <span className="w-4 text-center opacity-70">🏷️</span>
                    <span className="opacity-70">Tag/BID Status:</span>
                  </div>
                  <span className="opacity-40 italic text-right">Not Required</span>
                </div>
              )}
            </div>
          </div>

          {/* Intel ME Region Status Panel */}
          {meInfo.found && (
            <div className="px-3 py-2 bg-warning/10 border-b border-base-content/10 flex items-center justify-between text-xs text-warning">
              <div className="flex items-center gap-2">
                <span>⚙️ <strong>Intel ME Region:</strong> Detected at <strong>{meInfo.offset}</strong> (Ver: {meInfo.version})</span>
                <span className="badge badge-warning badge-sm uppercase font-bold text-[9px]">{meInfo.status}</span>
              </div>
              <button className="btn btn-warning btn-xs px-3 font-bold" onClick={handleCleanMeRegion}>
                🧹 Clean ME Region
              </button>
            </div>
          )}

          <div className="flex-1 overflow-auto p-2">
            <pre className="hex-viewer w-full h-full p-3 rounded-lg overflow-auto font-mono text-xs select-text">
              {hexText || "No data loaded.\n\nDetect + Read chip, atau Load File (.bin)."}
            </pre>
          </div>
          <div
            ref={logRef}
            className="h-28 overflow-auto bg-base-300 border-t border-base-content/10 p-2 text-xs font-mono"
          >
            {statusLog.split("\n").map((line, i) => (
              <div key={i} className={line.startsWith("❌") ? "text-error" : line.startsWith("✅") ? "text-success" : line.startsWith("⚠") ? "text-warning" : ""}>
                {line}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Bottom */}
      <div className="p-3 bg-base-300 border-t border-base-content/10 flex justify-center">
        <button
          className={`btn btn-primary btn-wide btn-lg ${isProcessing ? "loading" : ""}`}
          onClick={() => executeAction(activeMenu)}
          disabled={isProcessing}
        >
          {isProcessing ? "Processing..." : instantMode ? "⚡ Run Instant Mode" : `▶ Apply: ${MENU_ITEMS.find(m => m.id === activeMenu)?.label}`}
        </button>
      </div>

      {/* DMI Injector Modal */}
      {showInjector && (
        <div className="modal modal-open">
          <div className="modal-box relative border border-base-content/10 bg-base-200 max-w-md">
            <button 
              className="btn btn-sm btn-circle absolute right-2 top-2"
              onClick={() => setShowInjector(false)}
            >✕</button>
            <h3 className="text-md font-bold flex items-center gap-2 text-primary">
              💉 DMI Injector & Identity Merger
            </h3>
            <p className="text-[10px] opacity-60 mt-1">Suntik data Serial / License asli dari BIOS rusak ke Clean BIOS.</p>
            
            <div className="py-4 space-y-4">
              {/* Box 1: Old Bios Input */}
              <div className="p-3 bg-base-300 rounded-lg space-y-2">
                <span className="text-xs font-semibold block">1. BIOS Lama (Unit Pelanggan)</span>
                <div className="flex gap-2">
                  <button className="btn btn-sm btn-outline btn-wide flex-1" onClick={handleSelectOldBios}>
                    {oldBiosName ? "📁 Change File" : "📂 Choose BIOS Lama"}
                  </button>
                </div>
                {oldBiosName && <span className="text-[10px] text-success block truncate">Selected: {oldBiosName}</span>}
              </div>

              {/* Box 2: New Bios Input */}
              <div className="p-3 bg-base-300 rounded-lg space-y-2">
                <span className="text-xs font-semibold block">2. BIOS Baru (Clean/Tested BIOS)</span>
                <div className="flex gap-2">
                  <button className="btn btn-sm btn-outline btn-wide flex-1" onClick={handleSelectNewBios}>
                    {newBiosName ? "📁 Change File" : "📂 Choose BIOS Baru"}
                  </button>
                </div>
                {newBiosName && <span className="text-[10px] text-success block truncate">Selected: {newBiosName}</span>}
              </div>
            </div>

            <div className="modal-action">
              <button className="btn btn-ghost btn-sm" onClick={() => setShowInjector(false)}>
                Cancel
              </button>
              <button 
                className="btn btn-primary btn-sm px-6" 
                onClick={handleRunInjection}
                disabled={!oldBiosData || !newBiosData}
              >
                ⚙️ Inject & Load Buffer
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Red Alert BSOD Diagnostic Modal */}
      {diagnosticError && (
        <div className="modal modal-open">
          <div className="modal-box relative border border-error bg-red-950/90 text-white max-w-lg select-text font-mono">
            <button 
              className="btn btn-sm btn-circle btn-ghost absolute right-2 top-2 text-white hover:bg-white/20"
              onClick={() => setDiagnosticError(null)}
            >✕</button>
            <h3 className="text-lg font-bold flex items-center gap-2 text-red-400">
              🚨 SYSTEM DIAGNOSTIC ALERT (BSOD)
            </h3>
            <p className="text-[10px] text-red-300">A hardware or backend execution error was captured by Megapass Boundary.</p>
            
            <div className="my-4 p-4 bg-black/50 border border-red-800 rounded-lg space-y-3.5 text-xs text-left">
              <div>
                <span className="text-red-400 block font-bold">ERROR CODE:</span>
                <span className="text-white text-sm tracking-wide bg-red-900/40 px-1.5 py-0.5 rounded font-bold">{diagnosticError.code}</span>
              </div>
              
              <div>
                <span className="text-red-400 block font-bold">MESSAGE:</span>
                <span className="text-red-200">{diagnosticError.message}</span>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <div>
                  <span className="text-red-400 block font-bold">FILE SOURCE:</span>
                  <span className="text-red-200 text-[10px] truncate block">{diagnosticError.file}</span>
                </div>
                <div>
                  <span className="text-red-400 block font-bold">LINE NUMBER:</span>
                  <span className="text-red-200">{diagnosticError.line > 0 ? `line ${diagnosticError.line}` : "Unknown"}</span>
                </div>
              </div>

              <div>
                <span className="text-red-400 block font-bold">DIAGNOSTIC CONTEXT:</span>
                <pre className="text-red-300 text-[10px] overflow-auto max-h-24 bg-black/30 p-2 rounded border border-red-900/20 whitespace-pre-wrap">
                  {diagnosticError.context || "No context data available"}
                </pre>
              </div>
            </div>

            <p className="text-[9px] text-red-400 italic mb-4">💡 Tip: Klik tombol di bawah ini lalu paste log ke AI untuk perbaikan langsung tanpa halusinasi.</p>

            <div className="modal-action flex justify-between items-center w-full">
              <button 
                className="btn btn-sm btn-outline btn-warning font-mono"
                onClick={handleSaveDiagnosticLog}
              >
                📥 Save Error Log (.log)
              </button>
              <div className="flex gap-2">
                <button className="btn btn-sm btn-ghost text-white hover:bg-white/10" onClick={() => setDiagnosticError(null)}>
                  Tutup
                </button>
                <button 
                  className={`btn btn-sm ${copiedDiagnostic ? "btn-success text-white" : "btn-error text-white"}`}
                  onClick={handleCopyDiagnostic}
                >
                  {copiedDiagnostic ? "✓ Diagnostic Copied!" : "📋 Copy Diagnostic for AI"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Intel ME Region Clean Option Modal */}
      {showMeCleanModal && (
        <div className="modal modal-open">
          <div className="modal-box relative border border-base-content/10 bg-base-200 max-w-sm">
            <button 
              className="btn btn-sm btn-circle absolute right-2 top-2"
              onClick={() => setShowMeCleanModal(false)}
            >✕</button>
            <h3 className="text-md font-bold flex items-center gap-2 text-warning">
              🧹 Pembersihan ME Region
            </h3>
            <p className="text-[10px] opacity-60 mt-1">Mengatasi late display atau mati tiap 30 menit akibat BIOS donor.</p>
            
            <div className="py-4 space-y-3">
              <div className="form-control bg-base-300 p-2.5 rounded-lg border border-base-content/5">
                <label className="label c‍ursor-pointer justify-start gap-3">
                  <input 
                    type="radio" 
                    name="me-clean-mode" 
                    className="radio radio-warning radio-sm"
                    checked={meCleanMode === "flag"}
                    onChange={() => setMeCleanMode("flag")}
                  />
                  <div>
                    <span className="label-text font-bold text-xs">Reset Flag Cepat (Default)</span>
                    <p className="text-[9px] opacity-50">Mengubah byte status $FPT. Cocok untuk laptop lama/TXE.</p>
                  </div>
                </label>
              </div>

              <div className="form-control bg-base-300 p-2.5 rounded-lg border border-base-content/5">
                <label className="label c‍ursor-pointer justify-start gap-3">
                  <input 
                    type="radio" 
                    name="me-clean-mode" 
                    className="radio radio-warning radio-sm"
                    checked={meCleanMode === "python"}
                    onChange={() => setMeCleanMode("python")}
                  />
                  <div>
                    <span className="label-text font-bold text-xs">Gunakan me_cleaner.py (Rekomendasi)</span>
                    <p className="text-[9px] opacity-50">Sunat partisi ME region secara mendalam. Rekomendasi Intel Gen 6 ke atas.</p>
                  </div>
                </label>
              </div>
            </div>

            <div className="modal-action gap-2">
              <button className="btn btn-ghost btn-sm" onClick={() => setShowMeCleanModal(false)}>Batal</button>
              <button className="btn btn-warning btn-sm" onClick={executeMeClean}>Jalankan Clean</button>
            </div>
          </div>
        </div>
      )}

      {/* About Modal */}
      {showAbout && (
        <div className="modal modal-open">
          <div className="modal-box relative border border-base-content/10 bg-base-200">
            <button 
              className="btn btn-sm btn-circle absolute right-2 top-2"
              onClick={() => setShowAbout(false)}
            >✕</button>
            <h3 className="text-lg font-bold flex items-center gap-2">
              🔧 Megapass Service HP & Laptop Sidoarjo
            </h3>
            <p className="text-xs opacity-60 mt-1">Version 2.1.7 (Tauri Professional Edition)</p>
            
            <div className="my-6 flex flex-col items-center justify-center py-6 border border-dashed border-base-content/20 rounded-lg bg-base-300">
              <div className="w-24 h-24 rounded-xl bg-primary/10 flex items-center justify-center border border-primary/20 text-primary font-bold text-center text-xs p-2 select-none">
                MEGAPASS LOGO
              </div>
              <span className="text-[10px] opacity-40 mt-2">Ready to load logo.png</span>
            </div>

            <div className="text-sm space-y-2 opacity-80">
              <p>📍 <strong>Alamat:</strong> Sidoarjo, Jawa Timur, Indonesia</p>
              <p>💻 <strong>Layanan:</strong> Pemrograman BIOS, Servis Laptop, Handphone, dan Komputer tingkat lanjut.</p>
              <p>🛠️ <strong>Flasher Engine:</strong> Integrated with flashrom 1.6.0 & CH341A USB controller.</p>
            </div>
            
            <div className="modal-action">
              <button className="btn btn-primary btn-sm" onClick={() => setShowAbout(false)}>
                Tutup
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Indonesian Changelog Modal */}
      {showChangelog && (
        <div className="modal modal-open">
          <div className="modal-box relative border border-base-content/10 bg-base-200 max-w-lg select-text">
            <button 
              className="btn btn-sm btn-circle absolute right-2 top-2"
              onClick={() => setShowChangelog(false)}
            >✕</button>
            <h3 className="text-lg font-bold flex items-center gap-2 text-primary">
              📜 Catatan Rilis (Changelog) - Megapass
            </h3>
            <p className="text-xs opacity-60 mt-1">Daftar pembaruan fitur flasher Megapass Sidoarjo</p>
            
            <div className="my-4 space-y-4 max-h-[350px] overflow-y-auto pr-1">
              {INDO_CHANGELOG.map((log) => (
                <div key={log.version} className="border-b border-base-content/10 pb-3 last:border-0">
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="font-mono font-bold text-sm bg-primary/10 px-2 py-0.5 rounded text-primary">
                      {log.version}
                    </span>
                    <span className="text-[10px] opacity-50 font-mono">{log.date}</span>
                  </div>
                  <ul className="list-disc pl-5 space-y-1 text-xs opacity-90 leading-relaxed text-left">
                    {log.items.map((item, idx) => (
                      <li key={idx}>{item}</li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>

            <div className="modal-action">
              <button className="btn btn-primary btn-sm" onClick={() => setShowChangelog(false)}>
                Tutup Catatan
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
