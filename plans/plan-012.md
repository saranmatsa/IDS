## Phase 012 — Create the ids-protocol crate: HX-241-style native frame format, self-defined, carrier-agnostic

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

Note on numbering: prompts.txt entry 12 is ids-protocol ("Create the ids-protocol crate: HX-241-style native frame format, self-defined, carrier-agnostic."). Entry 13 (ids-write) is a separate phase.

---\n### 1. Executive Summary

Phase 012 creates `ids-protocol`, the IDS native wire/message format crate. It defines a self-defined, HX-241-style carrier-agnostic frame format: every unit of traffic is a `Frame` carrying a frame header, frame body, sequencing, acknowledgements, fragmentation/reassembly metadata, delivery-semantics flags, flow-control credits, loss-detection state, and retransmission markers. `ids-protocol` knows nothing about the transport: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic rides `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`), making the crate usable over any carrier — reliable, unreliable, datagram, stream, store-and-forward, or opaque.

Contents per prompts.txt entry 12 and CRATES.md §12: frame header, frame body, framing, sequencing, acknowledgements, fragmentation/reassembly, delivery semantics, stream/session semantics, flow control, loss detection, retransmission, protocol invariants. All crypto is confined to the allowed set only: ed25519 (signatures), xchacha20poly1305 / chacha20poly1305 (AEAD), sha256 and blake3 (hashing/checksums). `ids-000-aa` is the master crate holding every canonical type and sealed trait contract; `ids-protocol` depends on `ids-000-aa` for frame types (`FrameId`, `SeqNum`, `FrameType`, `DeliverySemantics`, `Checksum`, `Signature`, `Endpoint`) and only consumes type surface from `ids-crypto` (no runtime dep, preserving the no-cycle rule).

`ids-protocol` does NOT yet exist at `C:/idfs/ids-protocol/`. This plan specifies creation from scratch; no source modifications are made in this phase. Research basis: 30 parallel searches covering length-prefixed framing (tokio-util `LengthDelimitedCodec`, krot-proto postcard framing), SCALE codec (Parity), protobuf length-delimited varints, SSH binary packet protocol (RFC 4253), AMQP 1.0 frame header (8-byte fixed header), MQTT 5.0 variable-byte-integer remaining length, gRPC length-prefixed message over HTTP/2, Yamux credit-based flow control, QUIC loss detection (RFC 9002), QUIC stream credit-based flow control (RFC 9000), TCP sliding window / PAWS (RFC 7323/1323), selective-repeat ARQ, QUIC ACK scheduling, Kafka at-least-/exactly-once semantics, TLA+/IC3 distributed protocol verification, Rust `nom` zero-copy binary parsing, `bytes::BytesMut` zero-copy buffers, Cap'n Proto / FlatBuffers zero-copy comparison, UDT reliable-over-UDP, OSI session layer (stream vs datagram semantics), Merkle/MMR synchronization, XChaCha20-Poly1305 (libsodium), Ed25519 RFC 8032, BLAKE3.

---\n### 2. Architecture Overview

`ids-protocol` is the wire-format and reliability layer for all IDS inter-node traffic. It is not a transport: it serializes application/control messages into carrier-agnostic `Frame`s that `ids-carrier::Endpoint` ships. Reliability (sequencing, acks, retransmission, flow control) is implemented inside the frame format itself so that any carrier — reliable or unreliable — can carry IDS traffic without change.

```
┌─────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / CONTROL LAYER                       │
│  ids-rpc · ids-pubsub · ids-replication · ids-sync · ids-session   │
├─────────────────────────────────────────────────────────────────────┤
│                    Phase 012: ids-protocol (THIS CRATE)             │
│  ┌─────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌─────────┐  │
│  │ frame header│ │ frame body   │ │ framing         │ │ sequen- │  │
│  │ (type/flags │ │ (payload     │ │ (length prefix, │ │ cing    │  │
│  │  + meta)    │ │  opaque      │ │  checksum,      │ │  (SeqNum│  │
│  │            │ │  Vec<u8>)    │ │  crypto block)  │ │   + ACK)│  │
│  └─────────────┘ └──────────────┘ └─────────────────┘ └─────────┘  │
│  ┌─────────────┐ ┌──────────────┐ ┌─────────────────┐ ┌─────────┐  │
│  │ fragmentation│ │ delivery     │ │ flow control    │ │ loss    │  │
│  │ /reassembly │ │ semantics    │ │ (credits,       │ │ detection│ │
│  │ (frag id/   │ │ (at-most/    │ │  window)        │ │  (loss   │ │
│  │  offset/total)│ │  at-least/   │ │                 │ │  + PTO) ││
│  │            │ │  exactly-once)│ │                 │ │         │ │
│  └─────────────┘ └──────────────┘ └─────────────────┘ └─────────┘  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ stream/session semantics · retransmission · protocol invariants│  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                               │ Endpoint (ids-carrier, opaque)
                               │ CarrierType + Vec<u8> only
┌──────────────────────────────▼──────────────────────────────────────┐
│              CARRIER LAYER (ids-carrier, Phase 002)                 │
│  any carrier: reliable, unreliable, datagram, stream, store-fwd     │
└─────────────────────────────────────────────────────────────────────┘
```

CARRIER BOUNDARY (non-negotiable): `ids-carrier::Endpoint` is the sole address and envelope type. `ids-protocol` never imports `std::net`, `tokio::net`, libp2p, or any carrier-specific crate.

**Design rules (non-negotiable):**
- Self-defined native frame format: header + body, length-prefixed or varint-delimited, checksummed per frame (blake3 or sha256 — CRC not in allowed crypto).
- Sequencing: monotonic `SeqNum` per session/endpoint; ACKs carried in-frame (piggybacked) or as pure ACK frames.
- Acknowledgements: cumulative + selective (SACK-style bitmask) to tolerate reordering/loss without RTT-bound timers.
- Fragmentation/reassembly: per-frame MTU-aware splitting with fragment ID + offset + total; reassembly deadline keyed to creation time (never RTT).
- Delivery semantics: `AtMostOnce`, `AtLeastOnce`, `ExactlyOnce` per frame (idempotency via frame dedup window).
- Stream/session semantics: ordered byte-stream sessions and datagram sessions coexist; stream reassembly preserves order within a session.
- Flow control: credit-based (per-session credit window), no RTT dependence.
- Loss detection: packet/time thresholds per QUIC RFC 9002 (per-packet-number-space loss detection, PTO probe), keyed to monotonic clocks, never raw wall-clock validity.
- Retransmission: selective retransmit of unacked frames after loss detection or PTO expiry; new frame number on retransmit (QUIC-style) to avoid retransmission ambiguity.
- Protocol invariants: frame length bounded; checksum verified before any semantic processing; sequencing gaps bounded; dedup window bounded; no frame ever carries carrier addressing.
- Allowed crypto only: ed25519 (frame signature), xchacha20poly1305/chacha20poly1305 (frame AEAD encryption), sha256/blake3 (frame checksum). No other crypto.

---\n### 3. Detailed Research Dimensions

#### 3.1 Frame Header & Frame Body (native wire format)
Every `Frame` = fixed/variable header + opaque body. Header fields: frame type (u8), flags (u8), version (u8), `FrameId` (u64, globally unique per origin), `SeqNum` (u64 per session), ACK field (cumulative ack + SACK bitmask), length (varint or fixed u32), checksum (blake3/sha256 over header+body), optional signature (ed25519 over canonical encoding), optional crypto block (xchacha20poly1305/chacha20poly1305 nonce + ciphertext tag). Body = opaque `Vec<u8>` payload. Reference layouts: SSH binary packet protocol (RFC 4253: packet_length/padding_length/payload/padding/MAC), AMQP 1.0 frame header (8-byte fixed: SIZE u32, DOFF, TYPE, channel), MQTT 5.0 fixed header (packet type/flags + variable-byte-integer remaining length), gRPC length-prefixed message (1-byte compressed flag + 4-byte big-endian length + message). tokio-util `LengthDelimitedCodec` and krot-proto postcard framing confirm varint length prefixes are standard and safe when max-frame-size is enforced. SCALE codec (Parity/Substrate) shows a compact little-endian aggregate encoding suitable when self-describing framing is unnecessary.

#### 3.2 Framing (length-prefix vs varint vs delimiter)
Length-prefixed framing solves "where does one message end" on byte streams. Options: fixed-width length (1/2/4/8 bytes, wire_codec `LengthPrefixed`), varint length (LEB128, protobuf/gRPC/krot-proto style), delimiter (newline/0xCE, AMQP 0-9-1 frame end). For carrier-agnostic `Vec<u8>` payloads over possibly-datagram carriers, length-prefix + max-frame-size bound is safest: receiver reads length, then exactly N bytes, bounds memory against framing bombs. Varint (protobuf base-128 varints) compact for small frames; fixed u32 simpler for hardware/ASCII boundaries. Decision: u32 little-endian length prefix + varint option as feature, max frame size configurable (default 1 MiB), checksum appended per frame (wire_codec `Checksummed` decorator pattern adapted to blake3/sha256).

#### 3.3 Sequencing & Acknowledgements
Per-session `SeqNum` (u64) monotonic, assigned at frame creation. Sender tracks unacked frames in a send buffer; receiver tracks highest in-order seq and a SACK bitmask of received out-of-order frames. ACK frames carry cumulative ack + selective bitmask (TCP SACK / QUIC ACK frame pattern). Cumulative ACK acknowledges everything up to N; SACK bitmask covers received-but-not-cumulative frames. Receiver sends ACKs on data frames (piggyback) and as pure ACK frames when no outbound data (QUIC ack-eliciting packet pattern, ack-frequency guidance). QUIC loss detection (RFC 9002): packet declared lost if unacked, in flight, and sent prior to an acked packet; packet threshold kPacketThreshold=3 or time threshold; PTO timer per packet-number-space sends probe packets. IDs-protocol adapts: loss detection per session, PTO per session, no RTT-bound timeouts (absolute deadlines keyed to frame creation time).

#### 3.4 Fragmentation & Reassembly
Frames exceeding carrier MTU (or configured max) are fragmented: fragment header carries `FragId` (u64, per original frame), `FragOffset` (u32, multiple of 8 like IP), `FragTotal` (u16), `MoreFragments` flag. Reassembly buffer per `FragId` with creation-time-keyed expiry (IP-style reassembly timer, but keyed to creation time not RTT). Incomplete reassembly at deadline = discard fragment set (never guess). IP fragmentation precedent (ID/flags/offset, destination-only reassembly, multiples-of-8 offsets) adapted to frame-internal framing rather than network layer.

#### 3.5 Delivery Semantics
Three per-frame delivery semantics: `AtMostOnce` (send once, no retry, may be lost), `AtLeastOnce` (retry until acked, dedup at receiver), `ExactlyOnce` (idempotent apply via frame dedup window + apply-once marker, Kafka idempotent-producer / RIFL exactly-once pattern). Dedup: receiver keeps seen `FrameId`s within a bounded window; duplicate = drop, re-ack prior result. Window expiry = duplicate-admission event, not silent cleanup (Machine-Checked Dual-Write Recovery pattern). Idempotency key = `FrameId` (origin + seq). Exactly-once requires sender+receiver cooperation (Kafka principle).

#### 3.6 Stream / Session Semantics
Two session types: stream sessions (ordered byte stream, like TCP stream / QUIC STREAM: offsets reassemble in order, frame boundaries not preserved) and datagram sessions (message boundaries preserved, like UDP datagram / QUIC datagram extension). Stream session state: next-expected offset, receive buffer for out-of-order data, flow-control credit per stream. Session ID in frame header binds frames to a session; sessions created via SYN/SYN-ACK handshake (yamux-style SYN flag on new stream) and torn down with FIN/GOAWAY. OSI session-layer precedent: dialog control, synchronization points/checkpoints, half/full-duplex. IDs-protocol keeps session state minimal: ID, type, next offset/seq, credit, open/closed state.

#### 3.7 Flow Control (credit-based)
Credit-based flow control (QUIC stream + connection level, HTTP/2 WINDOW_UPDATE, yamux Window Update): receiver advertises credit (bytes it can buffer); sender may not send beyond outstanding unacked bytes ≤ credit. Receiver sends Window Update frames as buffer drains. Two levels: per-session credit and per-frame (connection) credit. Yamux: 256KiB initial stream window, no session window; window updates sent regularly to avoid stalls. IDs-protocol: credit carried in frame header flags/field or dedicated credit frame; no RTT dependence; credit exhaustion = backpressure (sender waits), distinct from carrier backpressure.

#### 3.8 Loss Detection & Retransmission
Loss detection per QUIC RFC 9002: track sent frames per packet-number-space-equivalent (session); frame declared lost when (a) unacked, in flight, sent prior to an acked frame, and (b) kPacketThreshold=3 packets after an ack or time threshold elapsed. PTO timer per session: on expiry send at least one ack-eliciting probe frame (up to two datagrams). Retransmission: lost frame re-sent with a NEW `SeqNum` (QUIC removes retransmission ambiguity — new frame number means an ACK cannot be confused about which transmission is acknowledged). Original frame data identical; only seq number changes. Reordering tolerance via SACK bitmask; spurious retransmit detection if original later acked (increase reordering threshold). No RTT-estimate timers; absolute deadlines keyed to frame creation time.

#### 3.9 Protocol Invariants & Safety
- Frame length ≤ max_frame_size (reject oversized before allocation — framing bomb guard).
- Checksum (blake3/sha256) verified before any semantic processing; mismatch = drop frame, count error, never act on corrupt frame.
- `SeqNum` monotonic per session; wraparound handled by modular comparison (TCP PAWS protects against old duplicates after wrap; timestamps monotone non-decreasing).
- Dedup window bounded; oldest evicted on pressure (LRU + TTL).
- Reassembly buffers bounded per `FragId`; expiry by creation-time deadline.
- Credit never exceeded by sender; receiver enforces.
- Crypto confined to allowed set; no frame carries plaintext keys.
- Invariant: an ACKed frame is never retransmitted; a lost frame is retransmitted at most N times before session error.
- Fail-closed: on invariant violation, session enters error state, not silent corruption.

---\n### 4. Threat Model & Mitigations

| Threat | Mitigation |
|---|---|
| Framing bomb / oversized length prefix exhausts memory | Enforce max_frame_size before allocating; reject-and-count per peer |
| Corrupt frame acted on | Per-frame blake3/sha256 checksum verified before semantics; mismatch = drop |
| Replay of old frame | Dedup window on FrameId; monotonic SeqNum; PAWS-style timestamp rejection of old duplicates |
| Retransmission ambiguity (old vs new transmission confused) | New SeqNum on retransmit (QUIC-style); ACK cannot be confused |
| Sequence number wraparound | Modular comparison + monotone timestamps (PAWS) |
| Flow-control credit exceeded | Sender bounded by advertised credit; receiver enforces |
| Reassembly never completes | Creation-time-keyed deadline; discard incomplete set |
| Dedup window DoS | Bounded LRU + TTL eviction; per-peer quota |
| Disallowed crypto in frame | Dependency audit bans non-allowed crates; only ed25519/xchacha20poly1305/chacha20poly1305/sha256/blake3 |
| Carrier-agnostic rule violation (std::net etc.) | Endpoint sole address type; source grep assertion in tests |
| Session stall (credit never replenished) | Window update frames sent regularly (yamux pattern); stall detection |
| Unbounded send buffer | Bound unacked bytes; apply backpressure / reject new frames |

---\n### 5. Integration with Existing Crates

| Crate | Role in Phase 012 |
|---|---|
| `ids-000-aa` (master) | Canonical types: `FrameId`, `SeqNum`, `FrameType`, `DeliverySemantics`, `Checksum`, `Signature`, `Endpoint`, sealed trait contracts, invariants, errors |
| `ids-carrier` (Phase 002) | `Endpoint` opaque transport (`CarrierType` + `Vec<u8>`); sole carrier boundary; no `std::net` |
| `ids-crypto` (Phase 003) | Type surface only (`Checksum`, `Signature`); no runtime dependency (no-cycle rule: `ids-crypto` does not depend on `ids-000-aa`) |
| `ids-session` | Consumes `ids-protocol` frames for session lifecycle (stream/datagram sessions) |
| `ids-framing` | Potential future split; framing logic lives in `ids-protocol` for now |
| All other `ids-*` crates | Depend on `ids-000-aa`; `ids-protocol` is a low-level dependency consumed by higher crates |

Integration points:
- `ids-protocol` never touches the network directly; all output is `Vec<u8>` frames handed to `ids-carrier::Endpoint`.
- `ids-protocol` depends on `ids-000-aa` only (plus type surface from `ids-crypto`, no runtime dep).
- Cargo workspace member; `ids-protocol = { path = "../ids-protocol" }`.
- No dependency on `std::net`, `tokio::net`, libp2p, or any carrier-specific crate.

---\n### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-protocol` (new at `C:/idfs/ids-protocol/`)

```
ids-protocol/
├── Cargo.toml                          # blake3, sha2, thiserror, serde, bytes; workspace member; depends on ids-000-aa
├── lib.rs                              # crate root; module declarations + root re-exports
├── constants.rs                        # protocol constants (max frame size, default credit, dedup window)
├── error.rs                            # ProtocolError (thiserror): framing, checksum, sequencing, flow-control
├── prelude.rs                          # re-export glob
│
├── frame/                              # frame model: header + body
│   ├── mod.rs
│   ├── frame.rs                        # Frame struct (header + body Vec<u8>)
│   ├── frame_id.rs                     # FrameId (origin + sequence, globally unique)
│   ├── seq_num.rs                      # SeqNum (u64 per session, monotonic)
│   ├── frame_type.rs                   # FrameType enum (DATA, ACK, CREDIT, FRAG, SYN, FIN, GOAWAY, PING, PONG)
│   ├── flags.rs                        # frame flags bitfield
│   ├── version.rs                      # frame format version
│   ├── header.rs                       # FrameHeader: type, flags, seq, ack, length, checksum, optional sig/crypto
│   ├── body.rs                         # FrameBody opaque Vec<u8> + accessors
│   ├── checksum.rs                     # per-frame blake3/sha256 checksum
│   ├── signature.rs                    # optional ed25519 signature over canonical encoding
│   ├── crypto_block.rs                 # optional xchacha20poly1305/chacha20poly1305 AEAD block
│   ├── serialization.rs                # Frame -> bytes (encode/decode, length prefix)
│   ├── size.rs                         # frame size estimation & MTU checks
│   └── invariants.rs                   # protocol invariant checks (length bound, seq monotonic)
│
├── framing/                            # framing layer: length prefix / varint / delimiter
│   ├── mod.rs
│   ├── length_prefix.rs                # fixed u32/u64 length-prefix framing
│   ├── varint.rs                       # LEB128 varint length (protobuf/gRPC style)
│   ├── delimiter.rs                    # byte-delimited framing (0xCE / newline)
│   ├── checksummed.rs                  # checksum decorator over any framer (wire_codec Checksummed pattern)
│   ├── max_frame.rs                    # max_frame_size enforcement (framing bomb guard)
│   ├── encoder.rs                      # encode frame to Vec<u8>
│   ├── decoder.rs                      # decode frame from &[u8] (partial reads, BytesMut)
│   └── codec.rs                        # Framer trait + implementations
│
├── sequencing/                         # sequencing & acknowledgements
│   ├── mod.rs
│   ├── seq_space.rs                    # sequence number space (modular comparison, wrap)
│   ├── ack.rs                          # ACK frame: cumulative + SACK bitmask
│   ├── sack.rs                         # selective ACK bitmask (received out-of-order)
│   ├── ack_tracker.rs                  # sender: tracks unacked frames, cumulative ack state
│   ├── pto.rs                          # PTO timer per session (probe frames)
│   ├── loss_detection.rs               # QUIC RFC 9002 loss detection (packet/time thresholds)
│   ├── retransmit.rs                   # selective retransmit with new SeqNum
│   └── dedup.rs                        # receiver dedup window (FrameId → seen)
│
├── fragmentation/                      # fragmentation / reassembly
│   ├── mod.rs
│   ├── frag_id.rs                      # FragId (per original frame)
│   ├── frag_header.rs                  # FragOffset, FragTotal, MoreFragments flag
│   ├── fragmenter.rs                   # split frame into MTU-sized fragments
│   ├── reassembler.rs                  # per-FragId reassembly buffer
│   ├── reassembly_deadline.rs          # creation-time-keyed expiry (never RTT)
│   └── frag_manager.rs                 # manage multiple in-flight fragment sets
│
├── delivery/                           # delivery semantics
│   ├── mod.rs
│   ├── semantics.rs                    # DeliverySemantics enum (AtMostOnce/AtLeastOnce/ExactlyOnce)
│   ├── at_least_once.rs                # retry-until-acked logic
│   ├── exactly_once.rs                 # idempotent apply via dedup + apply-once marker
│   ├── dedup_window.rs                 # bounded dedup window (LRU + TTL)
│   └── idempotency.rs                  # apply-once guarantee
│
├── session/                            # stream/session semantics
│   ├── mod.rs
│   ├── session_id.rs                   # SessionId
│   ├── session_type.rs                 # StreamSession / DatagramSession
│   ├── session_state.rs                # open/closed, next offset, next seq, credit
│   ├── stream_session.rs               # ordered byte-stream reassembly by offset
│   ├── datagram_session.rs             # message-boundary-preserving session
│   ├── handshake.rs                    # SYN/SYN-ACK session setup (yamux-style)
│   └── teardown.rs                     # FIN/GOAWAY session teardown
│
├── flow_control/                       # credit-based flow control
│   ├── mod.rs
│   ├── credit.rs                       # credit window (bytes)
│   ├── window_update.rs                # Window Update frame
│   ├── session_credit.rs               # per-session credit
│   ├── connection_credit.rs            # per-connection credit
│   └── backpressure.rs                 # credit exhaustion → backpressure signal
│
├── timing/                             # timing: creation-time keyed, no RTT dependence
│   ├── mod.rs
│   ├── clock.rs                        # monotonic clock abstraction
│   ├── creation_time.rs                # frame creation timestamp
│   ├── deadline.rs                     # absolute deadlines (reassembly, dedup)
│   ├── pto_timer.rs                    # PTO timer wheel per session
│   └── monotonic.rs                    # monotonic interval arithmetic
│
├── carrier/                            # carrier-agnostic boundary
│   ├── mod.rs
│   └── endpoint_binding.rs             # Endpoint usage only, no std::net
│
├── crypto_surface/                     # type-level crypto consumption only
│   ├── mod.rs
│   ├── frame_checksum.rs               # blake3/sha256 Checksum binding
│   ├── frame_signature.rs              # ed25519 Signature binding
│   └── frame_aead.rs                   # xchacha20poly1305/chacha20poly1305 AEAD binding
│
├── invariants/                         # protocol invariant verification
│   ├── mod.rs
│   ├── frame_invariants.rs             # length bound, seq monotonic, checksum verified
│   └── session_invariants.rs           # credit bound, dedup bounded
│
├── tests/                              # conformance tests (no source modification)
│   ├── frame_tests.rs                  # encode/decode round-trip
│   ├── framing_tests.rs                # length-prefix/varint/delimiter
│   ├── checksum_tests.rs               # blake3/sha256 verification
│   ├── sequencing_tests.rs             # seq space, wrap, SACK
│   ├── ack_tests.rs                    # cumulative + selective ack
│   ├── loss_detection_tests.rs         # QUIC thresholds, PTO
│   ├── retransmit_tests.rs             # new SeqNum retransmit
│   ├── fragmentation_tests.rs          # split/reassemble, out-of-order
│   ├── delivery_tests.rs               # at-most/at-least/exactly-once
│   ├── dedup_tests.rs                  # dedup window
│   ├── session_tests.rs                # stream/datagram, handshake, teardown
│   ├── flow_control_tests.rs           # credit, window update, backpressure
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net usage
│   ├── crypto_surface_tests.rs         # asserts only allowed crypto types
│   └── invariant_tests.rs              # protocol invariant assertions
│
├── fuzz/                               # fuzzing harnesses
│   ├── mod.rs
│   ├── fuzz_frame.rs
│   ├── fuzz_framing.rs
│   └── fuzz_sequencing.rs
│
├── benches/                            # benchmarks
│   ├── mod.rs
│   ├── frame_bench.rs
│   ├── framing_bench.rs
│   └── flow_control_bench.rs
│
├── spec/                               # normative specifications
│   ├── frame_spec.md                   # frame format & lifecycle spec
│   ├── sequencing_spec.md              # sequencing & ack spec
│   ├── flow_control_spec.md            # credit-based flow control spec
│   └── threat_model.md                 # threat model document
│
├── docs/                               # documentation
│   ├── architecture.md
│   ├── integration_guide.md
│   └── carrier_agnostic_guide.md
│
└── test_vectors/                       # deterministic test vectors
    ├── frame_vectors.json
    ├── ack_vectors.json
    └── fragmentation_vectors.json
```

Total: ~90–150 files (within ~90–1000 target; room for growth). All `.rs` source + `Cargo.toml` + tests; no binary targets.

---\n### 7. Key Algorithms & Data Structures

1. **Frame encode/decode** — length prefix (u32 LE or varint LEB128) + header + body + checksum; max_frame_size enforced before allocation. Sources: tokio-util `LengthDelimitedCodec`, protobuf base-128 varints, gRPC length-prefixed message.
2. **Sequence number space** — u64 `SeqNum` per session; modular comparison for wraparound; monotonic timestamps (PAWS) reject old duplicates. Sources: TCP PAWS (RFC 1323/7323), QUIC packet numbers (RFC 9000).
3. **Cumulative + SACK acknowledgement** — cumulative ack (everything up to N) + selective bitmask for out-of-order received frames; piggybacked on data frames or pure ACK frames. Sources: TCP SACK, QUIC ACK frame (RFC 9000).
4. **Loss detection (QUIC RFC 9002)** — frame lost if unacked, in flight, sent prior to an acked frame, and past packet threshold (k=3) or time threshold; PTO per session sends probe frames. Source: RFC 9002 §6.
5. **Selective retransmit with new SeqNum** — retransmitted frame gets a new `SeqNum` to remove retransmission ambiguity; data identical. Source: QUIC retransmission (RFC 9000).
6. **Dedup window (exactly-once)** — bounded FrameId → seen map; duplicate = drop + re-ack; window expiry = duplicate-admission event. Sources: Kafka idempotent producer, RIFL exactly-once, Machine-Checked Dual-Write Recovery.
7. **Fragmentation/reassembly** — FragId + offset + total + MoreFragments; reassembly buffer per FragId; creation-time-keyed deadline; discard incomplete. Source: IP fragmentation (ID/flags/offset), adapted frame-internal.
8. **Credit-based flow control** — receiver advertises credit; sender bounded by outstanding ≤ credit; Window Update frames replenish. Sources: QUIC stream/connection flow control, HTTP/2 WINDOW_UPDATE, yamux.
9. **Stream session reassembly** — next-expected offset + receive buffer; out-of-order data buffered; delivered in order. Source: QUIC STREAM frames, TCP byte stream.
10. **Datagram session** — message boundaries preserved; no reassembly ordering beyond seq. Source: QUIC datagram, UDP datagram semantics.
11. **Checksum per frame** — blake3 (preferred speed) or sha256; CRC32C not used (not in allowed crypto). Sources: BLAKE3 spec, NIST FIPS 180-4.
12. **Protocol invariants** — length bound, checksum-before-semantics, seq monotonic, credit bound, dedup bounded; fail-closed on violation.

---\n### 8. Dependencies

`ids-protocol/Cargo.toml`:
- `ids-000-aa = { path = "../ids-000-aa" }` — canonical types, sealed contracts, invariants, errors
- `blake3 = "1.5"` — frame checksum (preferred)
- `sha2 = "0.10"` — SHA-256 frame checksum (alternative)
- `thiserror = "2.0"` — `ProtocolError` enum
- `serde = { version = "1", features = ["derive"] }` — frame serialization
- `bytes = "1"` — `BytesMut` zero-copy buffers for decode
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime. No networking stack. `ids-protocol` does NOT depend on `ids-crypto` at runtime (consumes `Checksum`/`Signature` type surface from `ids-000-aa` only, preserving the no-cycle rule).

Downstream dependents (all on `ids-000-aa`, plus `ids-protocol` where they consume frames): `ids-session`, `ids-rpc`, `ids-pubsub`, `ids-replication`, `ids-sync`, `ids-carrier` consumers — all send/receive `Vec<u8>` frames via `ids-carrier::Endpoint`.

---\n### 9. Measurable Acceptance Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | Frame encode/decode round-trip | `src/frame/serialization.rs` exposes encode/decode; round-trip byte-identical; `cargo check -p ids-protocol` exits 0 |
| 2 | Length-prefix + varint framing | `src/framing/length_prefix.rs` and `varint.rs`; max_frame_size enforced before allocation |
| 3 | Per-frame blake3/sha256 checksum | `src/frame/checksum.rs`; only blake3/sha256 used (no CRC32C) |
| 4 | Checksum verified before semantics | Decoder rejects corrupt checksum before any frame processing |
| 5 | Monotonic SeqNum + wraparound | `src/sequencing/seq_space.rs` modular comparison; PAWS-style old-duplicate rejection |
| 6 | Cumulative + SACK ACK | `src/sequencing/ack.rs`, `sack.rs`; piggybacked + pure ACK frames |
| 7 | QUIC-style loss detection + PTO | `src/sequencing/loss_detection.rs`, `pto.rs`; packet threshold k=3, probe frames |
| 8 | Selective retransmit with new SeqNum | `src/sequencing/retransmit.rs`; new SeqNum on retransmit, data identical |
| 9 | Fragmentation/reassembly | `src/fragmentation/fragmenter.rs`, `reassembler.rs`; split/reassemble byte-identical, out-of-order tolerated |
| 10 | Creation-time-keyed reassembly deadline | Incomplete set discarded at deadline; never RTT |
| 11 | Delivery semantics (at-most/at-least/exactly-once) | `src/delivery/semantics.rs`; exactly-once via dedup + apply-once |
| 12 | Dedup window bounded | `src/delivery/dedup_window.rs`; LRU + TTL eviction; duplicate = drop + re-ack |
| 13 | Stream/datagram sessions | `src/session/stream_session.rs`, `datagram_session.rs`; handshake SYN/SYN-ACK, teardown FIN/GOAWAY |
| 14 | Credit-based flow control | `src/flow_control/credit.rs`, `window_update.rs`; sender bounded by advertised credit |
| 15 | Carrier-agnostic assertion | Test greps source for `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket` — zero matches |
| 16 | Allowed-crypto-only assertion | Source references only ed25519/xchacha20poly1305/sha256/blake3/chacha20poly1305 type surface; no other crypto crates |
| 17 | Crate compiles cleanly | `cargo check -p ids-protocol` exits 0 with no new warnings |
| 18 | Conformance tests pass | `cargo test -p ids-protocol` passes all frame/framing/sequencing/ack/loss/fragmentation/delivery/session/flow-control tests |

---\n### 10. Remaining Uncertainties

| Uncertainty | Impact | Resolution path |
|---|---|---|
| Length prefix: fixed u32 vs varint LEB128 | Frame size overhead, simplicity | u32 LE default; varint as feature-gated option; bench both |
| Checksum: blake3 vs sha256 default | Record size, speed | blake3 default (speed), sha256 option; configurable per session |
| Whether signature is per-frame or per-session | Overhead vs handshake cost | Per-frame optional signature; session-level signature as future optimization |
| Dedup window size | Memory vs outage survival | Sized from longest intended outage; expiry = duplicate-admission event |
| Whether flow control is per-session or per-connection | Granularity of backpressure | Both levels supported; per-session default |
| Whether sessions need explicit handshake | Session setup latency | SYN/SYN-ACK handshake (yamux-style); datagram sessions may skip |
| PTO initial value without RTT | Probe timing | Conservative fixed initial; adaptive after first RTT sample (not used for validity) |
| Max frame size default | Memory vs large payloads | 1 MiB default; configurable; enforced before allocation |
| Whether crypto block (AEAD) is per-frame | Encryption overhead | Optional per-frame AEAD block; session-level encryption as future optimization |

---\n### 11. Engineering Decisions Summary

| Decision | Rationale |
|---|---|
| `ids-protocol` as separate crate (not folders in existing crate) | Matches user directive: separate crates, not more folders |
| Self-defined native frame format (header + body, length-prefixed) | HX-241-style; carrier-agnostic; no reliance on existing wire formats |
| Length-prefix framing with max_frame_size bound | Solves stream framing; bounds memory against framing bombs |
| blake3/sha256 checksum per frame (no CRC32C) | Checksum restricted to allowed crypto set |
| Monotonic `SeqNum` per session + modular wraparound | Total order per session; survives u64 wrap via modular comparison |
| Cumulative + SACK ACKs | Tolerates reordering/loss without RTT-bound timers |
| QUIC-style loss detection (packet/time thresholds, PTO) | Proven loss/retransmission over unreliable carriers |
| New `SeqNum` on retransmit | Removes retransmission ambiguity (QUIC pattern) |
| Fragmentation keyed to creation time, not RTT | Correct over arbitrary delays; avoids TCP-style timeout misfire (RFC 3449) |
| Credit-based flow control | Bounded receiver buffer; backpressure without RTT dependence |
| Stream + datagram session types | Ordered byte stream and message-boundary sessions coexist |
| Delivery semantics per frame (at-most/at-least/exactly-once) | Flexible reliability; exactly-once via idempotent dedup |
| Dedup window expiry = duplicate-admission event | Avoids silent cleanup and duplicate-admission hazards |
| No `ids-crypto` runtime dependency | Preserves no-cycle rule (`ids-crypto` does not depend on `ids-000-aa`); type surface only |
| `saturating`/modular arithmetic on all seq/tick arithmetic | No overflow panics in distributed sequencing |
| Cargo workspace member at `C:/idfs/ids-protocol/` | Matches existing workspace layout |

---\n### 12. Sources

1. tokio-util `LengthDelimitedCodec` — length-prefixed framing, default max payload 8 MiB. https://docs.rs/tokio-util/latest/tokio_util/codec/length_delimited
2. krot-proto postcard framing — varint LEB128 length prefix + postcard payload, frame too large error. https://docs.rs/krot-proto/latest/src/krot_proto/framing.rs.html
3. wire_codec `LengthPrefixed` / `VarintPrefixed` / `Checksummed` — fixed-width and varint length-prefixed framing, checksum decorator. https://docs.rs/wire_codec/latest/wire_codec/framing
4. SCALE codec (Parity/Substrate) — Simple Concatenated Aggregate Little-Endian, compact encoding. https://github.com/paritytech/parity-scale-codec
5. Protocol Buffers encoding — base-128 varints, length-delimited messages. https://protobuf.dev/programming-guides/encoding/
6. SSH Binary Packet Protocol (RFC 4253 §6) — packet_length/padding_length/payload/padding/MAC framing. https://www.rfc-editor.org/info/rfc4253/
7. AMQP 1.0 Transport (OASIS) — 8-byte fixed frame header (SIZE u32, DOFF, TYPE, channel). https://docs.oasis-open.org/amqp/core/v1.0/csprd01/amqp-core-transport-v1.0-csprd01.html
8. MQTT 5.0 — fixed header (packet type/flags + variable-byte-integer remaining length). https://docs.oasis-open.org/mqtt/mqtt/v5.0/cos02/mqtt-v5.0-cos02.pdf
9. gRPC wire protocol — length-prefixed message (compressed flag + 4-byte big-endian length + message) over HTTP/2 DATA frames. https://grpc.io/docs/guides/wire/
10. Yamux spec — 12-byte frame header (version/type/flags/streamID/length), credit-based window updates, SYN/ACK/FIN/RST flags. https://github.com/hashicorp/yamux/blob/master/spec.md
11. QUIC Loss Detection and Congestion Control (RFC 9002 §6) — per-packet-number-space loss detection, kPacketThreshold=3, PTO probe packets. https://rfcinfo.com/rfc-9002/6
12. QUIC: A UDP-Based Multiplexed and Secure Transport (RFC 9000) — streams, credit-based flow control, retransmission with new packet numbers. https://ietf.org/ietf-ftp/rfc/rfc9000.pdf
13. TCP Extensions for High Performance (RFC 7323 / RFC 1323) — window scale, timestamps, PAWS (Protect Against Wrapped Sequences). https://www.rfc-editor.org/rfc/rfc7323.html
14. Selective Repeat ARQ — selective retransmit, receiver buffer for out-of-order frames, window ≤ half sequence space. https://erg.abdn.ac.uk/useRS/gorry/couRSe/arq-pages/sr.html
15. QUIC Acknowledgment Frequency — ACK scheduling, ack-eliciting threshold, max ack delay, reordering threshold. https://datatracker.ietf.org/doc/html/draft-ietf-quic-ack-frequency
16. Kafka delivery semantics — at-most-once, at-least-once, exactly-once; idempotent producer. https://docs.confluent.io/kafka/design/delivery-semantics.md
17. RIFL — Reusable Infrastructure for Linearizability, at-least-once → exactly-once via idempotent ops. https://stanford.edu/~ouster/cgi-bin/papers/rifl.pdf
18. Machine-Checked Dual-Write Recovery — dedup window shelf life, expiry as duplicate-admission event. https://arxiv.org/html/2608.00501v4
19. IP Fragmentation and Reassembly — ID/flags/offset, destination-only reassembly, multiples-of-8 offsets, reassembly timer. https://web.eecs.umich.edu/~sugih/courses/eecs489/lectures/07-Fragmentation+IPv6+NAT.pdf
20. OSI Session Layer — dialog control, synchronization points/checkpoints, half/full-duplex. https://en.wikipedia.org/wiki/Session_layer
21. XChaCha20-Poly1305 construction (libsodium) — 192-bit nonce AEAD, random nonces safe. https://libsodium.gitbook.io/doc/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction
22. RFC 8032 — EdDSA/ed25519, 64-byte signatures, 32-byte keys. https://datatracker.ietf.org/doc/rfc8032/
23. BLAKE3 — fast content-addressed hashing, streaming verification. https://github.com/BLAKE3-team/BLAKE3
24. NIST FIPS 180-4 — Secure Hash Standard (SHA-256). https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf
25. Rust `nom` — zero-copy binary parser combinators. https://docs.rs/crate/nom/7.1.1
26. `bytes::BytesMut` — zero-copy reference-counted byte buffers for network I/O. https://docs.rs/bytes/latest/bytes/
27. Cap'n Proto / FlatBuffers / SBE zero-copy comparison — zero-copy serialization trade-offs. https://capnproto.org/news/2014-06-17-capnproto-flatbuffers-sbe.html
28. UDT — reliable datagram protocol over UDP, application-layer reliability. https://onlinelibrary.wiley.com/doi/10.1002/0470028696.ch9
29. TLA+/IC3 distributed protocol verification — inductive invariants, model checking distributed protocols. https://arxiv.org/html/2605.24619
30. IDS prompts.txt entry 12 (verbatim) + CRATES.md §12. C:/idfs/prompts.txt, C:/idfs/CRATES.md — ids-protocol crate definition, contents, constraints.
31. HX-241 reference plan format. C:/hx-241/PLANS/plan-077.md, plan-080.md, plan-121.md — Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration, File Structure, Algorithms, Dependencies, Acceptance Criteria, Uncertainties, Engineering Decisions, Sources.
32. Cargo workspaces. https://doc.rust-lang.org/cargo/reference/workspaces.html — workspace members, path dependencies.

---\n**End of plan-012.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**