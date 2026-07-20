import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save, message } from "@tauri-apps/plugin-dialog";

const MENU_ITEMS = [
  { id: 1, icon: "🔍", label: "Detect Chip", direct: true },
  { id: 2, icon: "📖", label: "Read", direct: false },
  { id: 3, icon: "💾", label: "Backup", direct: true },
  { id: 4, icon: "📂", label: "Open Backup", direct: true },
  { id: 7, icon: "🗑️", label: "Erase", direct: false },
  { id: 5, icon: "✍️", label: "Write", direct: false },
  { id: 6, icon: "✅", label: "Verify", direct: false },
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

function formatHex(bytes, highlightOffset = -1, searchLen = 0) {
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

  const showBytes = 65536;
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
        if (isMatch) {
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
    lines.push(`... showing 64KB from offset 0x${startOffset.toString(16).toUpperCase()} of ${totalKB}KB total`);
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

  const instantStageRef = useRef(null); 
  const logRef = useRef(null);

  const appendLog = useCallback((msg) => {
    setStatusLog((prev) => prev + "\n" + msg);
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
    setHexText(buffer ? formatHex(buffer, searchResultIdx, searchLen) : "");
  }, [buffer, searchResultIdx, searchLen]);

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
    } catch (e) {
      console.error("DMI Extraction failed:", e);
    }
  };

  async function handleMenuClick(menuId) {
    if (isProcessing) return;
    setActiveMenu(menuId);
    const item = MENU_ITEMS.find((m) => m.id === menuId);
    if (item?.direct) {
      if (menuId === 3) { await wrapAction(handleBackup); return; }
      if (menuId === 4) { await wrapAction(handleOpenBackup); return; }
    }
  }

  async function wrapAction(fn) {
    if (isProcessing) return;
    setIsProcessing(true);
    setProgress(0);
    try { 
      await fn(); 
    } catch (err) { 
      appendLog(`❌ Error: ${err}`); 
      playSound('error');
      await message(
        `Proses gagal!\n\nKemungkinan penyebab:\n1. IC Bios terkorup atau rusak\n2. Kaki adapter programmer longgar\n3. Driver tidak terinstall dengan benar\n\nDetail:\n${err}`, 
        { title: "⚠️ Hardware/Process Error", type: "error" }
      );
    } finally { 
      setIsProcessing(false); 
    }
  }

  async function executeAction(menuId) {
    if (instantMode) {
      await wrapAction(handleInstantMode);
      return;
    }

    if (menuId === 3 || menuId === 4) {
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
      appendLog("⚠️ No chip detected. Check connection.");
      if (result.raw_output) appendLog(result.raw_output.slice(0, 300));
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
      title: "Open BIOS Backup File",
      filters: [{ name: "Binary", extensions: ["bin", "rom"] }],
      multiple: false,
    });
    if (!path) { appendLog("Open cancelled."); return; }
    appendLog(`📂 Loading ${path}...`);
    const data = await invoke("open_backup", { path });
    const bytes = new Uint8Array(data);
    setBuffer(bytes);
    setFileName(path);
    setSearchResultIdx(-1);
    let nonFF = 0;
    for (let i = 0; i < bytes.length; i++) { if (bytes[i] !== 0xFF) nonFF++; }
    const pctUsed = ((nonFF / bytes.length) * 100).toFixed(1);
    appendLog(`✅ Loaded ${(bytes.length / 1024).toFixed(0)}KB | ${pctUsed}% used | from ${path.split(/[/\\]/).pop()}`);
    setProgress(100);
    playSound('success');
    await triggerDmiExtraction(bytes);
  }

  async function handleWrite() {
    if (!chip) { appendLog("⚠️ Detect chip first!"); return; }
    if (!buffer || buffer.length === 0) { appendLog("⚠️ No data in buffer!"); return; }
    appendLog(`✍️ Writing ${(buffer.length / 1024).toFixed(0)}KB to ${chip}...`);
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
      throw new Error(`Verification mismatch: ${result}`);
    }
  }

  async function handleErase() {
    if (!chip) { appendLog("⚠️ Detect chip first!"); return; }
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
    if (!chip) { appendLog("⚠️ Detect chip first!"); return; }
    if (!buffer || buffer.length === 0) { appendLog("⚠️ No data in buffer to write!"); return; }

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
      throw new Error(`Verification mismatch: ${verifyResult}`);
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
            <span className="badge badge-ghost text-xs">
              {chipInfo.manufacturer} {chipInfo.size_kb ? `${chipInfo.size_kb/1024}MB` : ""} {chipInfo.voltage || ""}
            </span>
          )}
        </div>
        <div className="flex items-center gap-3 text-sm opacity-70">
          <span>Buffer: <strong>{bufferSize}</strong></span>
          {fileName && <span className="truncate max-w-48" title={fileName}>📁 {fileName.split(/[/\\]/).pop()}</span>}
        </div>
      </div>

      {/* Main area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left pane */}
        <div className="w-2/5 flex flex-col bg-base-200 border-r border-base-content/10 justify-between">
          <div className="flex flex-col flex-1 overflow-y-auto pt-2">
            <ul className="menu px-2">
              {MENU_ITEMS.map((item) => (
                <li key={item.id}>
                  <button
                    className={`flex items-center gap-3 text-base py-3.5 ${
                      activeMenu === item.id ? "active bg-primary/20 text-primary font-semibold" : ""
                    } ${isProcessing ? "pointer-events-none opacity-50" : ""}`}
                    onClick={() => handleMenuClick(item.id)}
                  >
                    <span className="text-xl">{item.icon}</span>
                    {item.label}
                  </button>
                </li>
              ))}
            </ul>

            {/* Smart Card DMI & Info (Opsi 1 + Clipboard copy) */}
            <div className="mx-4 my-2 p-3 bg-base-300/60 border border-base-content/10 rounded-lg text-xs space-y-2">
              <div className="font-semibold opacity-60 uppercase tracking-wider text-[10px] flex justify-between items-center">
                <span>📟 Detected Device Identity</span>
                {dmiInfo.brand !== "Unknown" && (
                  <span className="badge badge-accent badge-outline text-[9px] scale-95 font-bold uppercase">{dmiInfo.brand}</span>
                )}
              </div>

              {/* Dynamic Brand & Model */}
              <div className="flex justify-between border-b border-base-content/5 py-1">
                <span className="opacity-70">💻 Device Model:</span>
                <span className="font-bold text-base-content select-text">
                  {dmiInfo.brand !== "Unknown" ? `${dmiInfo.brand} ${dmiInfo.model}` : "Unknown"}
                </span>
              </div>

              {/* Windows Key (Always shown) */}
              <div className="flex justify-between items-center border-b border-base-content/5 py-1">
                <span className="opacity-70">🔑 Windows Key:</span>
                <div className="flex items-center gap-1.5">
                  <span className="font-mono font-bold text-primary select-text">{dmiInfo.windows_key}</span>
                  {dmiInfo.windows_key !== "Not Found" && (
                    <button 
                      className={`btn btn-xs btn-ghost p-1 ${copiedField === "winKey" ? "text-success" : "opacity-50 hover:opacity-100"}`} 
                      onClick={() => copyToClipboard(dmiInfo.windows_key, "winKey")}
                      title="Copy to clipboard"
                    >
                      {copiedField === "winKey" ? "✓" : "📋"}
                    </button>
                  )}
                </div>
              </div>

              {/* Serial Number (Always shown) */}
              <div className="flex justify-between items-center border-b border-base-content/5 py-1">
                <span className="opacity-70">📋 Serial Number:</span>
                <div className="flex items-center gap-1.5">
                  <span className="font-mono font-semibold select-text">{dmiInfo.serial_number}</span>
                  {dmiInfo.serial_number !== "Not Found" && (
                    <button 
                      className={`btn btn-xs btn-ghost p-1 ${copiedField === "sn" ? "text-success" : "opacity-50 hover:opacity-100"}`} 
                      onClick={() => copyToClipboard(dmiInfo.serial_number, "sn")}
                      title="Copy to clipboard"
                    >
                      {copiedField === "sn" ? "✓" : "📋"}
                    </button>
                  )}
                </div>
              </div>

              {/* HP Specific: Board ID */}
              {dmiInfo.brand === "HP" && dmiInfo.board_id !== "Not Found" && (
                <div className="flex justify-between items-center border-b border-base-content/5 py-1">
                  <span className="opacity-70 text-warning font-semibold">⚙️ Board ID (BID):</span>
                  <div className="flex items-center gap-1.5">
                    <span className="font-mono font-bold text-warning select-text">{dmiInfo.board_id}</span>
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

              {/* Dell Specific: Service Tag */}
              {dmiInfo.brand === "Dell" && dmiInfo.service_tag !== "Not Found" && (
                <div className="flex justify-between items-center border-b border-base-content/5 py-1">
                  <span className="opacity-70 text-info font-semibold">🏷️ Dell Service Tag:</span>
                  <div className="flex items-center gap-1.5">
                    <span className="font-mono font-bold text-info select-text">{dmiInfo.service_tag}</span>
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
              type="text"
              placeholder="Search Text or Hex (e.g. AMIBIOS or 0x4D53444D)..."
              className="input input-bordered input-sm flex-1 font-mono text-xs focus:input-primary"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            />
            <button className="btn btn-primary btn-sm px-4" onClick={handleSearch}>
              🔍 Find
            </button>
            {searchResultIdx !== -1 && (
              <button 
                className="btn btn-ghost btn-sm text-[10px] opacity-50 px-1"
                onClick={() => setSearchResultIdx(-1)}
              >
                ✕ clear
              </button>
            )}
          </div>

          <div className="flex-1 overflow-auto p-2">
            <pre className="hex-viewer w-full h-full p-3 rounded-lg overflow-auto font-mono text-xs select-text">
              {hexText || "No data loaded.\n\nDetect a chip and Read, or Open a backup file."}
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
            <p className="text-xs opacity-60 mt-1">Version 2.1.0 (Tauri Professional Edition)</p>
            
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
    </div>
  );
}
