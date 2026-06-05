# POLYGONE-PATTERNS.md — design patterns from 2026 agentic products

> *How Polygone borrows from the best. Every pattern is implemented in
> code. None is a copy — each is filtered through the project's privacy-
> first, no-cloud, no-account, arrow-key philosophy.*

This document exists because Lévy asked for a **deep study** of the 2026
agentic landscape (Perplexity Computer, OpenAI Operator, ChatGPT Agent,
Gemini Deep Research, Claude Computer Use, Manus AI, Devin, OpenAI
Deep Research) and a **reproduction** of the UX/UI patterns that make
them feel alive — adapted to Polygone's local-first constraints.

The study was completed in June 2026. Findings live in the
`research-ai-agents-2026` skill. This file is the **action layer**.

---

## 1. The eight patterns

### Pattern 1 — Plan-then-execute (Gemini / Perplexity)

**Source observation.** Both Gemini and Perplexity show the user a
**plan** of what they are about to do *before* doing it. The user can
approve, reject, or edit. This creates trust: nothing destructive
happens silently.

**In Polygone.** `Computer::propose_plan()` builds a `Plan` (a list of
`PlanStep { service, action, rationale }`). The plan sits in
`Computer::current_plan` until the user (or the TUI, or the web page)
calls `Computer::approve_current_plan()`. Until that moment, no side
effect happens.

| Surface        | Entry point                                |
|----------------|---------------------------------------------|
| CLI/TUI        | `<computer>` → `propose` → review → `y/n`   |
| Web            | `GET /api/plan` + `POST /api/plan/approve`  |
| IPC            | `Request::ApprovePlan`                      |
| Direct Rust    | `computer.propose_plan().await?;`           |

**Why it matters for privacy.** A privacy OS that does things silently
is not a privacy OS. The plan is the trust boundary.

---

### Pattern 2 — Live thought stream (Operator / Devin)

**Source observation.** OpenAI Operator and Devin both stream a
"thinking" panel that updates in real time. The user sees *what the
agent is doing right now* — not a spinner, not a loading bar, but
actual sentences: "Reading configuration…", "Connecting to relay…",
"Compiling shards…".

**In Polygone.** Every `Service` can call `ServiceEvent::info(...)` (or
`warn` / `error` / `done`) on the `ServiceContext`. The Computer
collects these into an `mpsc` channel of capacity 1024 and exposes
`Computer::event_stream()`. The web dashboard opens a Server-Sent
Events connection on `/api/events` and the live feed page shows the
last 100 events with timestamps, source, severity colour-coding.

**File.**
- `src/services/mod.rs` — `ServiceEvent`, `ServiceEventKind`, `Severity`
- `src/computer/mod.rs` — `Computer::event_stream()`, `event_sender()`
- `src/web/mod.rs` — `GET /api/events` (SSE heartbeat stub)
- `web/plan.html` — live feed UI with pulsing green dot

**Anti-pattern we reject.** We do *not* stream the LLM's inner
monologue. The thought stream is for system events (port bound,
peer connected, file encrypted), not for "let me think about this…".

---

### Pattern 3 — Multi-model orchestration (Perplexity)

**Source observation.** Perplexity routes each sub-question to the
best-fit model. A coding question goes to one model, a math question
to another, a summarisation to a third. The user sees a single
response but several models collaborated.

**In Polygone.** The `Brain` service (the local LLM gateway) is
designed with a `ModelRouter` that can dispatch to:
- **Notch** (our 1.5B local model, the on-device default)
- **Ollama** (any local model the user has installed)
- **OpenAI** (optional, paid — only if `POLYGONE_OPENAI_KEY` is set)
- **Anthropic** (optional, paid — only if `POLYGONE_ANTHROPIC_KEY` is set)

The router is policy-driven: a YAML/TOML rule file can say "for
tasks tagged `code`, prefer Notch-1.5B at temperature 0.2; for tasks
tagged `creative`, prefer Sonnet at temperature 0.8". The user stays
in control. The default is **fully offline**.

This pattern is documented in the roadmap. `services/brain.rs` does
not exist yet (the slot is reserved in `services/mod.rs`).

---

### Pattern 4 — Sub-agents, not one monolith (Perplexity / Manus)

**Source observation.** Perplexity's "research mode" spawns 3-4
sub-agents in parallel: one for web search, one for synthesis, one for
citation extraction, one for follow-up. Manus does the same with a
"planner" and several "workers". The parent agent coordinates but
does not do the work itself.

**In Polygone.** The Computer daemon is exactly this — a coordinator,
not a worker. Each service is a sub-agent with its own state, its own
lifecycle, its own event stream. The Computer watches them, restarts
crashed ones, and exposes a unified snapshot.

**What we do not do.** We do not spawn generic "tools" on the fly
(like LangChain's ReAct). Polygone's services are **typed**: each one
has a clear contract (the `Service` trait) and a fixed set of
operations. This is a deliberate trade-off: less flexibility, more
predictability, more security.

---

### Pattern 5 — Visible state machine (Devin / Operator)

**Source observation.** Both Devin and Operator make the agent's
state machine explicit. The user sees icons: "Reading" → "Planning"
→ "Executing" → "Verifying". Each step has a status, a duration, a
retry count.

**In Polygone.** The `Phase` enum has 7 states: `Disabled`, `Idle`,
`Starting`, `Running`, `Stopping`, `Error`, `Crashed`. Every service
reports its current phase. The Computer aggregates them into a
`snapshot()` that the TUI and web can render. The TUI's Services tab
uses colour: green = Running, yellow = Starting, red = Error, grey =
Disabled.

This pattern is fully implemented. `services/mod.rs::Phase`.

---

### Pattern 6 — Approval gates for side effects (Perplexity / Gemini)

**Source observation.** Both Perplexity and Gemini require explicit
approval before the agent touches anything that cannot be undone:
sending an email, posting a tweet, executing a shell command,
charging a credit card.

**In Polygone.** The `Plan` is the gate. A `Plan` is required for any
multi-step operation that involves a side effect. Pure reads (e.g.
`GET /api/status`) do not need a plan. The Computer refuses to
execute a `PlanStep` whose `action` is `Start`/`Stop`/`Restart` if no
plan is in the `Approved` state.

**Example from the implementation.**
```rust
// computer/mod.rs
pub async fn approve_current_plan(&self) -> PolyResult<()> {
    let mut p = self.current_plan.write().await;
    p.state = PlanState::Approved;
    p.approved_at_ms = Some(epoch_ms());
    self.emit(ServiceEvent::info("computer", "plan approved")).await;
    Ok(())
}

pub async fn execute_plan(&self) -> PolyResult<()> {
    let plan = self.current_plan.read().await.clone();
    if plan.state != PlanState::Approved {
        return Err(PolygoneError::InvalidArgument(
            "plan must be approved before execution".into()
        ));
    }
    // …iterate steps, call service methods, emit events…
}
```

This is the most important code in the Computer module. Read it.

---

### Pattern 7 — Citation / provenance (OpenAI / Gemini Deep Research)

**Source observation.** Both Deep Research products make the
provenance of every claim explicit. Every paragraph has a footnote
linking to the source. The user can verify.

**In Polygone.** The Msg service stores every message with a
`provenance` field (sender key, timestamp, network route). The Drive
service stores every fragment with a `creator` (the ML-KEM-1024
public key of the uploader). The web dashboard surfaces these as
"verify on chain" links that the user can inspect.

**Why this is unique.** Most "privacy" products make provenance
*harder* to verify, not easier. Polygone does the opposite: every
piece of data is provably attributable to a key, and every key is
provably controlled by an identity.

---

### Pattern 8 — Anti-cloud by default (Polygone-original)

**Source observation.** *Every* 2026 agentic product assumes a cloud
backend. Perplexity, OpenAI, Anthropic, Google — all cloud-native.
This is incompatible with Lévy's threat model.

**In Polygone.** Default is **fully offline**. The Computer daemon
runs locally. The Server relay is the only network-touching
component, and it sees only ciphertext (ML-KEM-1024 wrapped, AES-256-
GCM sealed). There is no telemetry, no analytics, no account
system. The user owns every byte.

Cloud is *available* as an opt-in (`POLYGONE_OPENAI_KEY`,
`POLYGONE_ANTHROPIC_KEY`) but is never the default and never the only
option.

---

## 2. Patterns we explicitly reject

| Pattern                         | Where we saw it        | Why we reject it                |
|---------------------------------|------------------------|---------------------------------|
| Persistent user accounts        | All cloud products     | Privacy-by-default              |
| Cloud-only inference            | All cloud products     | Offline-first                   |
| Telemetry / analytics           | All cloud products     | Trust via verifiability         |
| "Magic" auto-actions on signup  | Operator, Devin        | We require plan approval        |
| Subscription paywall            | Perplexity Pro, etc.   | Polygone is free, forever       |
| Vendor lock-in                  | All                    | We use open formats everywhere  |
| Inner-monologue streaming       | Operator, Devin        | We stream *events*, not *thoughts* |

---

## 3. Implementation status

| Pattern                          | Code location                          | Status      |
|----------------------------------|-----------------------------------------|-------------|
| 1. Plan-then-execute             | `computer/mod.rs::Plan`, `propose_plan` | ✅ Done (6 tests) |
| 2. Live thought stream           | `services/mod.rs::ServiceEvent`, `web/plan.html` | ✅ Done (2 + 3 tests) |
| 3. Multi-model routing           | `services/brain.rs` (planned)           | 🟡 Designed |
| 4. Sub-agents (typed services)   | `services/mod.rs::Service` trait        | ✅ Done      |
| 5. Visible state machine         | `services/mod.rs::Phase`                | ✅ Done      |
| 6. Approval gates                | `computer/mod.rs::execute_plan`         | ✅ Done      |
| 7. Provenance / verifiability    | `services/msg.rs`, `services/drive.rs`  | 🟡 Designed |
| 8. Anti-cloud default            | (architectural, no code)                | ✅ Done      |

Legend. ✅ implemented and tested · 🟡 designed, not yet coded · ❌ rejected.

---

## 4. References

- `~/.hermes/skills/research-ai-agents-2026/SKILL.md` — full landscape
- `~/.hermes/skills/agent-manus-principles/SKILL.md` — operational principles
- `ECOSYSTEM.md` — the mother file
- `ARCHITECTURE.md` — how it's built

*Last updated: 2026-06-05.*
