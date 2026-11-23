## Phase 002 — Create the ids-carrier crate: Native carrier interface, no existing networking stack: Research Findings & Implementation Blueprint

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### 1. Executive Summary

Phase 002 creates `ids-carrier`, the single carrier-agnostic transport abstraction crate for the IDS workspace. It is the only crate that owns the traffic surface: `ids-carrier::Endpoint` (`CarrierType` + opaque `Vec<u8>`), `ids-carrier::Datagram` (opaque `Vec<u8>` payload + sequence), `send_datagram(endpoint, payload)`, and `receive_datagram()`. Every one of the 99 dependent `ids-*` crates addresses peers exclusively through `ids-carrier::Endpoint`; no other crate may import `std::net`, `tokio::net`, libp2p, or any IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh stack. `ids-carrier` is datagram-native and unreliable by default: it moves opaque byte sequences from one `Endpoint` to another, never inspects payload contents, performs no cryptography (crypto boundary belongs to `ids-crypto`, limited to ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-carrier` envelope checksums may use only BLAKE3 or SHA-256), and assumes nothing about delivery (loss, reordering, and duplication are expected and handled by layers above, chiefly `ids-protocol`).

The crate hosts the `CarrierPlugin` trait + plugin registry: every carrier (in-process, local, loopback, test, user-supplied) is a plugin behind the trait; the core imports zero sockets. Each plugin advertises a `CarrierCapabilities` set (max datagram size/MTU, ordered, reliable, duplex, fragmentation) and peers negotiate by intersection before the first send; MTU overshoot triggers length-prefixed fragmentation/reassembly inside `ids-carrier`. Carrier health is scored with a phi accrual failure detector over plugin heartbeats; unhealthy carriers are quarantined and reconnect with exponential backoff plus jitter. Addressing is carrier-agnostic: the `Vec<u8>` address bytes are opaque to `ids-carrier` and interpreted only by the plugin; there is no IP, port, DNS name, or locator anywhere.

Research basis: 30 parallel searches returned datagram transport traits with associated address types (kcp-tokio `Transport`, reconcile-gossip `Transport`, embedded-nal, `datagram::DatagramSender`), a carrier-agnostic "carriage layer = send/receive/discover over opaque bytes" model, SDP offer/answer capability negotiation (RFC 3264 model), phi accrual failure detection (Hazelcast, Akka, Cassandra), address-without-IP research (locator/identifier separation LISP/ILNP/DIME, addressless Internet draft, Netsukuku ANDNA, OpenNet ServiceID, content-addressed/IPFS key-based routing), reliable-datagram patterns (ARQ stop-and-wait/go-back-N/selective repeat, NAK-based LAMS-DLC, NORM RFC 5740), congestion-controlled unreliable datagrams (DCCP RFC 4340), backpressure flow control (NSDI'22 BFC, credit-based flow control), WebRTC data channel reliability modes and RFC 9221 QUIC DATAGRAMs (research only — QUIC is prohibited), ZeroMQ/nanomsg/NNG transport-agnostic opaque-blob sockets, Akka.Remote location-transparent remoting, PTRS/OpenCarrier Rust pluggable-transport trait patterns (Tor pluggable transports reviewed as an anti-example: obfuscation is out of scope), path MTU discovery framing rules, message framing (length-prefix vs delimiter), gRPC exponential backoff with jitter, consistent hashing/Chord coordinator-free routing, and CIRP capability-oriented routing (research only).

A skeleton `ids-carrier` crate does not yet exist; this plan specifies crate creation from scratch: `Cargo.toml` (depends on `ids-000-aa` only, plus `thiserror`/`bytes`/`serde`/`blake3`), `src/` modules below, no source modifications to any existing crate.

---

### 2. Architecture Overview

`ids-carrier` is layer 1 of the IDS dependency stack: it sits directly on top of `ids-000-aa` (which defines `Endpoint`, `CarrierType`, `trait Carrier`, and the error/invariant contracts) and below all other `ids-*` crates. It is **not** a network protocol stack — it is a plugin host that moves opaque datagrams between `Endpoint`s.

```
┌─────────────────────────────────────────────────────────────────────┐
│                   DOWNSTREAM ids-* CRATES (2–100)                   │
│  ids-protocol · ids-discovery · ids-routing · ids-membership · …    │
│  (each addresses peers only via ids-carrier::Endpoint)              │
├─────────────────────────────────────────────────────────────────────┤
│                Phase 002: ids-carrier (THIS CRATE)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────────┐   │
│  │   Endpoint   │  │   Datagram   │  │   CarrierType enum      │   │
│  │ CarrierType  │  │ payload:     │  │ Loopback Serial Optical │   │
│  │ + Vec<u8> ≤  │  │ Vec<u8> +    │  │ Radio Custom(u16)       │   │
│  │   1024 bytes │  │ seq: u64     │  │ (closed; Custom = user) │   │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬────────────┘   │
│         └─────────────────┴──────────────────────┘                │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ send_datagram(endpoint, payload) -> Result<DatagramId>      │   │
│  │ receive_datagram() -> Result<Datagram, CarrierError>        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────────┐   │
│  │ Capability   │  │ Carrier      │  │ Carrier Health (phi)    │   │
│  │ negotiation  │  │ Health       │  │ Monitor + quarantine    │   │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬────────────┘   │
│         └─────────────────┴──────────────────────┘                │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  CarrierPlugin trait — EVERY carrier is a plugin            │   │
│  │  send / recv / capabilities / health — no sockets imported  │   │
│  └──────┬──────────┬──────────┬──────────────┬───────────────┘   │
│   in_process   local   loopback   test     user (Custom u16)     │
└──────────┴──────────┴──────────┴──────────────┴───────────────────┘
│                      CARRIER BOUNDARY                                │
│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP   │
│  / DNS / Wi-Fi / Bluetooth / cellular / VPN / mesh anywhere here    │
└─────────────────────────────────────────────────────────────────────┘
```

**Data flow (send):** caller builds `Endpoint { carrier, address: Vec<u8> }` + payload → `ids_carrier::send_datagram` → plugin selected by `CarrierType` → capability negotiation (intersect local/peer `CarrierCapabilities`; on first contact) → framing (length-prefixed envelope + BLAKE3/SHA-256 payload digest) → MTU check → fragment if payload > plugin MTU → plugin `send` → wire bytes. **Data flow (receive):** plugin `recv` → envelope verification → reassembly if fragmented → health update (phi sample) → deliver `Datagram` to caller. Loss/reordering/duplication are visible to callers via sequence numbers; recovery is `ids-protocol`'s responsibility.

**Design rules (non-negotiable):**
- One carrier crate only: `ids-carrier` is the single carrier crate in the workspace; zero other crates may touch sockets or carrier internals.
- `Endpoint` is the sole address type: `CarrierType` + opaque `Vec<u8>` capped at 1024 bytes. No IP, port, `SocketAddr`, DNS name.
- Datagram payloads are opaque `Vec<u8>`; `ids-carrier` never parses, inspects, or semantically interprets payload contents.
- No cryptography inside `ids-carrier`; framing checksum only, using BLAKE3 or SHA-256 (both in the allowed set).
- Unreliable datagrams by default (no ACK/retransmit/flow-control state in the core); optional reliability modules exist as research references but live behind `ids-protocol`.
- Plugins are loaded behind `CarrierPlugin`; dynamic loading is gated behind a feature flag and require explicit opt-in.
- Capability negotiation is mandatory before first send; MTU mismatch → fragment (ids-carrier) or reject, never silent truncation.
- Carrier health: phi accrual scoring; unhealthy → quarantined; reconnect via exponential backoff + jitter; never silent eternal retry.

---

### 3. Detailed Research Dimensions

#### 3.1 Opaque datagram transport interfaces

Research returned several prior-art datagram traits that shape the `CarrierPlugin` interface: kcp-tokio's `Transport` trait (`send_to(buf, target: &Self::Addr) -> usize`, `recv_from(buf) -> (usize, Self::Addr)`, `local_addr()`) with an associated `Addr` type; reconcile-gossip's `Transport` trait (`recv_from` into `buf` returning `(usize, SocketAddr)`, `send_to(buf, dst)`, `local_addr()`); embedded-nal's `UdpStack`/`ConnectedUdp` (send/receive datagrams in `no_std` embedded contexts); and the `datagram` crate's `DatagramSender::send(&[u8])`. Common pattern: a transport is defined by (a) an associated address type, (b) send/datagram-out and receive/datagram-in primitives, (c) a local address query. **Decision for `ids-carrier`:** `CarrierPlugin` exposes exactly this shape — `send_datagram(Endpoint, &[u8])`, `receive_datagram() -> Option<Datagram>`, `capabilities()`, `health()`, `name()` — with the address type fixed to `Endpoint` (carrier-agnostic) rather than `SocketAddr`. The associated-address pattern means a plugin's address representation stays inside the plugin; `ids-carrier` core sees only `Endpoint`.

#### 3.2 Carrier-agnostic endpoint and addressing

`Endpoint` = `CarrierType` (Loopback, Serial, Optical, Radio, Custom(u16), closed enum defined in `ids-000-aa`) + opaque `Vec<u8>` address bytes (≤1024 bytes per plan-001). Research on addressing without IP supports this: the "Addressless Internet" draft proposes eliminating end-to-end IP addresses entirely; LISP/ILNP/DIME locator/identifier separation splits "who" from "where"; Netsukuku ANDNA replaces DNS with hashed hostnames; OpenNet binds domains to cryptographic ServiceIDs rather than machines; IPFS-style content-addressed routing locates objects by key, not by IP. None of these require IP, TCP, or DNS inside the transport crate. **Decision:** address bytes are fully opaque to `ids-carrier` and interpreted only by the owning plugin; address resolution/naming (if any) belongs to `ids-discovery`/`ids-routing` layers above, never to `ids-carrier`. There is no DNS client, no address resolver, and no network-layer header in `ids-carrier`.

#### 3.3 Capability negotiation

Research: SDP offer/answer (RFC 3264 model) lets two peers advertise potential configurations and converge on a mutually supported actual configuration; SDP capability negotiation adds "potential configs" with attributes (crypto suites, transport types). WebRTC data channels expose reliability modes (reliable/ordered, unreliable/unordered, partial-reliable via maxRetransmits/maxPacketLifeTime) negotiated at channel creation. RFC 9221 (QUIC Unreliable Datagram Extension) advertises `max_datagram_frame_size` as a transport parameter during handshake — reviewed as research only; QUIC itself is prohibited. **Decision:** each plugin publishes `CarrierCapabilities { max_datagram_size, ordered, reliable, duplex, fragmentation_supported }`; before the first datagram to a peer, peers exchange capability advertisements and compute the intersection; `max_datagram_size` becomes the send MTU for that peer; mismatches (e.g., peer lacks fragmentation) cause the sender to fragment locally or reject oversized datagrams with `CarrierError::MtuExceeded`, never silent truncation. Negotiation results are cached per `Endpoint` and invalidated on health failure.

#### 3.4 Reliable datagram patterns (deferred to ids-protocol)

Research: ARQ reliable transmission (stop-and-wait, go-back-N, selective repeat with ACKs/NAKs, timeouts, sequence numbers); NAK-based reliable datagram service (LAMS-DLC, IEEE — checkpoint NAKs, near-optimal buffer use); NORM RFC 5740 (NACK-oriented reliable multicast). All assume an unreliable datagram substrate plus reliability layered on top. **Decision:** `ids-carrier` is deliberately unreliable (best-effort datagrams; loss/reorder/dup possible). Sequence numbers on `Datagram` let upper layers detect gaps. Reliability (ACK/NAK/retransmit/window) is owned by `ids-protocol`, not `ids-carrier`. An optional `reliability/` module (ARQ sketch, NAK sketch) is planned behind a feature flag for reference implementations only; it is not part of the core contract.

#### 3.5 Congestion control and backpressure for datagrams

Research: DCCP RFC 4340 (congestion-controlled unreliable datagrums with reliable handshake/teardown, CCID negotiation, ECN); Backpressure Flow Control (NSDI'22 — per-hop per-flow pause/resume with bounded state); credit-based flow control (receiver advertises free buffers; sender never exceeds credit); token bucket/leaky bucket rate shaping. **Decision:** `ids-carrier` exposes per-peer credit-based backpressure: `CarrierPlugin::credit_available() -> usize` and a credit-return signal; `send_datagram` returns `CarrierError::Backpressure` when peer credit is exhausted, so callers (and `ids-protocol`) throttle voluntarily. No end-to-end congestion control algorithm is mandated inside the crate; plugin authors may implement DCCP-style CCIDs inside their plugin.

#### 3.6 Framing, MTU, fragmentation, reassembly

Research: path MTU discovery (RFC 1191/3032 rules — fragments too large for output link; DF-bit semantics; reassembly only at destination); message framing alternatives — length-prefix (read N bytes then payload) vs delimiter (scan for escape byte; length-prefix preferred for opaque binary); IP fragmentation research (reassembly at destination, independent fragment travel, further fragmentation possible). **Decision:** `ids-carrier` uses a length-prefixed wire envelope (4-byte length + payload + BLAKE3/SHA-256 digest) per datagram fragment; per-plugin MTU comes from `CarrierCapabilities.max_datagram_size`; payloads exceeding MTU are fragmented by `ids-carrier` into numbered fragments and reassembled at the receiver before delivery; reassembly has a creation-time-keyed deadline (not RTT-based) so partial fragments never block indefinitely. No IP headers are ever constructed or parsed.

#### 3.7 Carrier health and failure detection

Research: phi accrual failure detector (Hazelcast, Akka, Cassandra — phi = -log10(1 − F(delay)) over heartbeat inter-arrival distribution; adaptive suspicion, threshold ~8–10; gracefully absorbs GC pauses and jitter); SWIM (alive/suspect/dead states, indirect probing). gRPC connection backoff (exponential backoff with jitter, initial 1s, multiplier 1.6, max 120s, jitter ±20%) prevents thundering herds on reconnect. **Decision:** each plugin emits heartbeat samples; `ids-carrier` runs a phi accrual scorer per peer; phi above threshold → `CarrierHealth::Unhealthy` → carrier quarantined (no sends, queued datagrams held or dropped per policy); recovery via exponential backoff with jitter (gRPC parameters). Health state is observable (`carrier_health(endpoint) -> CarrierHealth`) so `ids-failure-detector` and `ids-routing` can react. No binary dead/alive timeout; suspicion is continuous.

#### 3.8 Pluggable carrier architecture

Research: PTRS (Pluggable Transports in Rust — `PluggableTransport` trait with client/server builders, `name()`, `method_name()`, wraps `AsyncRead+AsyncWrite` streams); Tor pluggable transports (reviewed as anti-example — obfuscation/circumvention is out of scope for IDS; `ids-carrier` does not hide traffic patterns); ZeroMQ/nanomsg/NNG transport-agnostic socket API (opaque message blobs, bind/connect via `transport://address`, multiple transports — TCP, IPC, WebSocket — behind one socket type); Akka.Remote (location-transparent actor remoting, `Address`/`ActorPath` extended with remote addressing); WebRTC data channels and WebTransport datagrams (reliable/unreliable datagram modes over carrier); OpenCarrier Plugin SDK (write plugins in safe Rust). **Decision:** `ids-carrier` core is plugin-agnostic: `CarrierPlugin: Send + Sync + 'static` trait object; plugins register under a `CarrierType::Custom(u16)` id; a gated `plugin_loader` module supports dynamic loading (opt-in feature, audited `libloading`-style); static built-in plugins (in_process, local, loopback, test, blackhole) ship with the crate. No `tokio::net`, `std::net`, or socket types appear in the core; plugins may be sync or async internally behind the trait boundary.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
|| Endpoint spoofing (peer claims another's address) | Plugin authenticates endpoint ownership at bind time; `ids-crypto` (ed25519) signs endpoint binding above the carrier layer; carrier never trusts address bytes alone ||
|| Datagram tampering in transit | Envelope payload digest (BLAKE3 or SHA-256) verified before delivery; tampered datagrams dropped, never delivered ||
|| Replay of delivered datagrams | Datagram sequence numbers per endpoint; replay window tracked by `ids-protocol` layer; carrier delivers, upper layer deduplicates ||
|| MTU/fragmentation bomb (oversized payload exhausts buffers) | `max_datagram_size` from capability intersection; `CarrierError::MtuExceeded` on overshoot; fragment count cap per datagram; reassembly deadline (creation-time-keyed) bounds fragment buffer lifetime ||
|| Plugin compromise or misbehavior | Plugins run behind `CarrierPlugin` trait; quarantine unhealthy carriers (phi); revoke `Custom(u16)` plugin registration; no plugin can access another plugin's state ||
|| Health false positives mark healthy carrier unhealthy | Phi accrual adapts to observed jitter; quarantine requires sustained phi above threshold; recovery path always exists via backoff reconnect ||
|| Amplification via plugin (small in → massive out) | Per-peer credit-based backpressure; send credit cap; `send_datagram` returns `CarrierError::Backpressure` when exhausted ||
|| Address collision (two endpoints with identical bytes) | Address uniqueness is plugin-scoped (`CarrierType` + bytes); duplicate registration rejected by plugin registry at bind time ||
|| Loss/reordering/duplication (inherent to datagrams) | Explicit in contract; `Datagram.seq` lets callers detect; reliability is `ids-protocol`'s responsibility ||
|| Denial of service via datagram flood | Per-endpoint ingress queue caps; drop-tail with audit log; `receive_datagram` returns `CarrierError::QueueFull`; caller throttles ||
|| No address resolution (no DNS) by design | Addresses provisioned out of band or by `ids-discovery` above; `ids-carrier` never resolves names ||
|| Dynamic plugin loading abuse | `plugin_loader` behind feature flag; plugin signatures verified where configured; static plugins are the default and only audited path ||

---

### 5. Integration with Existing Crates

|| Crate | Role in Phase 002 ||
||-------|-------|-------------------|
|| `ids-000-aa` | Defines `Endpoint`, `CarrierType`, `trait Carrier` contract, unified error contract, invariants (incl. `out_eq_in` byte accounting at the carrier boundary); `ids-carrier` depends on it and re-exports `Endpoint`/`CarrierType` as `ids-carrier::` traffic surface ||
|| `ids-carrier` (this crate) | `Endpoint` re-export, `Datagram`, `send_datagram`/`receive_datagram`, `CarrierPlugin` trait + registry, capability negotiation, carrier health, carrier-agnostic addressing, length-prefixed framing + BLAKE3/SHA-256 checksums ||
|| `ids-crypto` | NOT a dependency of `ids-carrier`; cryptographic boundary — if authenticated/encrypted datagrams are ever layered on top, only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 may be used ||
|| `ids-protocol` | Consumer: framing, sequencing, acks, retransmission sit above `ids-carrier` using `Datagram.seq` and opaque payloads ||
|| `ids-discovery` / `ids-routing` | Consumers: address peers via `Endpoint`; no address resolution inside `ids-carrier` ||
|| All crates 3–100 | Depend on `ids-000-aa` and use `ids-carrier::Endpoint` as the only traffic surface; none import sockets ||
|| User carrier plugins | Implement `CarrierPlugin`; register under `CarrierType::Custom(u16)`; audited static plugins ship in-tree (in_process, local, loopback, test, blackhole) ||

**Integration points:** `ids-000-aa::contracts::carrier::Carrier` is implemented by `ids_carrier::CarrierEngine`; `send_datagram`/`receive_datagram` on the engine satisfy the contract. `ids-carrier` never calls into any other `ids-*` crate (one-way dependency: `ids-carrier` → `ids-000-aa` only).

---

### 6. Proposed File Structure (~94 files, all in subfolders)

**Crate:** `ids-carrier` (new crate; depends on `ids-000-aa`; `thiserror`, `bytes`, `serde`, `serde_bytes`, `blake3`; dev-only `proptest`, `criterion`; **zero** networking crates, no `tokio`, no `libp2p`, no IP/TCP/UDP/QUIC/HTTP/DNS crates)

```
ids-carrier/
├── Cargo.toml                            # ids-000-aa only + std deps; no networking crates
├── lib.rs                                # crate root, feature gates, public re-exports
├── constants.rs                          # defaults: max address 1024 B, fragment caps, health intervals
├── error.rs                              # CarrierError (thiserror): MtuExceeded, Backpressure, Plugin, Health, Io
├── features.rs                           # feature flags: std/no-std, dynamic-loading, testing, fuzzing
├── prelude.rs                            # root re-exports
│
├── endpoint/                             # Endpoint, CarrierType, Datagram
│   ├── mod.rs
│   ├── endpoint.rs                       # Endpoint { carrier: CarrierType, address: Vec<u8> }
│   ├── carrier_type.rs                   # CarrierType: Loopback, Serial, Optical, Radio, Custom(u16)
│   ├── datagram.rs                       # Datagram { endpoint, payload: Vec<u8>, seq: u64 }
│   ├── address.rs                        # Opaque address bytes (≤1024 B), plugin-interpreted only
│   └── datagram_id.rs                    # DatagramId (origin endpoint + seq)
│
├── carrier/                              # carrier plugin engine
│   ├── mod.rs
│   ├── engine.rs                         # CarrierEngine: implements ids_000_aa::trait Carrier
│   ├── trait.rs                          # CarrierPlugin: send/recv/capabilities/health/name
│   ├── plugin.rs                         # Plugin handle + registration gate
│   ├── registry.rs                       # CarrierType → plugin registry
│   ├── capabilities.rs                   # CarrierCapabilities struct
│   ├── capability_set.rs                 # intersection/merge of capability sets
│   ├── negotiation.rs                    # capability negotiation between peers
│   ├── health.rs                         # CarrierHealth status
│   ├── monitor.rs                        # phi accrual health monitor per peer
│   ├── backoff.rs                        # exponential backoff with jitter (reconnect)
│   └── quarantine.rs                     # unhealthy carrier quarantine
│
├── transport/                            # datagram transport semantics
│   ├── mod.rs
│   ├── datagram_transport.rs             # send/recv orchestration (engine front door)
│   ├── framing.rs                        # length-prefixed wire envelope
│   ├── envelope.rs                       # versioned envelope + payload digest
│   ├── mtu.rs                            # per-peer MTU from capabilities
│   ├── fragmentation.rs                  # fragment / reassemble datagrams to MTU
│   ├── checksum.rs                       # BLAKE3 / SHA-256 payload digest (allowed set only)
│   ├── reliability/                      # optional reliability (reference, behind ids-protocol)
│   │   ├── mod.rs
│   │   ├── arq.rs                        # ARQ stop-and-wait / selective-repeat sketch
│   │   └── nak.rs                        # NAK-based reliability sketch (LAMS-DLC style)
│   └── flow_control/                     # per-peer backpressure
│       ├── mod.rs
│       ├── credit.rs                     # credit-based send credits
│       ├── limiter.rs                    # send-rate limiter
│       └── token_bucket.rs               # token bucket rate shaping
│
├── addressing/                           # carrier-agnostic addressing
│   ├── mod.rs
│   ├── addressing.rs                     # addressing model: opaque bytes, no IP/DNS
│   ├── resolution.rs                     # address provisioning hook (out-of-band / ids-discovery)
│   ├── advertisement.rs                  # capability advertisement exchange
│   └── namespace.rs                      # CarrierType namespace management
│
├── plugins/                              # built-in carrier plugins (reference implementations)
│   ├── mod.rs
│   ├── in_process.rs                     # same-memory-space carrier
│   ├── local.rs                          # same-host carrier
│   ├── loopback.rs                       # self-addressed carrier
│   ├── test.rs                           # deterministic carrier for tests
│   └── blackhole.rs                      # drops all traffic (chaos/test carrier)
│
├── tests/
│   ├── mod.rs
│   ├── endpoint_tests.rs
│   ├── datagram_tests.rs
│   ├── engine_tests.rs
│   ├── negotiation_tests.rs
│   ├── health_tests.rs
│   ├── addressing_tests.rs
│   ├── mtu_tests.rs
│   ├── fragmentation_tests.rs
│   ├── flow_control_tests.rs
│   ├── plugin_tests.rs
│   ├── quarantine_tests.rs
│   ├── prohibited_deps_tests.rs          # cargo-tree + grep audit: no banned crates
│   ├── property_tests.rs                 # proptest: 100k iterations
│   └── fuzz/
│       ├── mod.rs
│       ├── fuzz_endpoint.rs
│       ├── fuzz_datagram.rs
│       ├── fuzz_framing.rs
│       ├── fuzz_envelope.rs
│       ├── fuzz_fragmentation.rs
│       └── fuzz_negotiation.rs
│
├── benches/
│   ├── mod.rs
│   ├── send_receive_bench.rs
│   ├── fragmentation_bench.rs
│   ├── health_bench.rs
│   ├── negotiation_bench.rs
│   └── wire_bench.rs
│
├── spec/                                 # normative specifications
│   ├── carrier_spec.md                   # CarrierPlugin trait contract spec
│   ├── addressing_spec.md                # Endpoint addressing spec
│   ├── negotiation_spec.md               # capability negotiation spec
│   ├── health_spec.md                    # carrier health / phi spec
│   ├── framing_spec.md                   # wire envelope + fragmentation spec
│   ├── capability_spec.md                # CarrierCapabilities spec
│   └── threat_model.md                   # threat model document (mirrors section 4)
│
├── docs/
│   ├── architecture.md                   # crate architecture & layering
│   ├── integration_guide.md              # integrating ids-carrier into ids-* crates
│   ├── plugin_dev_guide.md               # writing a CarrierPlugin
│   ├── carrier_plugin_list.md            # built-in plugins and their contracts
│   ├── security_considerations.md        # security considerations
│   └── design_rationale.md               # why carrier-agnostic, unreliable-by-default
│
└── test_vectors/                         # deterministic test vectors
    ├── endpoint_vectors.json
    ├── datagram_vectors.json
    ├── framing_vectors.json
    ├── health_vectors.json
    ├── capability_vectors.json
    └── mtu_vectors.json
```

**Why ~94 files:** one file per responsibility — endpoint model, plugin engine, capability negotiation, health/phi, framing/fragmentation, flow control, addressing, five built-in plugins, unit/integration/property/fuzz tests, benchmarks, seven spec documents, docs, test vectors. No stubs; every file holds production-grade code with error handling, documentation, and tests. Falls within the required ~90–1000 files in subfolders.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Endpoint and Datagram
```
Endpoint = { carrier: CarrierType, address: Vec<u8> }   // address ≤ 1024 bytes, opaque
CarrierType = Loopback | Serial | Optical | Radio | Custom(u16)
Datagram = { endpoint: Endpoint, payload: Vec<u8>, seq: u64 }
DatagramId = { origin: Endpoint, seq: u64 }
```

#### 7.2 CarrierPlugin trait (all carriers are plugins)
```
trait CarrierPlugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn capabilities(&self) -> CarrierCapabilities;
    fn health(&self) -> CarrierHealth;
    fn send_datagram(&self, endpoint: &Endpoint, payload: &[u8]) -> Result<DatagramId, CarrierError>;
    fn receive_datagram(&self) -> Result<Option<Datagram>, CarrierError>;
}
```

#### 7.3 Capability negotiation (intersection)
```
local ∩ peer = CarrierCapabilities {
    max_datagram_size = min(local.max, peer.max),
    ordered = local.ordered && peer.ordered,
    reliable = local.reliable && peer.reliable,
    duplex = local.duplex && peer.duplex,
    fragmentation_supported = local.frag && peer.frag,
}
// send path: if payload.len() > negotiated.max_datagram_size → fragment (ids-carrier)
//             if !fragmentation_supported && overshoot → CarrierError::MtuExceeded
```

#### 7.4 Carrier health (phi accrual)
```
// phi = -log10(1 - F(delay)) where F is the CDF of heartbeat inter-arrival times
if phi > threshold (≈8–10): health = Unhealthy → quarantine
reconnect: backoff = min(initial * multiplier^attempt, max) + jitter(±20%)
```

#### 7.5 Length-prefixed framing + fragmentation
```
envelope = [u32 length] [payload bytes] [u32 digest(BLAKE3 or SHA-256 of payload)]
fragment i of n = { datagram_id, frag_index, frag_count, payload_slice }
reassemble: buffer by datagram_id; deliver only when all fragments present or deadline passed
deadline: creation_time + lifetime (creation-time-keyed, never RTT-based)
```

#### 7.6 Credit-based backpressure
```
credit_available(peer) -> usize; send_datagram decrements credit; plugin returns credits on
drain; credit == 0 → CarrierError::Backpressure (caller throttles, never blocks silently)
```

#### 7.7 What ids-carrier deliberately does NOT do
No ACK/retransmit (ids-protocol); no congestion-control algorithm in core (plugin may implement); no cryptography (ids-crypto); no address resolution (ids-discovery); no sockets (CarrierPlugin boundary).

---

### 8. Dependencies

|| Crate | Purpose ||
||-------|---------|---------|
|| `ids-000-aa` | `Endpoint`, `CarrierType`, `trait Carrier`, error contract, invariants (sole hard dependency) ||
|| `thiserror` | `CarrierError` enum ||
|| `bytes` | buffer handling for payloads ||
|| `serde` + `serde_bytes` | serialization of envelopes and test vectors ||
|| `blake3` | envelope payload digest (allowed crypto set) ||
|| `proptest` | property-based tests (dev-only) ||
|| `criterion` | benchmarks (dev-only) ||

**Prohibited dependencies (audited by test):** `std::net`, `tokio::net`, `tokio` runtime (core crate is sync/async-agnostic), `libp2p`, any IP/TCP/UDP/QUIC/HTTP/DNS crate, Wi-Fi/Bluetooth/cellular/VPN/mesh crates. `sha256` usage limited to envelope digest if chosen over BLAKE3; `xchacha20poly1305`, `chacha20poly1305`, `ed25519` are **not** used by `ids-carrier` (crypto boundary: `ids-crypto`).

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| AC-002.1 | Byte-exact datagram round trip | send_datagram → receive_datagram over in_process carrier returns identical `Vec<u8>`, seq matched, 10k iterations ||
|| AC-002.2 | Endpoint is the only address type | All send/recv APIs take/return `ids-carrier::Endpoint`; grep audit confirms no `SocketAddr`, `IpAddr`, port, or DNS type in `ids-carrier` ||
|| AC-002.3 | Zero prohibited dependencies | `cargo tree` + grep audit: no std::net/tokio::net/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS/Wi-Fi/Bluetooth/cellular/VPN/mesh crate or module anywhere in `ids-carrier` ||
|| AC-002.4 | Capability negotiation converges | Two peers exchange `CarrierCapabilities`; negotiated `max_datagram_size` = min; mismatched reliability → neither peer claims reliable ||
|| AC-002.5 | MTU enforcement | Payload > negotiated MTU is fragmented or rejected with `CarrierError::MtuExceeded`; never silently truncated; reassembly is byte-exact ||
|| AC-002.6 | Carrier health + quarantine | Phi above threshold marks peer `Unhealthy`; sends return `CarrierError::CarrierUnhealthy`; backoff+jitter reconnect recovers ||
|| AC-002.7 | Opaque payloads | `ids-carrier` never inspects payload bytes beyond digest/length; test with entropy-random payloads passes unchanged ||
|| AC-002.8 | No cryptography in carrier | Only BLAKE3/SHA-256 envelope digest; no ed25519/xchacha20poly1305/chacha20poly1305 imports in `ids-carrier` ||
|| AC-002.9 | Deterministic test carrier | `test` plugin: fixed seed → identical send/recv sequence across runs ||
|| AC-002.10 | Property tests | 100,000+ proptest iterations: endpoint round-trip, datagram seq monotonicity, fragment reassembly, capability intersection idempotence ||
|| AC-002.11 | Fuzz targets pass | malformed envelopes, oversized fragments, capability chaos → all fail-safe (error returned, no panic) ||
|| AC-002.12 | Backpressure | `credit == 0` → `send_datagram` returns `CarrierError::Backpressure` without blocking ||
|| AC-002.13 | Plugin isolation | One plugin's failure/panic boundary does not corrupt another plugin's state (test via blackhole + in_process coexistence) ||

---

### 10. Remaining Uncertainties

1. **Closed vs open `CarrierType`:** plan-001 sketches a closed enum (`Loopback, Serial, Optical, Radio, Custom(u16)`). Confirm whether user plugins must always use `Custom(u16)` or whether the enum should be `#[non_exhaustive]`/plugin-registered.
2. **Sync vs async plugin trait:** `ids-carrier` core is sync; plugins may be async internally. Decide whether `CarrierPlugin::send_datagram` returns `Result` or `impl Future` — no `tokio` dependency, so any async is plugin-private behind the trait.
3. **Dynamic plugin loading:** `libloading`-style loading is opt-in and security-sensitive; decide whether it ships at all or whether plugins are linked statically only.
4. **Datagram size ceiling:** 1024-byte address cap comes from plan-001; the payload max (negotiated MTU default) has no agreed default value yet — workload-dependent.
5. **Fragmentation reassembly deadline:** creation-time-keyed lifetime is chosen, but the default deadline value and cleanup policy are unquantified.
6. **Address provisioning:** how endpoints learn peer addresses (static config vs `ids-discovery` gossip) is deferred to `ids-discovery`; `ids-carrier` provides only the hook.
7. **Credit exhaustion policy:** whether `send_datagram` blocks, errors, or drops when peer credit is exhausted needs a product decision (plan: return `CarrierError::Backpressure`).
8. **no_std support:** whether `ids-carrier` compiles `no_std` (embedded carriers) is undecided; `features.rs` reserves the flag.

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|-------------|
|| Separate `ids-carrier` crate (not subfolder) | User rule: separate crates instead of more folders; keeps carrier boundary crisp ||
|| `Endpoint` = `CarrierType` + opaque `Vec<u8>` (≤1024 B) | Matches plan-001 `ids-000-aa` design; sole IDS address type; no IP/DNS ||
|| `ids-carrier::Endpoint` is the traffic surface | Re-exports `ids-000-aa::Endpoint`; all 99 crates route through it ||
|| Unreliable datagrams by default | Loss/reorder/dup expected; reliability belongs to `ids-protocol`; keeps carrier thin ||
|| `CarrierPlugin` trait + registry | Every carrier is a plugin; core imports zero sockets; user carriers via `Custom(u16)` ||
|| Capability intersection before first send | SDP offer/answer pattern; MTU and reliability negotiated explicitly ||
|| Length-prefixed envelope + BLAKE3/SHA-256 digest | Length-prefix suits opaque binary; digest is the only integrity mechanism, using allowed primitives ||
|| Phi accrual carrier health + quarantine | Adaptive suspicion absorbs jitter/GC pauses (Hazelcast/Akka); quarantine avoids silent eternal retry ||
|| Exponential backoff with jitter for reconnect | gRPC pattern; prevents thundering herd after carrier outage ||
|| Per-peer credit-based backpressure | Sender never overwhelms receiver; `CarrierError::Backpressure` is explicit, never silent blocking ||
|| No crypto in `ids-carrier` | Crypto boundary is `ids-crypto`; allowed primitives (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305) apply only above ||
|| No `std::net`/`tokio::net`/libp2p/IP/TCP/UDP/QUIC/HTTP/DNS | Carrier-agnostic absolute rule; audited by `prohibited_deps_tests` ||

---

### 12. Sources consulted (30 targeted searches)

1. kcp-tokio `Transport` trait — datagram transport with associated `Addr` type (`send_to`/`recv_from`/`local_addr`): https://docs.rs/kcp-tokio/latest/kcp_tokio/transport/trait.Transport.html
2. reconcile-gossip `Transport` trait — connectionless datagram I/O pattern: https://docs.rs/reconcile-gossip/latest/reconcile_gossip/transport/trait.Transport.html
3. Memory-native protocol stack — carriage layer = send/receive/discover over opaque byte sequences, carrier-independent integrity: https://qu3ry.net/articles/memory-native-protocol/transport-agnosticism
4. SDP offer/answer capability negotiation (RFC 3264 model, potential configurations): https://blog.webex.com/engineering/understanding-sdp-offer-answer-negotiation
5. Hazelcast phi accrual failure detector — adaptive suspicion over heartbeat inter-arrival times: https://docs.hazelcast.com/hazelcast/5.7/clusters/phi-accrual-detector
6. Akka phi accrual failure detector — `phi = -log10(1 - F(delay))`, accrual vs binary up/down: https://doc.akka.io/libraries/akka-core/current/typed/failure-detector.html
7. "An Internet without the Internet protocol" — eliminating IP addresses as end-host identifiers: https://doi.org/10.1016/j.comnet.2010.06.009
8. draft-guruprasad-addressless-internet — the case for an addressless Internet: https://tools.ietf.org/html/draft-guruprasad-addressless-internet-00
9. Computer Networks: A Systems Approach — reliable transmission, ARQ (stop-and-wait, go-back-N, selective repeat): https://book.systemsapproach.org/direct/reliable.html
10. LAMS-DLC — NAK-based reliable datagram service for satellite links: https://ieeexplore.ieee.org/document/365441
11. RFC 4340 — Datagram Congestion Control Protocol (congestion-controlled unreliable datagrams): https://datatracker.ietf.org/doc/html/rfc4340
12. Backpressure Flow Control (NSDI'22) — per-hop per-flow pause/resume with bounded state: https://usenix.org/system/files/nsdi22-paper-goyal.pdf
13. draft-ietf-rtcweb-data-channel — WebRTC data channels: reliable/unreliable/partial-reliable datagram modes: https://tools.ietf.org/html/draft-ietf-rtcweb-data-channel-02
14. WebRTC data channels — ordered/unordered, reliable/unreliable delivery semantics: https://web.dev/articles/webrtc-datachannels
15. RFC 9221 — Unreliable Datagram Extension to QUIC (research only; QUIC is prohibited in IDS): https://datatracker.ietf.org/doc/html/rfc9221
16. ZeroMQ — transport-agnostic sockets, opaque message blobs, multiple underlying transports: https://en.wikipedia.org/wiki/ZeroMQ
17. NNG man page — endpoints, listeners/dialers, pipes over pluggable transports: https://github.com/nanomq/NanoNNG/blob/main/docs/man/nng.7.adoc
18. Akka.Remote — location-transparent remoting, remote addressing model: https://github.com/akkadotnet/akka.net/blob/dev/docs/articles/remoting/index.md
19. RFC 9299 — Locator/ID Separation Protocol (LISP) architecture (research only; no IP in IDS carrier): https://ietf.org/ietf-ftp/rfc/rfc9299.pdf
20. IPFS routing discussion — content-addressed routing without DNS: https://discuss.ipfs.io/t/how-does-resolution-and-routing-work-with-ipfs/365
21. `datagram::DatagramSender` — Rust datagram send trait: https://docs.rs/datagram/latest/datagram/trait.DatagramSender.html
22. embedded-nal — `no_std` UDP datagram stack for embedded (UdpStack/ConnectedUdp): https://docs.rs/embedded-nal/0.5.0/x86_64-apple-darwin/src/embedded_nal/stack/udp.rs.html
23. io_uring zero-copy networking — zero-copy send/receive (implementation reference for large payloads): https://docs.kernel.org/networking/iou-zcrx.html
24. RFC 5740 — NORM: NACK-Oriented Reliable Multicast transport protocol: https://iana.org/go/rfc5740
25. RFC 8656 — TURN relay extensions (anti-example research; NAT traversal out of scope): https://rfc-editor.org/rfc/rfc8656
26. gRPC Connection Backoff Protocol — exponential backoff with jitter: https://grpc.github.io/grpc/core/md_doc_connection-backoff.html
27. EECS 489 DHT lecture — consistent hashing, Chord, coordinator-free routing: https://web.eecs.umich.edu/~sugih/courses/eecs489/lectures/10-DHT.pdf
28. draft-verma-cirp — Capability-Oriented Intent Routing Protocol (research only): https://datatracker.ietf.org/doc/draft-verma-cirp
29. PTRS — Pluggable Transports in Rust (`PluggableTransport` trait, client/server builders): https://docs.rs/ptrs-gesher/latest/ptrs_gesher/trait.PluggableTransport.html
30. OpenCarrier Plugin SDK — write carrier plugins in safe Rust: https://docs.rs/opencarrier-plugin-sdk/latest/opencarrier_plugin_sdk

**Research only — no code written, no files created, no repository modified.**
