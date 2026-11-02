/* ──────────────────────────────────────────────────────────────────
   Polygone — Mesh canvas visualization
   Force-directed-ish layout drawn on HTMLCanvas. No external libs.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  // ── State ────────────────────────────────────────────────────────
  const peers = [
    { id: "self", name: "toi",     x: 0.5,  y: 0.5,  cpu: 35, ram: 58, kind: "self" },
    { id: "p1",   name: "dell-e6540",  x: 0.18, y: 0.22, cpu: 22, ram: 41, kind: "wifi" },
    { id: "p2",   name: "thinkpad-x250", x: 0.82, y: 0.28, cpu: 47, ram: 64, kind: "wifi" },
    { id: "p3",   name: "raspi-4",   x: 0.12, y: 0.78, cpu: 71, ram: 38, kind: "wifi" },
    { id: "p4",   name: "macbook-air", x: 0.85, y: 0.78, cpu: 12, ram: 22, kind: "wifi" },
    { id: "p5",   name: "phone-ble", x: 0.42, y: 0.88, cpu: 18, ram: 33, kind: "ble" },
  ];

  const KIND_COLOR = {
    self: "#22d3ee",
    wifi: "#22c55e",
    ble:  "#a78bfa",
  };

  // ── Canvas ───────────────────────────────────────────────────────
  const canvas = document.getElementById("canvas");
  const ctx = canvas.getContext("2d");
  let w = 800, h = 500, dpr = 1;

  const resize = () => {
    const r = canvas.getBoundingClientRect();
    dpr = window.devicePixelRatio || 1;
    canvas.width = r.width * dpr;
    canvas.height = r.height * dpr;
    w = r.width; h = r.height;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  };
  window.addEventListener("resize", resize);

  // ── Render loop ──────────────────────────────────────────────────
  const t0 = performance.now();
  const tick = (now) => {
    const t = (now - t0) / 1000;
    draw(t);
    requestAnimationFrame(tick);
  };

  const draw = (t) => {
    ctx.clearRect(0, 0, w, h);

    // Subtle grid
    ctx.strokeStyle = "rgba(30, 41, 59, 0.6)";
    ctx.lineWidth = 0.5;
    for (let x = 0; x < w; x += 40) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, h); ctx.stroke();
    }
    for (let y = 0; y < h; y += 40) {
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
    }

    const self = peers[0];

    // Connection lines (from self)
    peers.slice(1).forEach((p, i) => {
      const x1 = self.x * w, y1 = self.y * h;
      const x2 = p.x * w,   y2 = p.y * h;
      const pulse = (Math.sin(t * 1.4 + i) + 1) / 2;
      ctx.strokeStyle = `rgba(34, 211, 238, ${0.15 + pulse * 0.35})`;
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
    });

    // Lateral connections (peers between themselves)
    for (let i = 1; i < peers.length; i++) {
      for (let j = i + 1; j < peers.length; j++) {
        const a = peers[i], b = peers[j];
        const dx = (a.x - b.x) * w, dy = (a.y - b.y) * h;
        const dist = Math.hypot(dx, dy);
        if (dist < 280) {
          ctx.strokeStyle = "rgba(167, 139, 250, 0.08)";
          ctx.lineWidth = 0.6;
          ctx.beginPath();
          ctx.moveTo(a.x * w, a.y * h);
          ctx.lineTo(b.x * w, b.y * h);
          ctx.stroke();
        }
      }
    }

    // Nodes
    peers.forEach((p) => {
      const x = p.x * w, y = p.y * h;
      const color = KIND_COLOR[p.kind] || "#94a3b8";
      const r = p.kind === "self" ? 14 : 6;
      const breathe = p.kind === "self" ? 1 + Math.sin(t * 1.5) * 0.1 : 1;

      // Glow
      if (p.kind === "self") {
        const grd = ctx.createRadialGradient(x, y, 0, x, y, 50);
        grd.addColorStop(0, "rgba(34, 211, 238, 0.4)");
        grd.addColorStop(1, "rgba(34, 211, 238, 0)");
        ctx.fillStyle = grd;
        ctx.beginPath(); ctx.arc(x, y, 50, 0, Math.PI * 2); ctx.fill();
      }

      // Node circle
      ctx.fillStyle = color;
      ctx.beginPath(); ctx.arc(x, y, r * breathe, 0, Math.PI * 2); ctx.fill();

      // Label
      ctx.fillStyle = "#cbd5e1";
      ctx.font = "11px ui-monospace, monospace";
      ctx.textAlign = "center";
      const labelY = y + r + 14;
      ctx.fillText(p.name, x, labelY);

      // Sub label (CPU%)
      ctx.fillStyle = "#475569";
      ctx.font = "10px ui-monospace, monospace";
      ctx.fillText(`${Math.round(p.cpu)}%`, x, labelY + 12);
    });
  };

  // ── DOM list ─────────────────────────────────────────────────────
  const peersEl = document.getElementById("peers");
  const tpl = document.getElementById("peer-template");
  const renderList = () => {
    peersEl.innerHTML = "";
    peers.forEach((p) => {
      if (p.kind === "self") return;
      const node = tpl.content.firstElementChild.cloneNode(true);
      const dot = node.querySelector(".peer__dot");
      dot.classList.add(
        p.kind === "ble" ? "peer__dot--bt" :
        p.cpu > 65 ? "peer__dot--busy" : "peer__dot--off"
      );
      node.querySelector(".peer__name").textContent = p.name;
      node.querySelector(".peer__addr").textContent =
        p.kind === "ble" ? "Bluetooth · BLE" : `192.168.1.${Math.floor(Math.random() * 200 + 10)}`;
      const metrics = node.querySelectorAll(".peer__metric span");
      metrics[0].textContent = p.cpu;
      metrics[1].textContent = p.ram;
      peersEl.appendChild(node);
    });
  };

  // ── Init ─────────────────────────────────────────────────────────
  resize();
  renderList();
  requestAnimationFrame(tick);

  // Random walk on peers (very gentle)
  setInterval(() => {
    peers.slice(1).forEach((p) => {
      p.x += (Math.random() - 0.5) * 0.01;
      p.y += (Math.random() - 0.5) * 0.01;
      p.x = Math.max(0.05, Math.min(0.95, p.x));
      p.y = Math.max(0.1, Math.min(0.9, p.y));
      p.cpu = Math.max(5, Math.min(95, p.cpu + (Math.random() - 0.5) * 8));
    });
  }, 3000);
})();
