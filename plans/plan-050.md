## Phase 050 — Create the ids-session crate: Client-session binding and state.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 745-759)

```
50. Research only do not read plan.md **[Phase 050 — Create the ids-session crate: Client-session binding and state.]**

Crate: ids-session
Purpose: Client-session binding and state.
Contents: Session creation, session expiration, session key management, session state machine, session recovery, client affinity, session stickiness, session token management, session replay protection.

try to use seperate crates instead of more folders in existing onesResearch only do not read plan.md research **[Phase 050 — Create the ids-session crate: Client-session binding and state.]**

Create plan-050.md in C:/idfs/plans/ for the ids-session crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.
```

---

### 1. Executive Summary

Phase 050 creates `ids-session`, the IDS crate that owns client-session binding and session state. It provides session creation, session expiration (idle + absolute timeouts), session key management, a session state machine, session recovery after disruption, client affinity, session stickiness, session token management, and session replay protection — all carrier-agnostic. `ids-session` does not open sockets or drive any transport directly; it binds a client to a session context, issues and validates session tokens, derives session keys, enforces a finite-state session lifecycle, and ships every protection-relevant message as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).

Sessions are modeled as a small finite-state machine (Created → Bound → Active → [Idle → Expired | Suspended → Resuming → Active | Terminated]) keyed by a session token. Each session carries an ephemeral client-bound key pair (ed25519) used for proof-of-possession binding: every session-bound message carries an ed25519 signature proving the presenter holds the session's private key, preventing session-token theft and replay from a different client. Tokens are opaque random values (>=128 bits entropy per NIST SP 800-63B / OWASP ASVS 3.2.2) with a separate verification secret; revocation uses a bounded denylist (token rotation lineage or jti-based) checked server-side.

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint`. Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-session` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-session` crate does NOT yet exist at `C:/idfs/ids-session/`. `ids-000-aa` already declares canonical types referenced by this crate but never redefined: `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, plus sealed trait contracts and error types. `ids-000-aa` is the master crate holding every canonical type and sealed trait contract. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned OWASP Session Management Cheat Sheet (idle/absolute/renewal timeouts, ≥64 bits entropy session IDs, CSPRNG generation, server-side enforcement); NIST SP 800-63B Session Management (session secret ≥64 bits, opaque string, directly presented or cryptographically proven possession); OWASP ASVS V3 Session Management (3.2.2 ≥64 bits entropy; timeout aligned with NIST 800-63); MDN Web Docs session management (idle/absolute/renewal timeouts, 64-bit entropy minimum); OWASP WSTG Session Timeout testing (server-side idle/absolute timeouts, replay of old session ID must be rejected); RFC 4303 IPsec ESP anti-replay (per-SA 32-bit sequence number, sliding-window replay detection); RFC 8705 / RFC 9449 OAuth DPoP (proof-of-possession token binding to client key, `jti` replay detection, token substitution prevention); RFC 9700 OAuth 2.0 Security BCP (refresh token rotation with reuse detection OR sender-constraining via DPoP/mTLS); Device Bound Session Credentials (DBSC, W3C/webappsec-dbsc; Chrome origin trial) — hardware-bound private key proving session possession, short-lived cookies, proof on refresh; Hashicorp/Traefik/Envoy sticky sessions and consistent hashing (Ringhash/MagLev) for client affinity across backend churn; Socket.IO connection state recovery (session ID + last offset replay on reconnect, maxDisconnectionDuration, missed-packet restoration); WebSocket reconnection state sync (message sequence numbers, last-received tracking, gap replay); Signal double ratchet and Noise protocol handshake patterns (XX/IK/NK) for session key establishment and key evolution; EDHOC RFC 9528 + OSCORE RFC 8613 + ACE EDHOC/OSCORE profile (lightweight authenticated key exchange, OSCORE security context, replay protection via sequence numbers); DTLS 1.3 anti-replay (sliding-window bitmap per epoch, sequence-numbered records); mTLS / TLS-session-bound access tokens (binding tokens to a specific mTLS connection via exporter value); JWT blacklist/denylist patterns (Bloom filter, Redis TTL, streaming propagation, jti revocation); IPsec ESP replay window (RFC 4303 Section 3.4.3 sliding window).

---

### 2. Architecture Overview

`ids-session` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each concern — session creation, expiration, key management, state machine, recovery, affinity/stickiness, token management, replay protection — is an independent module. `ids-session` is a binding-and-state layer: it does not store application payloads itself; it issues session tokens, binds sessions to client keys, enforces the session lifecycle, and gates all session-bound traffic before it reaches `ids-carrier::Endpoint`.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                    APPLICATION / ids-* LAYER (session users)                 │
│  ids-store, ids-fragment, ids-replication — open/use sessions on a client   │
├──────────────────────────────────────────────────────────────────────────────┤
│                 Phase 050: ids-session (THIS CRATE — binding & state)        │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │ │
│  │ session        │ │ session        │ │ session key    │ │ session      │ │ │
│  │ creation       │ │ expiration     │ │ management     │ │ state machine│ │ │
│  │ (token issue,  │ │ (idle/absolute │ │ (ed25519 client│ │ (Created→Bound│ │ │
│  │  binding key)  │ │  timeout,     │ │  bound key, PoP │ │  →Active→...│ │ │
│  └────────────────┘ └────────────────┘ └────────────────┘ └──────────────┘ │ │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │ │
│  │ session        │ │ client affinity│ │ session token  │ │ session      │ │ │
│  │ recovery       │ │ / stickiness   │ │ management     │ │ replay       │ │ │
│  │ (resume after  │ │ (consistent    │ │ (opaque token, │ │ protection   │ │ │
│  │  disruption)   │ │  hash → node)  │ │  rotation,     │ │ (sequence+   │ │ │
│  └────────────────┘ └────────────────┘ └────────────────┘ └─nonce window)│ │ │
│  ┌────────────────┐ ┌────────────────┐                   │                │ │ │
│  │ session        │ │ audit/logging │                   │                │ │ │
│  │ contracts +    │ │ (every decision│                   │                │ │ │
│  │ invariants     │ │  signed+chained│                   │                │ │ │
│  └────────────────┘ └────────────────┘                   │                │ │ │
└──────────────────────────────────────────────────────────┬──────────────────┘
                                                             │ opaque Vec<u8> datagrams
┌───────────────────────────────────────────────────────────▼──────────────────┐
│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │
│  send_datagram(Endpoint{CarrierType≤1024B addr, Vec<u8>}, payload: &[u8])  │
│  receive_datagram → (Endpoint, Vec<u8>)                                    │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Data flow of session creation:** client submits a session-request datagram to the node → `ids-session` generates a client-bound ed25519 key pair (ephemeral), issues a session token (opaque random ≥128 bits entropy), assigns a `SessionId`, initializes the session state machine in `Created`, binds the token to the client key, sets idle/absolute deadlines against monotonic `IdsTime`, returns `SessionCreateResponse{session_id, token, client_pubkey, expires_at}` as an opaque datagram to `ids-carrier::Endpoint`.

**Data flow of a session-bound message:** client sends `SessionMessage{session_id, token, seq, nonce, payload, sig}` where `sig` = ed25519 over `(session_id, token, seq, nonce, payload)` using the client bound key → `ids-session` validates: token present and not revoked, signature verifies against bound client key, replay window (seq+nonce) clean, session in `Active` state and not expired → session state advances (idle timer reset) → payload delivered to the owning ids-crate → response datagram to `ids-carrier::Endpoint`.

**Session states:** `Created` (binding established, not yet active), `Bound` (client key registered), `Active` (traffic permitted), `Idle` (past idle timeout, traffic blocked until re-auth/refresh), `Expired` (past absolute timeout, terminal until new session), `Suspended` (transport disruption, state preserved, resume possible), `Resuming` (recovery in progress, gap reconciliation), `Terminated` (explicit end, revoked). Transitions are event-driven: `session_open`, `session_active`, `idle_timeout`, `absolute_timeout`, `session_suspend`, `session_resume`, `session_close`, `token_renew`, `key_rotate`.

**Session stickiness / client affinity:** clients are mapped to node via consistent hash over `(session_id, client_pubkey)` — Ringhash/MagLev-style — so affinity survives node churn (only ~1/N of sessions remap when a node is added/removed). The hash key is the session token, never source IP (IP is prohibited and NAT/mobile changes break it). Affinity state (session→node) is itself session metadata carried as datagrams.

**Replay protection:** per-session sliding-window anti-replay (DTLS 1.3 / RFC 4303 ESP pattern): each session maintains a bitmap window of accepted `(seq, nonce)` pairs; out-of-window or duplicate entries are silently discarded; window advances only on accepted messages; nonce is fresh random per message to bind to a single use.

**Session recovery:** server persists session state (ID, bound key, token, last seq, idle/absolute deadlines) for a bounded `maxDisconnectionDuration` (e.g. 2 minutes default, configurable). On reconnect, client presents `session_id` + `last_seq_processed`; server restores state, replays missed messages after `last_seq`, resumes in `Active`. If the disconnect exceeds the bound or state was evicted, recovery fails and a new session is created.

**Crypto used:** ed25519 for client-bound key signatures and proof-of-possession; sha256/blake3 for token hashing, session ID derivation, audit-chain links; xchacha20poly1305 / chacha20poly1305 to encrypt session state at rest (bound key material, token secrets) when confidentiality required.

---

### 3. Detailed Research Dimensions

#### 3.1 Session creation and client-session binding

A session is the binding of a client identity to a server-side session context for the duration of an interaction. Per OWASP, once authenticated, the session ID/token is temporarily equivalent to the strongest authentication method used — disclosure, capture, prediction, brute force, or fixation of the session ID leads to session hijacking. `ids-session` therefore binds the session not just to an opaque token but to a client-held ed25519 key pair: the session token authorizes *which* session, the ed25519 proof-of-possession proves *who* presents it (holder-of-key / DPoP pattern). Binding is established at creation: server generates ephemeral ed25519 key pair for the client (or accepts a client-provided public key), signs a binding commitment, and ties `session_id` ↔ `client_pubkey` ↔ `token`. Every subsequent session-bound message carries an ed25519 signature over `(session_id, seq, nonce, payload)` verifiable against `client_pubkey`. A stolen token alone is unusable without the private key. Token Binding (RFC 8471) and DPoP (RFC 9449) both follow this proof-of-possession principle; DBSC (W3C) extends it to hardware-bound keys.

#### 3.2 Session expiration (idle + absolute timeouts)

OWASP and NIST SP 800-63B mandate both an idle timeout (inactivity window) and an absolute timeout (maximum lifetime regardless of activity). Common idle timeouts: 2–5 min high-value, 15–30 min low-risk; absolute: 4–8 hours for full-day office use. `ids-session` enforces both server-side against monotonic `IdsTime` (never client-supplied timestamps, which an attacker could manipulate). Idle: on each accepted session-bound message, reset `last_activity`; if `now - last_activity > idle_timeout`, transition `Active → Idle` and block traffic until a renewal/auth refresh. Absolute: if `now - created_at > absolute_timeout`, transition `Active → Expired` (terminal); client must create a new session. Optional renewal timeout: transparently rotate the session token mid-session (new token, old token valid for a short safety interval) to bound the reuse window of a stolen ID. Browser-close cannot be relied on for logout; server-side timeouts + explicit logout are authoritative.

#### 3.3 Session key management

Each session derives symmetric session keys for encrypting session state at rest (xchacha20poly1305 / chacha20poly1305) and an ed25519 key pair for proof-of-possession. Session keys are ephemeral: generated at creation, rotated on token renewal or key-rotation events, and zeroed on termination. Key derivation uses HKDF-style construction over the client bound key + session nonce (allowed primitives only; HKDF itself is a construction over sha256/blake3, not a new primitive). Bound private keys are never logged or transmitted; only public keys are exchanged via datagrams. Session key material at rest is encrypted; on session expiry/termination, keys are zeroized and the token is revoked. This mirrors EDHOC/OSCORE (RFC 9528/8613): lightweight authenticated key exchange establishing a security context, with sequence-number replay protection, suitable for constrained/carrier-agnostic environments.

#### 3.4 Session state machine

The session lifecycle is a deterministic finite-state machine so all replicas agree on whether a session is valid regardless of message delivery order. States and events (research-verified against Diameter RFC 6733 session state machines, OAM server-side session lifecycle, and ZTSAACM FSM papers):

| State | Meaning |
|-------|---------|
| Created | Binding established, client key registered, not yet active |
| Bound | Client authenticated to session, ready for traffic |
| Active | Traffic permitted; idle/absolute timers running |
| Idle | Past idle timeout; traffic blocked until renewal/re-auth |
| Expired | Past absolute timeout; terminal, must create new session |
| Suspended | Transport disruption; state preserved, resume within window |
| Resuming | Recovery in progress; gap reconciliation |
| Terminated | Explicit close or revoke; token revoked, keys zeroized |

Events: `session_open`, `session_bind`, `session_activate`, `idle_timeout`, `absolute_timeout`, `token_renew`, `key_rotate`, `session_suspend`, `session_resume`, `session_close`, `session_revoke`. Every transition is recorded in the append-only, hash-chained audit log (sha256/blake3 chain + ed25519-signed checkpoints). Invalid transitions (e.g. `Active → Terminated` without close, `Expired → Active`) are rejected. This determinism is what lets `ids-session` converge across replicas under carrier-agnostic, unordered delivery — the same CALM/convergent principle as CRDTs: all replicas applying the same transition sequence reach the same state.

#### 3.5 Session recovery after disruption

When transport is disrupted (carrier-agnostic: any `ids-carrier::Endpoint` may drop/reorder/delay datagrams), `ids-session` preserves session state server-side for `maxDisconnectionDuration`. On reconnect, client sends `SessionResume{session_id, token, last_seq_processed, client_sig}` → server validates signature against bound key, checks session not expired, restores `Active` state, reconciles the message gap (re-sends messages with seq > `last_seq_processed`), resumes timers. If the disconnect exceeds the bound or state was evicted, recovery fails → `Terminated` → client creates a new session. This mirrors Socket.IO connection state recovery (session ID + offset replay, maxDisconnectionDuration) and WebSocket reconnection state sync (sequence numbers, gap replay). Jittered exponential backoff is applied to reconnect attempts to avoid thundering herd after a node restart.

#### 3.6 Client affinity and session stickiness

Session stickiness routes all messages of a session to the same node that holds its state. `ids-session` uses consistent hashing (Ringhash/MagLev) over `(session_id, client_pubkey)` to map sessions to nodes deterministically. When nodes are added/removed, only ~1/N of sessions remap — far better than `hash % N` (which remaps ~(N-1)/N). Affinity is *soft* (best-effort): if the target node is unreachable, any node can proxy the request and forward session state via `ids-carrier::Endpoint` datagrams, because session state is itself replicable metadata. The hash key is the session token (not source IP): IP is prohibited by the carrier rule and breaks under NAT/mobile. Affinity mapping (session → node) is session metadata carried as datagrams, so it survives carrier re-framing.

#### 3.7 Session token management

Tokens are opaque random values with ≥128 bits entropy (NIST SP 800-63B / OWASP ASVS 3.2.2 recommend ≥64–112+ bits; OAuth RFC 6749 §10.10 prefers 128+). Generated by CSPRNG; the public token does not count toward the verification secret's entropy; a separate verification secret (HMAC/BLAKE3 keyed) is stored server-side. Tokens are rotated on renewal (new token, old valid for short safety interval) and revoked on expiry/termination/logout. Revocation: bounded denylist keyed by `jti`/token-id with TTL (token rotation with reuse detection per RFC 9700: presenting a rotated token revokes the whole lineage). Stateless bearer tokens are acceptable only when short-lived; long-lived sessions use sender-constraining (ed25519 proof-of-possession) so a stolen token cannot be replayed from another client.

#### 3.8 Session replay protection

Per-session sliding-window anti-replay (DTLS 1.3 / RFC 4303 ESP §3.4.3): each session tracks a bitmap window of accepted `(seq, nonce)` pairs. Sender increments `seq` per message and attaches a fresh random `nonce`; receiver accepts only if `seq` is within the window and `nonce` unseen; duplicates and out-of-window entries are silently discarded. Window advances on accepted messages, tolerating reordering within the window size. Nonces are single-use (bound to one message) to bind signatures to a single presentation (DPoP `jti` pattern). Replay cache is bounded (fixed-size + TTL eviction) to prevent memory DoS. Because `ids-session` is carrier-agnostic and datagrams may be duplicated by the carrier, replay protection is mandatory at the session layer, not the transport layer.

---

### Threat Model

| Threat | Asset at risk | Mitigation (this crate) |
|--------|---------------|-------------------------|
| Session token theft → hijack from another client | Session authenticity | ed25519 proof-of-possession bound to client key; stolen token alone is unusable |
| Session token guessing / brute force | Session confidentiality | ≥128-bit entropy CSPRNG tokens (NIST/ASVS); server-side verification secret |
| Session fixation (attacker forces known token) | Session integrity | Server generates tokens; client-supplied tokens never accepted as authoritative |
| Replay of session-bound messages | Session integrity | Per-session sliding-window (seq+nonce) anti-replay; bounded replay cache |
| Clock manipulation to extend/shorten session | Expiration correctness | Idle/absolute deadlines evaluated against monotonic `IdsTime`, never client timestamps |
| Session key extraction | Session confidentiality | Bound private keys never transmitted/logged; state encrypted at rest (xchacha20poly1305/chacha20poly1305); zeroized on termination |
| Token replay after rotation/expiry | Session integrity | Token rotation lineage + bounded denylist (jti); presenting rotated token revokes lineage |
| Session state loss on node restart | Availability | State persisted for `maxDisconnectionDuration`; reconnect + gap replay restores |
| Thundering herd on restart | Availability | Jittered exponential backoff on reconnect attempts |
| Carrier duplicating/delaying datagrams | Replay / ordering | Carrier-agnostic: replay protection at session layer; state machine tolerates unordered delivery |
| Signing-key compromise | Authenticity | Signed key-rotation handoff; old public keys retained for verification |

The carrier is never trusted: session bindings and tokens are signed over payload bytes, surviving re-framing by any carrier; the carrier-agnostic boundary (opaque `Endpoint`, `CarrierType` + `Vec<u8>`) means no network stack is in the trust base.

---

### Integration with Existing Crates

| Crate | Role | Relevance to ids-session |
|-------|------|--------------------------|
| `ids-000-aa` | Master crate: canonical types + sealed trait contracts + invariants + errors | `NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error` — consumed, never redefined |
| `ids-carrier` | Opaque `Endpoint` (`CarrierType` + `Vec<u8>`) datagram send/receive via `Carrier` trait | All session traffic is datagrams to `Endpoint`; no direct carrier dependency |
| `ids-crypto` | Allowed primitives: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | Signatures (PoP), token hashing, session-state encryption |
| `ids-store` | Primary record store (session user) | Opens/uses sessions per client; all session-bound writes go through `ids-session` binding |
| `ids-fragment` | Record fragments/shards | Fragment access bound to the client's active session |
| `ids-replication` | Replica synchronization | Replicates session state (token, bound key, state machine) across replicas for convergence |
| `ids-immutable` (Phase 040) | Immutable record protection | Session-bound mutations recorded to immutable audit chain; hold/retention checked per session |

---

### Proposed File Structure (~90-1000 files in subfolders)

`C:/idfs/ids-session/` (a separate crate, not a folder inside an existing crate):

```
ids-session/
  Cargo.toml
  src/
    lib.rs                      # crate root, public API surface
    errors.rs                   # SessionError, BindingError, ReplayError, ExpiryError
    types.rs                    # SessionId, SessionToken, SessionState, SessionConfig, BoundKey (re-exports ids-000-aa types)
    creation.rs                 # session creation, token issuance, binding establishment
    expiration.rs               # idle/absolute timeout enforcement against monotonic IdsTime
    key_management.rs           # ed25519 bound keys, session key derivation, rotation, zeroize
    state_machine.rs            # session FSM: states, transitions, validity guards
    recovery.rs                 # session resume after disruption, gap reconciliation, maxDisconnectionDuration
    affinity.rs                 # client affinity / stickiness via consistent hash (Ringhash/MagLev)
    token.rs                    # opaque token management, rotation, revocation (denylist/jti lineage)
    replay.rs                   # per-session sliding-window anti-replay (seq+nonce bitmap)
    carrier_adapter.rs          # opaque Vec<u8> datagram framing to ids-carrier::Endpoint
    contracts/                  # sealed trait contracts mirroring ids-000-aa style (~10 files)
      session_contract.rs, binding_contract.rs, key_contract.rs, state_contract.rs,
      token_contract.rs, replay_contract.rs, affinity_contract.rs, recovery_contract.rs,
      expiration_contract.rs, mod.rs
    invariants/                 # compile-time/runtime invariants (~8 files)
      token_entropy.rs, bound_key_never_leaked.rs, monotonic_deadlines.rs,
      replay_window_sound.rs, hold_overrides_expiry.rs, state_transition_valid.rs,
      carrier_agnostic.rs, crypto_allowlist.rs, mod.rs
  tests/                        # property/verification tests (~20 files)
  benches/                      # (~6 files)
  docs/                         # (~12 files)
```

Total: ~90–1000 files across `src/`, `src/contracts/`, `src/invariants/`, `tests/`, `benches/`, `docs/`, organized by subsystem so each concern is a separate file — consistent with the "separate crates instead of more folders" directive applied intra-crate as separate modules/files.

---

### Key Algorithms & Data Structures

- **SessionId / SessionToken**: `SessionId` = BLAKE3-256 of `(client_pubkey, creation_nonce)` truncated; `SessionToken` = opaque random ≥128 bits entropy (CSPRNG), server-stored verification secret keyed by BLAKE3/SHA-256.
- **BoundKey**: ephemeral ed25519 key pair per session; public key exchanged via datagram, private key never leaves the client; every session-bound message signed: `sig = ed25519_sign(client_priv, session_id ∥ seq ∥ nonce ∥ payload)`.
- **Session FSM**: states {Created, Bound, Active, Idle, Expired, Suspended, Resuming, Terminated}; transitions event-driven; invalid transitions rejected; all transitions recorded in append-only audit log.
- **Idle/absolute deadlines**: evaluated against monotonic `IdsTime`; idle resets on each accepted message; absolute is terminal.
- **Replay window**: per-session bitmap over `(seq, nonce)`; window size W (default 64); accept iff `seq` in window and `nonce` unseen; duplicates discarded; window advances on accept.
- **Token rotation + denylist**: new token on renewal; old token valid for safety interval; rotated/revoked tokens enter bounded denylist (jti + TTL); presenting a rotated token revokes the whole lineage (RFC 9700 rotation-with-reuse-detection).
- **Affinity**: `node = consistent_hash(session_id ∥ client_pubkey)` over Ringhash/MagLev ring; ~1/N remap on churn; soft affinity with proxy fallback.
- **Recovery**: persist session state for `maxDisconnectionDuration`; reconnect presents `(session_id, last_seq, sig)`; server restores, replays messages with `seq > last_seq`, resumes timers; failure → new session.
- **Audit chain**: each session decision (create, bind, expire, rotate, revoke, resume) = SHA-256/BLAKE3-chained entry + ed25519-signed checkpoint; offline verifiable.

---

### Dependencies

- `ids-000-aa` — canonical types (`NodeId`, `RegionId`, `IdsTime`, `CarrierType`, `RecordId`, `Checksum`, `Error`) and sealed trait contracts.
- `ids-carrier` — `Endpoint` (`CarrierType` + `Vec<u8>`) and `Carrier` trait for opaque datagram transport only.
- `ids-crypto` — allowed primitives only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `serde` — serialization of session envelopes, tokens, audit entries.
No `std::net`, `tokio::net`, libp2p, or any networking stack. No crypto beyond the allowed list.

---

### Measurable Acceptance Criteria

1. Session creation issues an opaque token with ≥128 bits entropy and binds it to a client ed25519 key; 100% of creations produce a unique binding.
2. Session-bound messages without a valid ed25519 proof-of-possession signature against the bound client key are rejected (0 accepted without verification).
3. Idle timeout: sessions past `idle_timeout` (against monotonic `IdsTime`) transition to `Idle` and block 100% of traffic until renewal.
4. Absolute timeout: sessions past `absolute_timeout` transition to `Expired` (terminal); 100% of traffic rejected; new session required.
5. Replay protection: replayed session-bound messages (duplicate seq/nonce) are rejected; sliding window tolerates reordering within W; bounded replay cache never exceeds configured size.
6. Token rotation: presenting a rotated token revokes the entire rotation lineage; rotation+revocation is audited.
7. Recovery: within `maxDisconnectionDuration`, reconnect + gap replay restores `Active` state; beyond the bound, recovery fails and a new session is created.
8. Affinity: consistent-hash mapping routes a session to the same node; adding/removing a node remaps only ~1/N of sessions.
9. All session traffic carried as opaque `Vec<u8>` datagrams to `ids-carrier::Endpoint`; zero references to `std::net`/`tokio::net`/IP/TCP/UDP/QUIC/HTTP/DNS in the crate.
10. Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 appear in the crate's crypto surface.
11. Every session decision (create, bind, expire, rotate, revoke, resume) recorded in append-only, hash-chained, signed audit log with actor + `IdsTime`.

---

### Remaining Uncertainties

- Clock source: monotonic `IdsTime` for deadlines vs loosely-synced wall time; monotonic avoids manipulation but complicates cross-node absolute deadlines without a network clock.
- Bound key provenance: server-generated ephemeral ed25519 per session vs client-provided public key; affects key management complexity and PoP flow.
- Cross-replica session state convergence under long partitions: session state (token, bound key, FSM state) must converge; replication strategy (state transfer vs full sync) to be validated at integration with `ids-replication`.
- `maxDisconnectionDuration` default and tuning: too short → premature session loss; too long → state memory bloat; workload-dependent.
- Interaction with `ids-immutable` (Phase 040): session-bound mutations must satisfy retention/hold before the immutable guard allows them; exact guard-session handshake is defined here but validated at integration.
- Affinity fallback behavior when the consistent-hash target node is down: proxy-and-forward vs session migration; policy decision pending.

---

### Engineering Decisions Summary

- Separate crate `ids-session` (not a folder inside an existing crate), organized into subfolders/files by subsystem — one file per concern.
- Client-session binding via ephemeral ed25519 key pair + proof-of-possession signatures on every message (DPoP/Token Binding/DBSC principle): stolen token alone is unusable.
- Session expiration via server-side idle + absolute timeouts against monotonic `IdsTime`; optional renewal timeout for token rotation.
- Session lifecycle as a deterministic 8-state FSM; all transitions audited and chained; invalid transitions rejected — enables convergence under unordered carrier delivery.
- Session recovery via persisted state + `maxDisconnectionDuration` + reconnect gap replay (Socket.IO / WebSocket pattern).
- Client affinity via consistent hashing over `(session_id, client_pubkey)`; soft affinity with proxy fallback; session token (not IP) as hash key.
- Session tokens: opaque ≥128-bit entropy CSPRNG values; rotation with bounded denylist and reuse-detection lineage revocation (RFC 9700 pattern).
- Replay protection: per-session sliding-window `(seq, nonce)` bitmap (DTLS 1.3 / RFC 4303 ESP pattern); bounded replay cache.
- Only allowed crypto; carrier-agnostic boundary via `ids-carrier::Endpoint`; no network stack.

---

### Sources

[^1]: OWASP, "Session Management Cheat Sheet" — idle/absolute/renewal timeouts, ≥64 bits entropy session IDs, CSPRNG generation, server-side enforcement, browser-close unreliability. https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
[^2]: NIST SP 800-63B, "Session Management" — session secret ≥64 bits, opaque string, directly presented or cryptographically proven possession, approved random bit generator. https://pages.nist.gov/800-63-4/sp800-63b/session
[^3]: OWASP ASVS v4.0.3, V3 Session Management — 3.2.2 ≥64 bits entropy; timeout aligned with NIST 800-63. https://github.com/OWASP/ASVS/blob/v4.0.3/4.0/en/0x12-V3-Session-management.md
[^4]: MDN Web Docs, "Session management" — idle/absolute/renewal timeouts, 64-bit entropy minimum, cookie storage (HttpOnly/SameSite). https://developer.mozilla.org/en-US/docs/Web/Security/Authentication/Session_management
[^5]: OWASP WSTG, "Session Timeout" testing — server-side idle/absolute timeouts; replay of old session ID must be rejected. https://github.com/OWASP/wstg/blob/master/document/4-Web_Application_Security_Testing/06-Session_Management/07-Session_Timeout.md
[^6]: RFC 4303, "IP Encapsulating Security Payload (ESP)" — per-SA 32-bit sequence number, sliding-window replay detection (§3.4.3). http://rfc.eu/rfc/rfc4303.htm
[^7]: RFC 8471, "Token Binding Protocol Version 1.0" — client proves possession of private key on every connection; binds tokens to TLS channel. https://datatracker.ietf.org/doc/rfc8471
[^8]: RFC 9449, "OAuth 2.0 Demonstrating Proof of Possession (DPoP)" — sender-constraining tokens via client key proof; `jti` replay detection; token substitution prevention. https://datatracker.ietf.org/doc/html/rfc9449
[^9]: RFC 9700, "Best Current Practice for OAuth 2.0 Security" — refresh token rotation with reuse detection OR sender-constraining (DPoP/mTLS). https://datatracker.ietf.org/doc/rfc9700
[^10]: W3C / webappsec-dbsc, "Device Bound Session Credentials" — hardware-bound private key proving session possession; short-lived cookies; proof on refresh. https://w3c.github.io/webappsec-dbsc
[^11]: Traefik / Envoy / Solo.io, "Consistent hashing & sticky sessions" — Ringhash/MagLev; affinity survives node churn (~1/N remap); hash key = cookie/header not IP. https://docs.traefik.io / https://docs.solo.io/gateway/latest/traffic-management/session-affinity/hash
[^12]: Socket.IO, "Connection state recovery" — session ID + last offset replay on reconnect; maxDisconnectionDuration; missed-packet restoration. https://socket.io/docs/v4/connection-state-recovery
[^13]: WebSocket.org, "WebSocket Reconnection: State Sync and Recovery Guide" — message sequence numbers, last-received tracking, gap replay, jittered backoff. https://websocket.org/guides/reconnection
[^14]: RFC 9528, "EDHOC" — lightweight authenticated Diffie-Hellman key exchange; OSCORE security context; sequence-number replay protection. https://rfc-editor.org/rfc/rfc9528.pdf
[^15]: RFC 8613, "OSCORE" — object security for constrained environments; replay protection via sequence numbers. https://datatracker.ietf.org/doc/rfc8613
[^16]: draft-ietf-ace-edhoc-oscore-profile — ACE profile using EDHOC for key establishment + OSCORE for secure communication. https://datatracker.ietf.org/doc/draft-ietf-ace-edhoc-oscore-profile/11
[^17]: DTLS 1.3 (draft-ietf-tls-dtls13) — record sequence numbers, sliding-window replay detection per epoch. https://datatracker.ietf.org/doc/draft-ietf-tls-dtls13/21
[^18]: draft-mw-oauth-tls-session-bound-tokens — binding OAuth tokens to a specific mTLS connection via exporter value. https://ietf.org/archive/id/draft-mw-oauth-tls-session-bound-tokens-03.html
[^19]: Signal Double Ratchet analysis — session key evolution, root key + chain key ratcheting, forward secrecy. https://crypto.iacr.org/2022/papers/530630_1_En_27_Chapter_OnlinePDF.pdf
[^20]: Noise Protocol Framework — handshake patterns (XX/IK/NK) for session key establishment. https://noiseprotocol.org
[^21]: RFC 6733, "Diameter" — user session state machines (client/server, state maintained vs not); session termination events. https://rfcinfo.com/rfc-6733/8-diameter-user-sessions
[^22]: Oracle OAM, "Understanding Server-Side Session Management" — session lifecycle states (Active/Idle/Expired); lifetime and idle timeout settings. https://docs.oracle.com/cd/E52734_01/oam/AIAAG/GUID-989FF475-CBAE-46B0-A809-46832D744C9F.htm
[^23]: ZTSAACM (IEEE) — event-driven FSM binding session lifecycle to access control; deterministic transitions. https://ieeexplore.ieee.org/document/11417787
[^24]: JWT blacklist/denylist patterns — Bloom filter, Redis TTL, streaming propagation, jti revocation. https://jsonic.io/guides/jwt-blacklist-strategies
[^25]: Alibaba Cloud ASM, "Implementing session persistence" — consistent hashing for sticky sessions; cookie/header hash keys. https://alibabacloud.com/help/en/asm/user-guide/implementing-session-persistence-in-asm-gateway