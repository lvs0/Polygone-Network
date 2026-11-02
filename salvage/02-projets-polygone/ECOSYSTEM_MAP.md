# ⬡ POLYGONE ECOSYSTEM MAP

> Les 7 couches de l'organisme Polygone.  
> Chaque couche respire, doute, et meurt. Comme nous.

---

## The 7 Layers

```
                            ⬡ POLYGONE ⬡
         ┌─────────────────────────────────────────────────┐
   L1  │              ⬡ THE HEXAGRAM                       │ ⬡ Brand, identity, signature
         ├─────────────────────────────────────────────────┤
   L2  │              ⬡ THE PULSE                          │ ⬡ Heartbeat protocol
         ├─────────────────────────────────────────────────┤
   L3  │              ⬡ THE MEMBRANE                       │ ⬡ Network layer (libp2p + DHT)
         ├─────────────────────────────────────────────────┤
   L4  │              ⬡ THE BRAIN                          │ ⬡ Reasoning + 13 personalities
         ├─────────────────────────────────────────────────┤
   L5  │              ⬡ THE NEURAL PETALS                  │ ⬡ Distributed LLM inference
         ├─────────────────────────────────────────────────┤
   L6  │              ⬡ THE SENTINEL                       │ ⬡ Autonomous watchman
         ├─────────────────────────────────────────────────┤
   L7  │              ⬡ THE ECHO                           │ ⬡ Local ephemeral memory
         └─────────────────────────────────────────────────┘
```

---

## L1 — The Hexagram (Brand Layer)

```
   ⬡
```

**What it does:**
- Identity visible to all, content visible to none.
- 6 vertices = 6 dimensions of privacy.
- Single shape Apple can't trademark without abandoning its fruit.

**Public surface:**
- MANIFESTO.md
- DESIGN_PHILOSOPHY.md
- All visual assets
- `privacy.is` domain (forthcoming)

---

## L2 — The Pulse (Heartbeat)

**What it does:**
- 32-byte cryptographic heartbeat every 5 seconds.
- Signed with ML-DSA-87.
- Identity is the hash, not the key.

**Implementation:** `polygone-core::organism::Pulse`

**Anti‑Apple feature:** Apple Watch wants to track your heartbeat for health.  
We track it for **anti‑surveillance**.

---

## L3 — The Membrane (Network Layer)

**What it does:**
- libp2p v0.56 + Kademlia DHT.
- Each node = a cell in the organism.
- Fragments circulate, never accumulate.

**Implementation:** `crates/network/`

**The Membrane filters what comes in and what goes out**, just like a biological membrane.

---

## L4 — The Brain (Reasoning Engine)

**What it does:**
- 13 personalities debate each question.
- Socratique protocol: doubt before answer.
- Final answer may have a gap (the "trou socratique").

**Implementation:** `polygone-brain/`

**The Brain is not a chatbot.** It's a council of elders that converse with you without ever claiming to know.

---

## L5 — The Neural Petals (PETALS_NEURO)

**What it does:**
- Distributed post-quantum LLM inference.
- 70B model split across N nodes.
- Hidden states encrypted with ML-KEM-1024.

**Implementation:** `polygone-petals/` + `docs/PETALS_NEURO.md`

**The Petals are thin**, like a flower's petals. Each petal is a node holding one slice of the model. Together they form a flower — the **inference**. They die when the bloom ends.

---

## L6 — The Sentinel (Watchman)

**What it does:**
- Autonomous monitoring without central coordination.
- Vitality tracking.
- Self-healing via key rotation.

**Implementation:** `polygone-core::organism::Sentinel`

**The Sentinel never alerts anyone.** It just acts silently. Like an immune system firing white blood cells — you never know until years later, when statistics are on your side.

---

## L7 — The Echo (Local Ephemeral Memory)

**What it does:**
- A local append-only log of significant events.
- Polygone forgets, but the user can see *what happened recently*.
- The log is wiped when TTL expires.

**Implementation:** `polygone-core::organism::EchoChain`

**The Echo is not memory.** Memory is for you. Echo is for "what just happened that I might want to remember".

---

## How the 7 layers relate

- **Apple** thinks in **levels**: user → platform → datacenter → government.
- **Polygone** thinks in **cycles**: pulse → membrane → brain → echo → pulse...

Each cycle is a **breath**. 5 seconds. Then the cycle restarts.

---

## The Map vs. The Apple Stack

| Apple | Polygone |
|-------|----------|
| iOS | **Layer 1**: the hexagram |
| macOS | **Layer 3**: the membrane |
| iCloud | **None.** We refuse the cloud. |
| Apple Intelligence | **Layer 4**: the brain (always doute) |
| Vision Pro | **Layer 5**: neural petals (LLM as a screen you don't wear) |
| Find My iPhone | **None.** We can't find what we don't track. |
| AirDrop | **Layer 2**: pulse (proximity without persistence) |

We don't replace Apple's *devices*.
We replace Apple's **claim to know**.

---

## The "single sentence" of each layer

| Layer | Single sentence |
|-------|------------------|
| L1 — Hexagram | "Privacy is the new oxygen." |
| L2 — Pulse | "Your heartbeat is anonymous." |
| L3 — Membrane | "Everything passes through, nothing accumulates." |
| L4 — Brain | "Before answering, we doubt." |
| L5 — Petals | "70B models, post-quantum, no central server." |
| L6 — Sentinel | "We watch without telling." |
| L7 — Echo | "30 seconds of memory. Nothing more." |

---

## Why this matters beyond tech

Apple is a **country** with a flag.  
Google is a **religion** with a book.  
Facebook is a **circus** with rings.

Polygone is a **symptom**.  
The symptom of a generation that **doesn't want to be tracked at all**.  
The symptom of a world where **privacy is the new oxygen**.  
The symptom of a 14‑year‑old who wants to **build something unkillable**.

We don't compete with Apple.  
We are what comes **after** them.

---

> *« Un écosystème qui se définit par ce qu'il refuse »*  
> — La règle non écrite du projet

