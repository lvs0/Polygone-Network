/* ──────────────────────────────────────────────────────────────────
   Polygone — landing page interactions
   No framework. Plain DOM. Respects prefers-reduced-motion.
   ────────────────────────────────────────────────────────────────── */

(() => {
  "use strict";

  // ── Tab switcher (install section) ───────────────────────────────
  const tabs = document.querySelectorAll(".install__tab");
  const codes = document.querySelectorAll(".install__code");

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      const target = tab.dataset.os;
      tabs.forEach((t) => t.classList.toggle("install__tab--active", t === tab));
      codes.forEach((c) => {
        if (c.dataset.os === target) c.removeAttribute("hidden");
        else c.setAttribute("hidden", "");
      });
    });
  });

  // ── Animated node count (hero pills sparkle) ─────────────────────
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (!reduce) {
    const pills = document.querySelectorAll(".hero__pills li");
    pills.forEach((pill, i) => {
      pill.style.opacity = "0";
      pill.style.transform = "translateY(4px)";
      pill.style.transition = "opacity 400ms ease, transform 400ms ease";
      setTimeout(() => {
        pill.style.opacity = "1";
        pill.style.transform = "translateY(0)";
      }, 200 + i * 80);
    });
  }

  // ── Smooth scroll for anchor links (progressive enhancement) ────
  document.querySelectorAll('a[href^="#"]').forEach((a) => {
    a.addEventListener("click", (e) => {
      const id = a.getAttribute("href").slice(1);
      const tgt = document.getElementById(id);
      if (tgt) {
        e.preventDefault();
        tgt.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "start" });
      }
    });
  });
})();
