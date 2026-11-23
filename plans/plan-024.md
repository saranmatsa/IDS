## Phase 024 — Create the ids-lease crate: Time-bounded authority to prevent split-brain.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

## 1. Executive Summary

Phase 024 creates a **new separate crate `ids-lease`** (not folders inside an existing crate), organized into subfolders by subsystem. `ids-lease` implements time-bounded authority grants to prevent split-brain in the IDS distributed system: a lease is a time-limited grant of exclusive authority (to be leader, to own a key range, to write to a resource) that auto-expires if not renewed, so a crashed, paused, or partitioned holder cannot block the system indefinitely.

The crate enforces safety through **fencing tokens** (monotonically increasing per-grant sequence numbers checked by the protected resource), **lease expiry with guard periods** (the holder stops acting before the grantor considers the lease expired), **leadership epochs** (term-based authority that invalidates stale holders), and **STONITH integration** (fencing the previous holder before any failover). All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`); the crate is carrier-agnostic and depends on `ids-000-aa` (master crate holding all canonical types and sealed trait contracts, including `trait Lease` and the `fencing_stonith` invariant) and `ids-carrier`.

Key engineering decisions:
- **New crate `ids-lease`**, separate from `ids-000-aa` and all other crates, per the IDS principle of separate crates over folders.
- **Lease grant / renewal / expiry** as core operations, each carrying a monotonically increasing fencing token issued by the grantor.
- **Guard period**: holder treats lease as expiring `T − ε` (safety margin for clock drift and scheduling pauses), eliminating the split-brain window between holder-expiry and grantor-expiry.
- **Epoch-based authority**: every lease grant increments a leadership epoch; the protected resource rejects any operation carrying a stale epoch.
- **STONITH/fencing before failover**: the new holder must confirm the old holder is fenced (powered down or isolated) before assuming active authority.
- **Premature leadership loss prevention**: renewal scheduled at `⅓ of TTL` with jitter, exponential backoff retries, and an immediate step-down on renewal failure.
- **Carrier-agnostic**: zero `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh; all frames carried via `ids-carrier::Endpoint`.
- **Allowed crypto only**: ed25519 (lease signatures, fencing token authentication), xchacha20poly1305 / chacha20poly1305 (lease payload encryption), sha256 and blake3 (lease content addressing, epoch commitment digests).
- **ids-000-aa** holds `trait Lease` (sealed contract), `LeaseId`, `FencingToken`, `Epoch`, `LeaseState`, and the `fencing_stonith` invariant; `ids-lease` implements the contract.

---

## 2. Architecture Overview

`ids-lease` sits above `ids-carrier` and below the application / coordinator layer, bridging the gap between "who is leader" (decided by consensus or a lease grantor) and "who may act" (enforced by fencing at the resource). It never opens sockets or speaks IP/TCP/UDP directly; it produces `Frame` payloads shipped as `Vec<u8>` over `ids-carrier::Endpoint`.

```
┌─────────────────────────────────────────────────────────────────┐
│                    APPLICATION / COORDINATOR                    │
│  leader election, key-range ownership, write authorization      │
├─────────────────────────────────────────────────────────────────┤
│                    Phase 024: ids-lease (NEW CRATE)             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  lease grant / renewal / expiry                         │   │
│  │  fencing token issuance & monotonic increment           │   │
│  │  epoch-based authority (term number)                    │   │
│  │  guard period (T − ε) + renewal scheduler (TTL/3)       │   │
│  │  premature leadership loss detection & step-down        │   │
│  │  STONITH/fencing integration hook                       │   │
│  └─────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  persistence: lease journal, epoch watermark,           │   │
│  │  fencing-token high-water mark (durable)                │   │
│  └─────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  wire: signed lease grants (ed25519),                   │   │
│  │  encrypted payloads (xchacha20poly1305/chacha20poly1305)│   │
│  │  content hashing (sha256 / blake3)                      │   │
│  └─────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│             ids-carrier (Endpoint = CarrierType + Vec<u8>)      │
│  opaque datagram transport; no IP/TCP/UDP/QUIC/HTTP/DNS/etc.    │
├─────────────────────────────────────────────────────────────────┤
│             ids-000-aa (master crate, sealed contracts)         │
│  trait Lease, LeaseId, FencingToken, Epoch, fencing_stonith     │
└─────────────────────────────────────────────────────────────────┘
```

**Data flow of a lease lifecycle:**
1. **Grant**: holder requests a lease from the grantor (a quorum-backed lease service or the current coordinator). Grantor checks no live lease exists (quorum intersection), issues `LeaseId`, `FencingToken = last_token + 1`, `Epoch = last_epoch + 1`, TTL, and expiry `now + TTL` on grantor's monotonic clock. Grant is signed (ed25519) and optionally encrypted.
2. **Holder activation**: holder records the lease, sets its local deadline to `T − ε` (guard period), and begins acting as authority. Every protected operation carries `(lease_id, fencing_token, epoch)`.
3. **Renewal**: holder renews at `TTL/3` with jitter, exponential backoff on failure. Each renewal re-issues or extends the fencing token and epoch.
4. **Expiry / step-down**: if renewal fails for `RenewDeadline` (typically `TTL − ε`), holder steps down immediately and stops admitting new work. If the holder is paused/partitioned and its lease expires, the grantor may grant to a new holder with a higher token/epoch.
5. **Fencing at the resource**: the protected resource persists the highest fencing token/epoch seen and rejects any operation carrying a lower or equal value — this is the safety backstop even if the old holder is still alive and unaware.
6. **STONITH**: before the new holder assumes active write authority, it confirms the previous holder is fenced (isolated / powered down) per the `fencing_stonith` invariant in ids-000-aa.

**Split-brain prevention argument:** quorum-backed grant ensures at most one valid lease per resource at any instant; guard period ensures the holder stops before the grantor can reassign; fencing tokens ensure even a paused holder that wakes late cannot overwrite the new holder's writes.

---

## 3. Detailed Research Dimensions

### 3.1 Lease fundamentals (time-bounded authority)

A lease is a lock with a built-in expiration: the holder may act as owner only until the lease expires, and the grantor refuses conflicting grants until its exclusion interval ends. Unlike an indefinite lock, a crashed holder's lease auto-expires, converting a liveness problem (waiting for a dead client to release) into a bounded performance problem (unavailable for at most the lease duration). (Sources: sanzgiri/patterns lease block; Aicassindra leases article; Martin Kleppmann DDIA process pauses; lease definition literature.)

**TTL trade-offs** (sanzgiri patterns):
- Short TTL (1–5 s): fast recovery; aggressive heartbeating; sensitive to network blips (false expiry).
- Medium TTL (5–30 s): production sweet spot; tolerates ordinary GC pauses and brief network issues.
- Long TTL (5+ min): stable, low overhead; long unavailability window on real holder death.

### 3.2 Grant / renew / expire mechanics

- **Grant**: atomic compare-and-set at the grantor (create-if-not-exists or CAS on an expired lease). Exactly one grantor (quorum-backed) can issue a live lease for a resource at any time.
- **Renew**: holder sends `KeepAlive`/`Renew` before expiry; grantor extends TTL. etcd uses a bidirectional keep-alive stream; Kubernetes Lease renews `renewTime` every `leaseDurationSeconds / 2`; Consul renews sessions periodically with a default 15 s TTL.
- **Expire**: no renewal within TTL → grantor releases automatically; attached state (lock, leadership, cached data) is invalidated. Chubby: session expiry invalidates cached copies; Consul: session invalidation releases or deletes held locks.
- **Revoke**: explicit early termination by grantor or operator; holder must step down immediately.

### 3.3 Fencing tokens (monotonic per-grant sequence)

Each grant returns a strictly increasing fencing token. The protected resource persists the highest token seen and rejects any operation carrying a lower/equal token. This moves safety from the holder (which may be paused and unaware) to the resource (which enforces ordering independently). etcd revision numbers and ZooKeeper zxids serve as natural fencing tokens; Chubby provides sequence numbers; DynamoDB lock client generates monotonic tokens. Without fencing tokens, Redlock-style schemes are unsafe under pauses/partitions (Kleppmann critique). (Sources: sanzgiri fencing; systemoverflow fencing tokens; Kleppmann Redlock analysis; redis distributed locks; various fencing-token explanations.)

### 3.4 Epoch / term-based authority

Every leadership term carries a monotonically increasing epoch (Raft "term", Paxos ballot, Chubby master epoch). A new leader invalidates all prior epochs; followers and storage reject operations tagged with stale epochs. Raft terms: each server stores `current_term`; votes only once per term; at most one winner per term (election safety). OpenRaft CheckQuorum: leader steps down if it cannot reach a quorum within `leader_lease`. LeaseGuard: new leader defers commits until the deposed leader's prior-term entry is `Δ` old. (Sources: Raft paper/lectures; CockroachDB replication layer; openraft check_quorum; LeaseGuard arxiv 2512.15659.)

### 3.5 Guard period and clock safety

The holder assumes its lease expires at `T − ε` (safety margin) so it stops before the grantor is entitled to reassign. Spanner TrueTime gives bounded-uncertainty timestamps; classical leases assume bounded clock drift. Monotonic clocks (not wall-clock) measure lease durations — wall-clock NTP steps backwards have caused real split-brain (2-second NTP step during VM migration gave node A 2 extra seconds of lease, overlapping with node B's lease). LeaseGuard requires clocks with bounded uncertainty for inherited-lease reads; timers suffice for deferred-commit writes. (Sources: sanzgiri clock-skew danger; michal-drozd clock step split-brain; distributedrequest clock skew; empty squa.re timers; arxiv PaxosLease relativistic frames.)

### 3.6 Renewal scheduling and failure modes

- **Renewal cadence**: `TTL/3` is a common convention (one failed renewal is survivable, two indicate real loss). Kubernetes leaderelection: retry period with jitter; `RenewDeadline` < `LeaseDuration`.
- **Jitter**: randomizes renewal times to avoid thundering herds (ExpressVPN incident: fixed 5 s interval across 12 k leases overwhelmed a single etcd node during a partition; adding 100–500 ms jitter cut expiry events by 99.8%).
- **Backoff**: exponential backoff on renewal retry (1 s, 2 s, 4 s, capped at 5 s).
- **Step-down**: on renewal failure for `RenewDeadline`, holder stops acting immediately; does not wait for lease expiry.
- **Pause resilience**: a long GC pause or VM freeze can outlive the lease; the holder must not resume writes after waking without re-verifying authority (fencing tokens catch this at the resource).
- **Clock jumps**: NTP step backwards extends apparent lease validity; forward steps expire leases early. Monotonic time for durations; wall-clock only for external display. (Sources: etcd lease.go; kubernetes leaderelection.go; consul sessions; ExpressVPN incident; aerospike jitter article; systemoverflow critical failure modes.)

### 3.7 STONITH / fencing before failover

STONITH ("Shoot The Other Node In The Head") isolates a suspected failed node (power off / reboot) so it cannot access shared storage. Pacemaker/SUSE/RedHat HA: STONITH must never report false positives; reset must complete before returning control. Linux-HA: a STONITH plugin that reports a node down must be correct. Chubby master failover: new master generates a new epoch larger than all previous, waits for existing sessions to expire or fail-over messages to be acked, then responds to operations. Consul lock-delay (default 15 s) gives the potentially-still-live leader time to detect invalidation and stop. (Sources: Wikipedia fencing/STONITH; Oracle fencing config; RedHat Pacemaker STONITH; Linux-HA STONITH; SUSE fencing; Chubby papers/slides; Consul sessions internals.)

### 3.8 Premature leadership loss prevention

A leader that loses its lease must step down before a new leader is elected, so their active windows never overlap. Timeline (Kubernetes one-nought-one module): T=0 partition; T=10 leader attempts renewal, cannot reach service; T=25 retries exhausted, enters guard period, stops acting; T=30 lease expires at service; T=31 new holder acquires. Key insight: old holder stops **before** new holder is elected → never two active leaders. Recovery: when partition heals, old holder discovers it is no longer leader and becomes follower. (Sources: onenoughtone leader election lease-based; various leader-lease explanations.)

### 3.9 Quorum-backed grant (split-brain prevention at the grantor)

At most one partition can hold a majority; quorum intersection (any two majorities share ≥1 node) prevents two independent grants for the same resource. Chubby: Paxos elects master for a lease period; new master accepted only if old master's lease has expired. Raft: majority vote elects one leader per term. PaxosLease: quorum of acceptors grants time-bounded ownership; no durable acceptor lease state; quarantine after acceptor restart. (Sources: Chubby slides/papers; Raft paper; Rutgers consensus notes; arxiv PaxosLease revisited 2609.14640.)

### 3.10 did:crdt / signed deltas relevance

did:crdt (arxiv 2606.16223): signed CRDT deltas with HLC timestamps, causal delivery, Ed25519 signatures over canonical content, BLAKE3-256 content addressing — a model for authenticated, carrier-agnostic authority transfers that `ids-lease` can mirror for signed lease grants. Decoupling Trust in Byzantine CRDTs (arxiv 2606.31759): decouples identity trust from content trust; post-compromise handling without breaking causality — relevant when a lease holder's key is compromised mid-lease. (Sources: did:crdt arxiv; Decoupling Trust arxiv.)

---

## 4. Threat Model & Mitigations

| Threat | Mitigation |
|--------|-----------|
| Split brain: two holders believe they have valid leases simultaneously | Quorum-backed grant (at most one grantor, quorum intersection); guard period `T − ε`; fencing tokens checked by resource |
| Stale holder resumes after pause/partition (zombie leader) | Fencing token: resource rejects writes with token < highest seen; epoch check |
| Clock skew between holder and grantor | Holder uses `T − ε`; monotonic clocks for durations; never trust wall-clock for lease arithmetic |
| NTP step backwards extends apparent lease | Monotonic time; alert on clock steps > 50 ms; fencing tokens as backstop |
| Long GC pause / VM freeze outlives lease | Renewal at TTL/3 with jitter; step-down on renewal failure; fencing at resource |
| Renewal thundering herd overwhelms grantor | Jitter (10–20% of interval, capped 500 ms); exponential backoff; circuit breakers |
| Grantor unavailability during partition | Holder steps down after `RenewDeadline`; no new grants until quorum heals |
| STONITH false positive (isolating a live node) | STONITH must never report false positives; reset confirmed before control returns; lock-delay grace window |
| Revoked key / compromised holder mid-lease | Epoch-based revocation; new grant invalidates old epoch; did:crdt-style decoupling of content/identity trust |
| Replay of old lease grant messages | Nonce + per-grant fencing token; grantor rejects duplicate/out-of-order grants; signed grants prevent forgery |
| Carrier-level interception / injection | All lease traffic signed (ed25519) and optionally encrypted (xchacha20poly1305/chacha20poly1305); carrier provides no security (per I-CARR-016/I-CARR-018) |

---

## 5. Integration with Existing Crates

`ids-lease` depends on and integrates with:

| Crate | Role in Phase 024 |
|-------|-------------------|
| `ids-000-aa` | Master crate: `trait Lease` (sealed contract), `LeaseId`, `FencingToken`, `Epoch`, `LeaseState`, `Endpoint`/`CarrierType` re-exported via `ids-carrier`, `fencing_stonith` invariant, sealed error contracts |
| `ids-carrier` | `Endpoint` (`CarrierType` + `Vec<u8>`); all lease traffic rides opaque datagrams over `Endpoint`; no direct carrier dependency |
| `ids-coordinator` (domain-scoped leadership, fencing/STONITH) | Consumer: lease grants back domain leadership; STONITH hook before failover |
| `ids-storage` | Durable persistence of lease journal, epoch watermark, fencing-token high-water mark |
| `ids-crypto` | ed25519 signatures, xchacha20poly1305/chacha20poly1305 encryption, sha256/blake3 hashing (only allowed primitives) |
| `ids-membership` | Join/leave/lifecycle context for lease holders |
| `ids-quorum` | Quorum intersection for grant safety |
| `ids-broadcast` | Reliable/causal broadcast of lease grants and revocations |

**Integration points:**
- `ids-coordinator` consumes `ids-lease` for domain-scoped leadership: a domain leader holds a lease; on lease loss it steps down and triggers STONITH/fencing before a new leader assumes authority.
- `ids-storage` persists the lease journal, epoch high-water mark, and fencing-token high-water mark durably so a restarted holder/grantor cannot regress tokens.
- All lease traffic is `ids-carrier::Endpoint` frames (`CarrierType` + `Vec<u8>`); `ids-lease` never imports `std::net`, `tokio::net`, libp2p, or any carrier-specific crate.
- No dependency on libp2p/TCP/IP/QUIC/HTTP/DNS — carrier-agnostic by design (I-CARR-001).

---

## 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-lease` (new crate, depends on `ids-000-aa`, `ids-carrier`, `ids-crypto`, `ids-storage`; ed25519-dalek, xchacha20poly1305, chacha20poly1305, sha2, blake3, thiserror, serde, prost/cbor for wire encoding)

```
ids-lease/
├── Cargo.toml
├── lib.rs                          # Crate root, feature gates, public re-exports
├── constants.rs                    # Protocol constants (TTL defaults, guard margin, renewal fraction)
├── error.rs                        # LeaseError (thiserror, comprehensive)
├── prelude.rs
│
├── grant/                          # Lease grant lifecycle
│   ├── mod.rs
│   ├── grant.rs                    # LeaseGrant: issue LeaseId, FencingToken, Epoch, TTL
│   ├── acquire.rs                  # Atomic compare-and-set acquisition at grantor
│   ├── renew.rs                    # Renewal request/response, TTL extension
│   ├── expire.rs                   # Expiry detection, automatic release
│   └── revoke.rs                   # Explicit revocation by grantor/operator
│
├── token/                          # Fencing tokens & epochs
│   ├── mod.rs
│   ├── fencing_token.rs            # Monotonically increasing FencingToken (u64)
│   ├── epoch.rs                    # Leadership Epoch (term-based authority)
│   ├── watermark.rs                # Durable high-water mark (token + epoch)
│   └── increment.rs                # Token increment logic (grantor-side)
│
├── clock/                          # Time model for leases
│   ├── mod.rs
│   ├── monotonic.rs                # Monotonic clock for lease durations (never wall-clock)
│   ├── guard.rs                    # Guard period T − ε computation
│   ├── deadline.rs                 # Local expiry deadline derived from grant
│   └── skew.rs                     # Clock-skew tolerance bounds, NTP step detection
│
├── renewal/                        # Renewal scheduler
│   ├── mod.rs
│   ├── scheduler.rs                # Renew at TTL/3 with jitter
│   ├── backoff.rs                  # Exponential backoff on renewal failure
│   ├── jitter.rs                   # Randomized jitter (10–20%, capped 500 ms)
│   ├── stepdown.rs                 # Immediate step-down on RenewDeadline expiry
│   └── heartbeat.rs                # Keep-alive heartbeat loop
│
├── stonith/                        # STONITH / fencing integration
│   ├── mod.rs
│   ├── stonith.rs                  # Fence previous holder before failover
│   ├── isolate.rs                  # Network/isolation fencing via carrier messages
│   ├── power.rs                    # Power-cycle fencing hook (plugin, carrier-agnostic)
│   └── confirm.rs                  # Confirmation that fencing completed
│
├── authority/                      # Active authority state machine
│   ├── mod.rs
│   ├── state.rs                    # AuthorityState: Acquired, Active, Jeopardy, Expired, Revoked, SteppedDown
│   ├── transitions.rs              # Valid state transitions
│   ├── act_as_leader.rs            # Guarded "may act" checks before each operation
│   └── precedence.rs               # Epoch comparison: reject stale authority
│
├── wire/                           # Carrier-agnostic wire format
│   ├── mod.rs
│   ├── frame.rs                    # LeaseFrame: opaque Vec<u8> payload for Endpoint
│   ├── grant_msg.rs                # Signed lease grant message (ed25519)
│   ├── renew_msg.rs                # Renewal request/response
│   ├── revoke_msg.rs               # Revocation message
│   └── codec.rs                    # Serialization (cbor/prost), versioned
│
├── crypto/                         # Allowed-crypto boundary
│   ├── mod.rs
│   ├── signing.rs                  # ed25519 sign/verify for grants and fencing tokens
│   ├── encryption.rs               # xchacha20poly1305 / chacha20poly1305 for payloads
│   ├── hashing.rs                  # sha256 / blake3 for content addressing, epoch commitments
│   └── nonce.rs                    # Nonce generation for replay resistance
│
├── persist/                        # Durable persistence
│   ├── mod.rs
│   ├── journal.rs                  # Append-only lease journal (grant, renew, revoke)
│   ├── epoch_store.rs              # Durable epoch high-water mark
│   ├── token_store.rs              # Durable fencing-token high-water mark
│   └── snapshot.rs                 # Periodic lease-state snapshots
│
├── resource/                       # Fencing at the protected resource
│   ├── mod.rs
│   ├── checker.rs                  # Reject operations with stale token/epoch
│   ├── register.rs                 # FencedRegister: highest_token + apply-if-newer
│   └── cas.rs                      # Compare-and-swap guarded by fencing token
│
├── quorum/                         # Quorum-backed grant safety
│   ├── mod.rs
│   ├── grantor.rs                  # Quorum-backed lease grantor (single grantor per resource)
│   ├── intersection.rs             # Quorum intersection verification
│   └── election.rs                 # Leader election for the grantor itself
│
├── recovery/                       # Lease recovery after restart/partition
│   ├── mod.rs
│   ├── replay.rs                   # Replay lease journal on restart
│   ├── restore.rs                  # Restore epoch/token watermarks
│   └── regrant.rs                  # Safe re-grant after grantor restart (quarantine interval)
│
├── test/                           # Property & convergence tests (not shipped)
│   ├── mod.rs
│   ├── split_brain.rs              # Simulated partitions, verify at most one active holder
│   ├── fencing.rs                  # Stale-token rejection at resource
│   ├── renewal.rs                  # Renewal/backoff/jitter behavior
│   ├── clock.rs                    # Clock-skew and NTP-step resilience
│   └── stonith.rs                  # STONITH-before-failover ordering
│
├── benches/                        # Performance benchmarks
│   ├── grant_latency.rs
│   ├── renewal_throughput.rs
│   └── fencing_check_cost.rs
│
├── docs/
│   ├── LEASE_PROTOCOL.md
│   ├── FENCING_TOKEN_MODEL.md
│   └── STONITH_INTEGRATION.md
│
└── examples/
    ├── leader_lease_example.rs
    └── keyrange_lease_example.rs
```

Target scale: ~300–500 source files across subfolders, production-grade, matching the ~90–1000 file subfolder structure directive.

---

## 7. Key Algorithms & Data Structures

**LeaseGrant** = `{ lease_id: LeaseId, holder: NodeId, fencing_token: FencingToken, epoch: Epoch, ttl_ms: u64, granted_at: Instant, expires_at: Instant, signature: ed25519::Signature }`

**FencingToken / Epoch**: monotonic `u64` counters incremented on every grant at the grantor; persisted durably (high-water mark) so restarts cannot regress.

**Guard period**: `local_deadline = grant_received_at + ttl − ε` where `ε` covers clock drift + scheduling pause + message delay. Holder stops acting at `local_deadline`, not at grantor's `expires_at`.

**Renewal scheduler**: fire at `ttl / 3` + jitter; on failure, exponential backoff (1s → 2s → 4s, cap 5s); if `RenewDeadline` (typically `ttl − ε`) elapses without success → `step_down()` immediately.

**FencedRegister** (at protected resource): `highest_token: u64`; `write(token, op)` → reject if `token <= highest_token`; else `highest_token = token; apply(op)`. Atomic check-and-apply prevents TOCTOU.

**Authority state machine**: `Acquired → Active → Jeopardy (renewal failing) → Expired | Revoked | SteppedDown`; transitions only on valid grant/renew/revoke/stepdown events.

**Quorum-backed grant**: grantor accepts a new grant only if (a) no live lease exists, or (b) the existing lease's expiry has passed; quorum intersection ensures at most one grantor can decide this consistently.

**STONITH-before-failover**: new holder confirms previous holder fenced (isolation confirmed via carrier messages, power-cycle confirmed via plugin) before entering `Active` write state.

---

## 8. Dependencies

**Crate dependencies:**
- `ids-000-aa` — canonical types, sealed `trait Lease` contract, `fencing_stonith` invariant, error contracts
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`), opaque datagram transport
- `ids-crypto` — ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 (only allowed primitives)
- `ids-storage` — durable lease journal, epoch/token watermarks

**External crypto dependencies (allowed set only):**
- `ed25519-dalek` — ed25519 signatures for lease grants and fencing tokens
- `xchacha20poly1305` / `chacha20poly1305` — lease payload encryption
- `sha2` — sha256 content addressing
- `blake3` — blake3 hashing, epoch commitments

**Standard library only for everything else** — no `std::net`, no `tokio::net`, no networking crates.

**Dev dependencies:** proptest/bolero for property tests, criterion for benchmarks (test-only, not in production build).

---

## 9. Measurable Acceptance Criteria

1. **Exclusivity:** At most one valid lease per resource at any instant (quorum-backed grant); verified by split-brain simulation test: 0 split-brain events across 5000 simulated rounds with partitions/pauses (cf. PRIMUS validation: 0.0 vs 794.5 without fencing).
2. **Fencing:** Resource rejects every operation carrying `fencing_token <= highest_seen`; stale-holder write rejected even if holder is still alive and unaware (unit test: token 41 rejected after token 42 accepted).
3. **Guard period:** Holder stops acting at `T − ε` before grantor's `T`; verified by clock-skew test (±5 s steps, no split-brain).
4. **Renewal:** Renewal fires at `TTL/3` + jitter; jitter spread ≥ 100 ms across concurrent leases (anti-thundering-herd); exponential backoff on failure.
5. **Step-down:** On `RenewDeadline` expiry without successful renewal, holder stops admitting new work within one tick (no waiting for lease expiry).
6. **Epoch invalidation:** New grant with `epoch = n+1` invalidates all operations tagged `epoch <= n`; storage rejects stale-epoch writes.
7. **STONITH ordering:** New holder enters `Active` write state only after confirming previous holder fenced; verified by ordering test (fencing confirmed before first write).
8. **Expiry auto-release:** Crashed holder's lease expires after TTL without renewal; another holder can acquire (no manual intervention).
9. **Carrier agnosticism:** `ids-lease` compiles and behaves identically with zero direct dependencies on TCP/IP, UDP, QUIC, HTTP, DNS, libp2p, Wi-Fi, Bluetooth, cellular, VPN, or mesh; all traffic via `ids-carrier::Endpoint`.
10. **Crypto compliance:** Only ed25519, xchacha20poly1305, chacha20poly1305, sha256, blake3 used; no other cryptographic primitives.
11. **Durability:** Epoch and fencing-token high-water marks survive grantor/holder restart; no token regression after restart (quarantine interval applied).
12. **Performance:** Grant + sign + verify ≤ 1 ms on reference hardware; fencing-token check is a single integer comparison per operation.

---

## 10. Remaining Uncertainties

1. **STONITH implementation mechanism:** The crate defines the fencing hook and ordering guarantee, but the concrete fencing action (power-cycle via carrier message vs. hardware plugin) is deployment-specific and deferred to the STONITH plugin layer.
2. **Grantor election:** `ids-lease` assumes a quorum-backed grantor per resource; electing the grantor itself may reuse a separate consensus crate (to be determined).
3. **ε sizing:** The guard-period margin must be derived from measured clock drift and pause distributions; `TTL/3` renewal is a convention, not a proof.
4. **Lease duration policy:** TTL values (short/medium/long) are workload-dependent; no single default is mandated by this plan.
5. **Revocation propagation latency:** Epoch revocation takes effect after causal convergence; the acceptable compromise window is unquantified.
6. **Offline lease allowance:** Preallocated offline authority for partitioned holders (bounded offline finality) is a possible extension, not in scope.
7. **Fencing-token persistence crash window:** If the resource crashes after accepting token `n` but before durably recording `n+1`, a stale `n` could be re-accepted; write-ahead logging of the high-water mark mitigates but adds latency.

---

## 11. Engineering Decisions Summary

| Decision | Rationale |
|----------|-----------|
| New separate crate `ids-lease` (not folders in `ids-000-aa`) | IDS principle: separate crates over folders; `ids-000-aa` holds only the sealed `trait Lease` contract |
| Fencing tokens on every grant | Safety backstop independent of holder awareness; moves enforcement to the resource |
| Guard period `T − ε` on holder side | Holder stops before grantor can reassign; eliminates split-brain window |
| Monotonic clocks for durations, never wall-clock | Wall-clock NTP steps have caused real split-brain; monotonic is immune |
| Renewal at `TTL/3` + jitter + exponential backoff | One failed renewal survivable; jitter prevents thundering herd |
| Immediate step-down on `RenewDeadline` | Old holder stops before new holder elected → never two active leaders |
| STONITH/fencing before failover | Confirms old holder isolated before new holder writes; per `fencing_stonith` invariant |
| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic; no `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh |
| Crypto limited to ed25519 / xchacha20poly1305 / chacha20poly1305 / sha256 / blake3 | Per IDS allowed-crypto policy |
| Epoch-based authority invalidation | New grant invalidates stale holders; term-based authority like Raft terms |
| Durable high-water marks for token + epoch | Prevents token regression after restart |

---

## 12. Sources

- sanzgiri/patterns — lease block (lease ID, TTL, fencing token, guard period `T − ε`, leader leases): https://github.com/sanzgiri/patterns/blob/main/reference/block/lease.md
- onenoughtone — Leader Election lease-based module (renewal at TTL/3, guard period, split-brain timeline): https://onenoughtone.com/learning-path/system-design-hld/learning/leader-election/4
- algoroq — Split Brain Mitigation (quorum, fencing, leases, CRDTs): https://algoroq.io/learn/system-design-advanced/split-brain-mitigation
- arxiv:2609.07910 — PRIMUS: leases with fencing, 0.0 split-brain rounds with fencing vs 794.5 without: https://arxiv.org/html/2609.07910
- vabs.github.io — Database failover and split-brain prevention (lease TTL + clock-skew margin, promote after old lease certainly expired): https://vabs.github.io/2026/04/24/database-failover-and-split-brain-prevention
- systemoverflow — Fencing Tokens: Preventing Split Brain Operations (monotonic token, resource rejects stale): https://systemoverflow.com/learn/distributed-primitives/distributed-locks/fencing-tokens-preventing-split-brain-operations
- glycerine/rpc25519 — fencing.go (Chubby/Etcd/Zookeeper fencing tokens, zombie leader): https://github.com/glycerine/rpc25519/blob/v1.44.4/tube/fencing.go
- systemoverflow — Preventing Split Brain: Quorums, Fencing, and Epochs (epoch numbers persisted and validated): https://systemoverflow.com/learn/distributed-primitives/leader-election/preventing-split-brain-quorums-fencing-and-epochs
- Kubernetes — Coordinated Leader Election (Lease API, holderIdentity, renewTime, leaseDurationSeconds, optimistic concurrency): https://github.com/kubernetes/website/blob/main/content/en/docs/concepts/cluster-administration/coordinated-leader-election.md
- Kubernetes client-go — leaderelection.go (RenewDeadline < LeaseDuration, jitter, retry period, clock-skew tolerance ratio): https://raw.githubusercontent.com/kubernetes/client-go/master/tools/leaderelection/leaderelection.go
- Consul — Sessions internals (TTL sessions, lock-delay default 15 s, release/delete behavior, Chubby-inspired): https://consul.io/docs/internals/sessions
- Chubby — Burrows 2006 (Paxos + master lease, session keep-alives, failover with new epoch): https://mwhittaker.github.io/papers/html/burrows2006chubby.html
- Chubby slides — Rutgers 417 (master lease several seconds, re-run master selection after lease expiry): https://pk.org/assets/files/rutgers-417-notes/06a-chubby-slides.pdf
- etcd — client/v3/lease.go (KeepAlive, TTL, revoke, time-to-live): https://github.com/etcd-io/etcd/blob/main/client/v3/lease.go
- etcd docs — Lease API (keep-alive stream, TTL expiration deletes attached keys): https://etcd.io/docs/v3.7/learning/api
- Raft — Ongaro/Ousterhout (terms, majority vote, at most one winner per term, heartbeats): https://raft.github.io/raft.pdf
- CockroachDB — Replication Layer (leaseholder, leader leases, epoch-based leases, leader-leaseholder split): https://cockroachlabs.com/docs/v25.4/architecture/replication-layer
- openraft — CheckQuorum (leader steps down without quorum, `now < last_quorum_acked + leader_lease`): https://docs.rs/openraft/latest/openraft/docs/protocol/check_quorum
- LeaseGuard — arxiv:2512.15659 (deferred commit, lease Δ, inherited lease reads need bounded clock uncertainty): https://arxiv.org/html/2512.15659v2
- Kleppmann DDIA — Process pauses (GC pause outlives lease, fencing tokens required): https://ebrary.net/64828/computer_science/process_pauses
- michal-drozd — Split-brain from clock step backwards (2-second NTP step, monotonic time): https://michal-drozd.com/en/blog/clock-step-backwards-split-brain
- empty squa.re — Timers in distributed algorithms (Timer Rule, ε for clock drift): https://emptysqua.re/blog/timers-distributed-algorithms
- distributedrequest — Clock skew & time ordering (monotonic vs wall clock, NTP steps): https://distributedrequest.com/distributed-coordination-locking-strategies/clock-skew-and-time-ordering-in-distributed-systems
- ExpressVPN incident — Lease renewal thundering herd, jitter fix (100–500 ms jitter, 99.8% expiry reduction): https://johal.in/incident-timeline-expressvpn-our-experience-incident
- aerospike — What Is Jitter (retry jitter, full/equal/decorrelated jitter): https://aerospike.com/blog/what-is-jitter
- Wikipedia — STONITH / Fencing (compute): https://en.wikipedia.org/wiki/STONITH
- Oracle — Configuring Fencing (stonith, device ensures node offline): https://docs.oracle.com/en-us/iaas/oracle-linux/availability/availability-configuring-fencing-stonith.htm
- RedHat — Pacemaker STONITH (fence failed nodes): https://docs.redhat.com/en/documentation/red_hat_openstack_platform/16.0/html/high_availability_deployment_and_usage/assembly_fencing
- Linux-HA — STONITH (never false positives, reset completes before return): http://linux-ha.org/STONITH
- SUSE — Fencing and STONITH: https://documentation.suse.com/sle-ha/12-SP5/html/SLE-HA-all/cha-ha-fencing.html
- Redis — Distributed locks (SET NX PX, Redlock, fencing tokens recommended): https://redis.io/docs/latest/develop/clients/patterns/distributed-locks
- Kleppmann — How to do distributed locking (Redlock critique, no fencing tokens = unsafe): https://pages.cs.wisc.edu/~remzi/Classes/739/Fall2018/Papers/leases-redis-problem.pdf
- etcfs — Kleppmann's Stale-Write Hazard Analysis (fencing token checked by storage): https://etcfs.github.io/etcfs-docs/architecture/storage/kleppmann-stale-write-analysis
- primitives.pub — Fencing Tokens and Lock Safety (highest_token durably persisted): https://primitives.pub/distributed-systems/monographs/fencing-tokens
- arxiv:2609.14640 — PaxosLease Revisited (quorum grants time-bounded ownership, diskless, quarantine on restart): https://arxiv.org/html/2609.14640v1
- arxiv:2606.16223 — did:crdt (signed CRDT deltas, HLC, Ed25519, BLAKE3-256, causal delivery): https://arxiv.org/html/2606.16223v1
- arxiv:2606.31759 — Decoupling Trust in Byzantine CRDTs (content vs identity trust, post-compromise): https://arxiv.org/html/2606.31759v1
- arxiv:2609.21284 — Authorization Revocation for Long-Running AI Agents (time-bounded leases, Gray and Cheriton 1989): https://arxiv.org/html/2609.21284
- Rutgers — Consensus and Replicated State Machines (split-brain, quorum majority): https://people.cs.rutgers.edu/~pxk/classes/417/notes/consensus.html
- systemoverflow — What are Distributed Locks and Why Do They Need Leases (Chubby 10–12 s sessions, keepalives): https://systemoverflow.com/learn/distributed-primitives/distributed-locks/what-are-distributed-locks-and-why-do-they-need-leases
- systemoverflow — Critical Failure Modes and Mitigation Strategies (TTL headroom, renewal at 25–33% of TTL, jittered backoff): https://systemoverflow.com/learn/distributed-primitives/distributed-locks/critical-failure-modes-and-mitigation-strategies

**Constraint compliance verified:**
- New separate crate `ids-lease` (not folders in existing crates): yes.
- Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh; all traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`): yes.
- Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305: yes.
- `ids-000-aa` is the master crate with all canonical types and sealed trait contracts (`trait Lease`, `fencing_stonith` invariant): yes.
- Research only — no code, no source modifications, only the plan document: yes.
- Plan matches HX-241 plan format (Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure, Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources): yes.
- Output file: `C:/idfs/plans/plan-024.md`: yes.