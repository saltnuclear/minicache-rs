import { useEffect, useRef, useState } from "react";
import Terminal from "./Terminal";

const API_BASE = "";

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
  const [darkMode, setDarkMode] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // 从 localStorage 读取主题偏好
  useEffect(() => {
    const saved = localStorage.getItem("mini-cache-theme");
    if (saved === "dark") setDarkMode(true);
  }, []);

  // 保存主题偏好
  useEffect(() => {
    localStorage.setItem("mini-cache-theme", darkMode ? "dark" : "light");
  }, [darkMode]);

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
        // 静默失败
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

  const bg = darkMode ? "#1a1a2e" : "#f5f5f5";
  const cardBg = darkMode ? "#16213e" : "#fff";
  const textColor = darkMode ? "#e0e0e0" : "#333";
  const subTextColor = darkMode ? "#8899aa" : "#666";
  const chartBorder = darkMode ? "#0f3460" : "#fff";

  return (
    <div style={{ minHeight: "100vh", background: bg, color: textColor, transition: "background 0.3s, color 0.3s" }}>
      <div style={{ padding: 20, maxWidth: 1200, margin: "0 auto" }}>
        {/* 顶部标题 + 夜间模式切换 */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
          <h1 style={{ margin: 0 }}>🚀 Mini-Cache Dashboard</h1>
          <button
            onClick={() => setDarkMode(!darkMode)}
            style={{
              padding: "8px 16px",
              borderRadius: 20,
              border: "none",
              background: darkMode ? "#4a90d9" : "#333",
              color: "#fff",
              cursor: "pointer",
              fontSize: 14,
              fontWeight: 600,
              display: "flex",
              alignItems: "center",
              gap: 6,
              transition: "background 0.3s",
            }}
          >
            {darkMode ? "☀️ 日间" : "🌙 夜间"}
          </button>
        </div>

        {/* 统计卡片 */}
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 16, marginBottom: 20 }}>
          <Card title="Commands" value={stats.commands} bg={cardBg} subColor={subTextColor} />
          <Card title="Connections" value={stats.connections} bg={cardBg} subColor={subTextColor} />
          <Card title="Keys" value={stats.keys} bg={cardBg} subColor={subTextColor} />
          <Card title="Hit Rate" value={`${hitRate}%`} bg={cardBg} subColor={subTextColor} />
        </div>

        {/* 图表区域 */}
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginBottom: 20 }}>
          <div style={{ background: chartBorder, borderRadius: 8, padding: 16, boxShadow: "0 2px 4px rgba(0,0,0,0.05)" }}>
            <h3 style={{ marginTop: 0, color: textColor }}>QPS Trend</h3>
            <QPSChart history={history} darkMode={darkMode} />
          </div>
          <div style={{ background: chartBorder, borderRadius: 8, padding: 16, boxShadow: "0 2px 4px rgba(0,0,0,0.05)" }}>
            <h3 style={{ marginTop: 0, color: textColor }}>Latency Distribution</h3>
            <div style={{ display: "flex", alignItems: "flex-end", gap: 12, height: 140, paddingTop: 8 }}>
              {stats.latency_histogram.map((v, i) => (
                <div key={i} style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center" }}>
                  <div style={{ fontSize: 12, marginBottom: 4, color: subTextColor }}>{v}</div>
                  <div
                    style={{
                      width: "100%",
                      height: `${(v / maxLatency) * 100}px`,
                      minHeight: 4,
                      background: darkMode ? "#4a90d9" : "#4a90d9",
                      borderRadius: 4,
                      transition: "height 0.3s ease",
                    }}
                  />
                  <div style={{ fontSize: 12, marginTop: 4, color: subTextColor }}>{labels[i]}</div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <Terminal apiBase={API_BASE} darkMode={darkMode} />
      </div>
    </div>
  );
}

function Card({ title, value, bg, subColor }: { title: string; value: string | number; bg: string; subColor: string }) {
  return (
    <div style={{ background: bg, borderRadius: 8, padding: 16, boxShadow: "0 2px 4px rgba(0,0,0,0.05)", transition: "background 0.3s" }}>
      <div style={{ fontSize: 12, color: subColor, marginBottom: 4 }}>{title}</div>
      <div style={{ fontSize: 24, fontWeight: 600 }}>{value}</div>
    </div>
  );
}

function QPSChart({ history, darkMode }: { history: { time: string; label: string; value: number }[]; darkMode: boolean }) {
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
    ctx.strokeStyle = darkMode ? "#4a90d9" : "#4a90d9";
    ctx.lineWidth = 2;

    history.forEach((pt, i) => {
      const x = i * step;
      const y = h - (pt.value / max) * h * 0.85;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
  }, [history, darkMode]);

  return <canvas ref={canvasRef} width={400} height={140} style={{ width: "100%", height: 140 }} />;
}
