"use client";

import { useEffect, useRef, useState } from "react";
import Terminal from "./Terminal";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://127.0.0.1:8080";

interface StatsData {
  commands: number;
  connections: number;
  hits: number;
  misses: number;
  keys: number;
  latency_histogram: [number, number, number, number];
}

const emptyStats: StatsData = {
  commands: 0,
  connections: 0,
  hits: 0,
  misses: 0,
  keys: 0,
  latency_histogram: [0, 0, 0, 0],
};

export default function Dashboard() {
  const [stats, setStats] = useState<StatsData>(emptyStats);
  const [history, setHistory] = useState<{ time: string; label: string; value: number }[]>([]);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const res = await fetch(`${API_BASE}/api/stats`, { cache: "no-store" });
        if (!res.ok) return;
        const data: StatsData = await res.json();
        setStats(data);
        setHistory((prev) => {
          const now = new Date().toLocaleTimeString();
          const next = [...prev, { time: now, label: "commands", value: data.commands }];
          return next.length > 60 ? next.slice(next.length - 60) : next;
        });
      } catch (e) {
        // 静默失败，避免控制台刷屏
      }
    };

    fetchStats();
    intervalRef.current = setInterval(fetchStats, 1000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, []);

  const total = stats.hits + stats.misses;
  const hitRate = total > 0 ? ((stats.hits / total) * 100).toFixed(1) : "0.0";
  const labels = ["<1ms", "1-5ms", "5-10ms", ">10ms"];
  const maxLatency = Math.max(1, ...stats.latency_histogram);

  return (
    <div style={{ padding: 20, maxWidth: 1200, margin: "0 auto" }}>
      <h1 style={{ marginBottom: 20 }}>🚀 Mini-Cache Dashboard</h1>

      {/* 顶部统计卡片 */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 16, marginBottom: 20 }}>
        <Card title="Commands" value={stats.commands} />
        <Card title="Connections" value={stats.connections} />
        <Card title="Keys" value={stats.keys} />
        <Card title="Hit Rate" value={`${hitRate}%`} />
      </div>

      {/* 中部图表 */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginBottom: 20 }}>
        <div style={{ background: "#fff", borderRadius: 8, padding: 16, boxShadow: "0 2px 4px rgba(0,0,0,0.05)" }}>
          <h3 style={{ marginTop: 0 }}>QPS Trend</h3>
          <QPSChart history={history} />
        </div>
        <div style={{ background: "#fff", borderRadius: 8, padding: 16, boxShadow: "0 2px 4px rgba(0,0,0,0.05)" }}>
          <h3 style={{ marginTop: 0 }}>Latency Distribution</h3>
          <div style={{ display: "flex", alignItems: "flex-end", gap: 12, height: 140, paddingTop: 8 }}>
            {stats.latency_histogram.map((v, i) => (
              <div key={i} style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center" }}>
                <div style={{ fontSize: 12, marginBottom: 4 }}>{v}</div>
                <div
                  style={{
                    width: "100%",
                    height: `${(v / maxLatency) * 100}px`,
                    minHeight: 4,
                    background: "#4a90d9",
                    borderRadius: 4,
                    transition: "height 0.3s ease",
                  }}
                />
                <div style={{ fontSize: 12, marginTop: 4, color: "#666" }}>{labels[i]}</div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* 底部命令行 */}
      <Terminal apiBase={API_BASE} />
    </div>
  );
}

function Card({ title, value }: { title: string; value: string | number }) {
  return (
    <div style={{ background: "#fff", borderRadius: 8, padding: 16, boxShadow: "0 2px 4px rgba(0,0,0,0.05)" }}>
      <div style={{ fontSize: 12, color: "#666", marginBottom: 4 }}>{title}</div>
      <div style={{ fontSize: 24, fontWeight: 600 }}>{value}</div>
    </div>
  );
}

function QPSChart({ history }: { history: { time: string; label: string; value: number }[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || history.length < 2) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    const max = Math.max(1, ...history.map((h) => h.value));
    const step = w / (history.length - 1);

    ctx.beginPath();
    ctx.strokeStyle = "#4a90d9";
    ctx.lineWidth = 2;

    history.forEach((pt, i) => {
      const x = i * step;
      const y = h - (pt.value / max) * h * 0.85;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
  }, [history]);

  return <canvas ref={canvasRef} width={400} height={140} style={{ width: "100%", height: 140 }} />;
}
