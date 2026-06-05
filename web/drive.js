/* ──────────────────────────────────────────────────────────────────
   Polygone — Drive logic
   Drag & drop, file list, ephemeral share links, toast.
   Works offline against a local simulation; real backend at /api/*.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  const API = "/api";
  const QUOTA_BYTES = 10 * 1024 ** 3; // 10 GB local quota

  // ── State ────────────────────────────────────────────────────────
  const state = {
    files: [
      { id: "f1", name: "notes-privees.md",  size: 4_321,     added: 1700000000, frags: "4/7", mime: "text/markdown" },
      { id: "f2", name: "photo-canons.jpg",  size: 2_456_789, added: 1700100000, frags: "4/7", mime: "image/jpeg" },
      { id: "f3", name: "archive-2025.zip",  size: 78_543_210,added: 1700200000, frags: "5/7", mime: "application/zip" },
    ],
    filter: "",
    shareFile: null,
  };

  // ── DOM ──────────────────────────────────────────────────────────
  const $ = (id) => document.getElementById(id);
  const dropzone  = $("dropzone");
  const fileInput = $("file-input");
  const pick      = $("pick");
  const refresh   = $("refresh");
  const search    = $("search");
  const tbody     = $("files-body");
  const empty     = $("empty");
  const quotaUsed = $("quota-used");
  const quotaTotal= $("quota-total");
  const modal     = $("share-modal");
  const shareFile = $("share-file");
  const shareTtl  = $("share-ttl");
  const shareMax  = $("share-max");
  const shareRes  = $("share-result");
  const shareLink = $("share-link");
  const createLnk = $("create-link");
  const copyLink  = $("copy-link");
  const closeBtn  = $("close-modal");
  const toast     = $("toast");
  const tpl       = document.getElementById("file-row");

  quotaTotal.textContent = (QUOTA_BYTES / 1024 ** 3).toFixed(0);

  // ── Format helpers ───────────────────────────────────────────────
  const fmtSize = (n) => {
    if (n < 1024) return `${n} o`;
    if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} Ko`;
    if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} Mo`;
    return `${(n / 1024 ** 3).toFixed(2)} Go`;
  };
  const fmtDate = (ts) => {
    const d = new Date(ts * 1000);
    const now = Date.now() / 1000;
    const diff = now - ts;
    if (diff < 60) return "à l'instant";
    if (diff < 3600) return `il y a ${Math.floor(diff / 60)} min`;
    if (diff < 86400) return `il y a ${Math.floor(diff / 3600)} h`;
    return d.toLocaleDateString("fr-FR", { day: "2-digit", month: "short" });
  };
  const mimeIcon = (mime) => {
    if (!mime) return "📄";
    if (mime.startsWith("image/")) return "🖼";
    if (mime.startsWith("video/")) return "🎞";
    if (mime.startsWith("audio/")) return "🎵";
    if (mime.includes("zip") || mime.includes("tar")) return "📦";
    if (mime.includes("pdf")) return "📕";
    return "📄";
  };

  // ── Toast ────────────────────────────────────────────────────────
  let toastTimer;
  const showToast = (msg, kind = "") => {
    toast.textContent = msg;
    toast.className = "toast toast--show" + (kind ? ` toast--${kind}` : "");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { toast.className = "toast"; }, 2400);
  };

  // ── Render ───────────────────────────────────────────────────────
  const renderQuota = () => {
    const used = state.files.reduce((a, f) => a + f.size, 0);
    quotaUsed.textContent = fmtSize(used);
  };
  const renderFiles = () => {
    tbody.innerHTML = "";
    const filtered = state.files.filter((f) =>
      f.name.toLowerCase().includes(state.filter.toLowerCase())
    );
    if (filtered.length === 0) {
      empty.hidden = false;
      empty.textContent = state.filter ? "Aucun résultat." : "Aucun fichier pour l'instant.";
      return;
    }
    empty.hidden = true;
    filtered.forEach((f) => {
      const row = tpl.content.firstElementChild.cloneNode(true);
      row.dataset.id = f.id;
      row.querySelector(".file__icon").textContent = mimeIcon(f.mime);
      row.querySelector(".file__label").textContent = f.name;
      row.querySelector(".file__size").textContent = fmtSize(f.size);
      row.querySelector(".file__date").textContent = fmtDate(f.added);
      row.querySelector(".file__frags").textContent = f.frags;
      tbody.appendChild(row);
    });
  };
  const render = () => { renderFiles(); renderQuota(); };

  // ── Drag & drop ──────────────────────────────────────────────────
  ["dragenter", "dragover"].forEach((ev) =>
    dropzone.addEventListener(ev, (e) => {
      e.preventDefault();
      dropzone.classList.add("dropzone--active");
    })
  );
  ["dragleave", "drop"].forEach((ev) =>
    dropzone.addEventListener(ev, (e) => {
      e.preventDefault();
      dropzone.classList.remove("dropzone--active");
    })
  );
  dropzone.addEventListener("drop", (e) => {
    const files = Array.from(e.dataTransfer.files);
    handleUpload(files);
  });
  dropzone.addEventListener("click", () => fileInput.click());
  pick.addEventListener("click", (e) => { e.stopPropagation(); fileInput.click(); });
  fileInput.addEventListener("change", () => {
    handleUpload(Array.from(fileInput.files));
    fileInput.value = "";
  });

  const handleUpload = (files) => {
    if (!files.length) return;
    files.forEach((f) => {
      // Optimistic add
      state.files.unshift({
        id: `f${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        name: f.name, size: f.size, mime: f.type || "application/octet-stream",
        added: Math.floor(Date.now() / 1000), frags: "0/7",
      });
    });
    render();
    showToast(`${files.length} fichier${files.length > 1 ? "s" : ""} en cours de chiffrement…`);

    // Simulate encryption + fragmentation progress
    files.forEach((f, i) => {
      const target = state.files.find((x) => x.name === f.name && x.size === f.size);
      if (!target) return;
      let prog = 0;
      const tick = setInterval(() => {
        prog += 1 + Math.random() * 2;
        target.frags = `${Math.min(7, Math.floor(prog / 2))}/7`;
        render();
        if (prog >= 14) {
          target.frags = "4/7";
          clearInterval(tick);
          showToast(`${f.name} prêt`, "success");
          render();
        }
      }, 200 + i * 50);
    });
  };

  // ── Row actions ──────────────────────────────────────────────────
  tbody.addEventListener("click", (e) => {
    const btn = e.target.closest("button[data-action]");
    if (!btn) return;
    const row = btn.closest("tr");
    const id = row.dataset.id;
    const file = state.files.find((f) => f.id === id);
    if (!file) return;
    const action = btn.dataset.action;
    if (action === "delete") {
      if (confirm(`Supprimer ${file.name} ?`)) {
        state.files = state.files.filter((f) => f.id !== id);
        render();
        showToast("Fichier supprimé", "success");
      }
    } else if (action === "download") {
      showToast("Téléchargement chiffré…");
    } else if (action === "share") {
      openShare(file);
    }
  });

  // ── Search & refresh ─────────────────────────────────────────────
  search.addEventListener("input", (e) => {
    state.filter = e.target.value;
    renderFiles();
  });
  refresh.addEventListener("click", () => {
    showToast("Synchronisation…", "info");
    render();
  });

  // ── Share modal ──────────────────────────────────────────────────
  const openShare = (file) => {
    state.shareFile = file;
    shareFile.textContent = file.name;
    shareRes.hidden = true;
    shareLink.textContent = "";
    modal.showModal();
  };
  closeBtn.addEventListener("click", () => modal.close());
  createLnk.addEventListener("click", (e) => {
    e.preventDefault();
    if (!state.shareFile) return;
    const token = Array.from(crypto.getRandomValues(new Uint8Array(16)))
      .map((b) => b.toString(16).padStart(2, "0")).join("");
    const link = `${location.origin}/s/${token}`;
    shareLink.textContent = link;
    shareRes.hidden = false;
    showToast("Lien généré", "success");
  });
  copyLink.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(shareLink.textContent);
      showToast("Copié", "success");
    } catch {
      showToast("Copie manuelle nécessaire", "error");
    }
  });

  // ── Init ─────────────────────────────────────────────────────────
  render();
})();
