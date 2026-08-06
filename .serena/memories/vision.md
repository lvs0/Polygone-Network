# Polygone — Vision & Mission (Zoe's Deep Re-Vision)

## The truth about Polygone

It is not "a P2P network" or "a crypto project" or "a chat app". 
It is **a political act by a 14-year-old who refuses to be tracked**.

Lévy wrote: "No server sees the message. No observer can prove a message existed."
That is the entire mission. Everything else is engineering to make it real.

---

## What it's NOT

- NOT a blockchain (no token, no mining, no consensus)
- NOT a VPN (no centralized tunnel, no trust in provider)
- NOT Signal (not just encrypted messaging — it's infrastructure)
- NOT a "crypto project" in the speculative sense (no ICO, no investors)
- NOT just academic (the crypto is real, the P2P is real, the use case is real)

## What it IS

**An ephemeral sovereign transit network.** 
Information flows through it without residing in it.
Nodes relay without seeing. Observers can't prove existence.
Post-quantum because quantum computers will break today's RSA in 10-15 years —
and Lévy thinks in decades, not quarters.

---

## The 5 layers (what exists vs. what needs work)

### Layer 1: CRYPTO ✅ DONE
ML-KEM-1024, ML-DSA-87, AES-256-GCM, Shamir 4/7, BLAKE3.
Real post-quantum. Not marketing. The foundation is solid.

### Layer 2: P2P TRANSPORT ✅ MOSTLY DONE
libp2p with 9 protocols stacked. Clean. Proper.
Missing: end-to-end demo (2 nodes in same LAN exchanging a message).

### Layer 3: SERVICES ✅ GOOD
Service trait. Phase/Health. Metrics. Extensible.
Every component (Drive, Hide, Mesh, Brain, Compute) is a Service.
This is the right model.

### Layer 4: ORCHESTRATION 🔶 INCOMPLETE
Computer is beautiful but disconnected from P2P.
Plan/PlanStep/PlanAction + SSE events = the right abstraction.
Need: a clear interface between Computer and the P2P network.
Who owns the P2pNode? Does Computer spawn it? Or do they communicate?

### Layer 5: DISTRIBUTED COMPUTE 🔴 MOSTLY VISION
The "Petals-style compute lending" is the most ambitious part.
Alice's GPU idle → lends to Bob → Bob runs inference → encrypted result flows back.
This is the killer feature. Not just chat. Not just storage.
**Distributed AI inference on idle hardware.** That's the cartonnant vision.

---

## The 3 things that matter before going public

### 1. A working local loop demo
Two instances on the same machine exchange a fragment. Visible in both TUIs.
That's the minimum viable proof. Everything else follows from this.

### 2. A clean push with CI
v1.0.0 tag, GitHub Actions (test + clippy), README with working instructions.
First impression is CI. If it fails, nothing else matters.

### 3. A one-page landing
polygone.network (or .xyz): the manifest, the architecture diagram, the screenshot.
Not a whitepaper. A **manifesto** in one page.

---

## What "v2.0 cartonnant" actually means

Not more features. The opposite: fewer, sharper, done.

| Feature | Status | Action |
|---|---|---|
| ML-KEM-1024 handshake | ✅ Done | Benchmark it |
| Local 2-node loop | ❌ Missing | Demo first |
| Computer → P2pNode link | ❌ Missing | Define interface |
| Petals-style compute lending | 🔶 Partial | Mock it real |
| CI/CD pipeline | ❌ Missing | Add GitHub Actions |
| README | ⚠️ Old | Rewrite with demo |
| v1.0.0 tag | ❌ Missing | Tag it |
| Landing page | ❌ Missing | One page manifesto |
| TUI dashboard | ✅ Done | Keep it |

---

## The brand (my re-visioning)

**Name**: Polygone
**One-liner**: "L'information n'existe pas. Elle traverse."
**Target**: People who refuse to be surveilled + builders who want sovereign infra
**Positioning**: Not crypto. Not chat. Not VPN. 
Sovereign ephemeral infrastructure. Post-quantum. Distributed.

**What to show**: 
- Architecture diagram (clean, minimal, not cluttered)
- TUI screenshot (it's genuinely beautiful)
- One demo video (2 nodes, message transit, proof of no server)
- The manifest sentence on a black background

**What NOT to do**:
- Don't show whitepapers
- Don't claim "decentralized" unless there's real bootstrap
- Don't compete on features — compete on purity of concept

---

## My strategic recommendation

**Week 1-2 (finish)**: Local demo + CI + v1.0.0 tag + README rewrite
**Week 3-4 (connect)**: Computer ↔ P2pNode interface + SSE event bus over P2P
**Week 5-6 (launch)**: Landing page + compute lending mock demo + social proof

Don't touch the crypto. Don't touch the libp2p stack. 
Work on the ORCHESTRATION layer (Computer ↔ P2pNode) and the PRESENTATION layer (demo + landing).

The code is ready. The vision is clear. The last mile is integration and storytelling.

---

## References
`mem:core` — full architecture map
`mem:technical_debt` — issues and roadmap
`mem:levy_profile` — who Lévy is (from Zoe Brain)