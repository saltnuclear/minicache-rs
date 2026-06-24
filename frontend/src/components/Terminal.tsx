"use client";

import { useState } from "react";

interface Props {
  apiBase: string;
  darkMode?: boolean;
}

interface LogEntry {
  time: string;
  command: string;
  result: string;
  ok: boolean;
}

export default function Terminal({ apiBase, darkMode = false }: Props) {
  const [input, setInput] = useState("");
  const [logs, setLogs] = useState<LogEntry[]>([]);

  const execute = async () => {
    const cmd = input.trim();
    if (!cmd) return;

    try {
      const res = await fetch(`${apiBase}/api/execute`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ command: cmd }),
      });
      const data = await res.json();
      const ok = !String(data.result).startsWith("-ERR");

      setLogs((prev) => [
        ...prev.slice(-49),
        { time: new Date().toLocaleTimeString(), command: cmd, result: data.result, ok },
      ]);
    } catch (e) {
      setLogs((prev) => [
        ...prev.slice(-49),
        { time: new Date().toLocaleTimeString(), command: cmd, result: "Connection failed", ok: false },
      ]);
    }
    setInput("");
  };

  const handleKey = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") execute();
  };

  const bg = darkMode ? "#1e1e1e" : "#fff";
  const color = darkMode ? "#d4d4d4" : "#333";
  const inputBg = darkMode ? "#2d2d2d" : "#f0f0f0";
  const inputBorder = darkMode ? "#444" : "#ccc";
  const inputColor = darkMode ? "#fff" : "#333";
  const placeholderColor = darkMode ? "#666" : "#999";

  return (
    <div style={{ background: bg, color: color, borderRadius: 8, padding: 16, fontFamily: "monospace", transition: "background 0.3s, color 0.3s" }}>
      <h3 style={{ marginTop: 0, color: darkMode ? "#fff" : "#333" }}>🖥️ Command Terminal</h3>

      <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
        <span style={{ color: "#4ec9b0", fontWeight: 600, lineHeight: "32px" }}>127.0.0.1:6379&gt;</span>
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKey}
          placeholder="SET key value EX 60"
          style={{
            flex: 1,
            background: inputBg,
            border: `1px solid ${inputBorder}`,
            borderRadius: 4,
            color: inputColor,
            padding: "4px 8px",
            fontFamily: "monospace",
            fontSize: 14,
            transition: "background 0.3s, color 0.3s, border-color 0.3s",
          }}
        />
        <button
          onClick={execute}
          style={{
            background: "#4a90d9",
            border: "none",
            borderRadius: 4,
            color: "#fff",
            padding: "4px 16px",
            cursor: "pointer",
            fontWeight: 600,
          }}
        >
          Send
        </button>
      </div>

      <div style={{ maxHeight: 240, overflowY: "auto" }}>
        {logs.length === 0 && (
          <div style={{ color: placeholderColor, fontStyle: "italic" }}>No commands yet. Type one above...</div>
        )}
        {logs.map((log, i) => (
          <div key={i} style={{ marginBottom: 8, fontSize: 13 }}>
            <span style={{ color: "#858585" }}>[{log.time}]</span>{" "}
            <span style={{ color: "#9cdcfe" }}>{log.command}</span>
            <span style={{ color: "#858585" }}> → </span>
            <span style={{ color: log.ok ? "#b5cea8" : "#f44747" }}>{log.result}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
