# IDS Phase 017 — ids-replication: Research Findings Tracker

**Topic:** Create the `ids-replication` crate: Change propagation, ordered/versioned streams, resumable delivery.
**Source prompt:** C:/idfs/prompts.txt entry 17 (lines 250-264): "Phase 017 — Create the ids-replication crate: Change propagation, ordered/versioned streams, resumable delivery."
**Output document:** C:/idfs/plans/plan-017.md
**Methodology:** 30 targeted web searches executed in one parallel batch (Q001-Q030), 10 results per query. Each result classified ESTABLISHED / HYPOTHESIS / SPECULATION and checked against constraints:
- Carrier-agnostic: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`).
- Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- `ids-000-aa` is the master crate with all canonical types and sealed trait contracts.
- Research only — no code, no source modifications, only the plan document.

## Evidence classification definitions

- **ESTABLISHED** — peer-reviewed / textbook consensus, reproducible, widely implemented.
- **HYPOTHESIS** — proposed in literature or implemented in production systems but not conclusively generalized.
- **SPECULATION** — conjectural, no strong experimental or production basis.

## Batch index

| Batch | Theme | Query range | Status |
|-------|-------|-------------|--------|
| 1 | Change propagation & versioned streams (CDC, LSN, ordered delivery, causal ordering) | Q001-Q006 | complete |
| 2 | Replication modes, acknowledgement tracking, lag observability | Q007-Q012 | complete |
| 3 | Checkpoints, incremental sync, resumable delivery, retries/backoff | Q013-Q018 | complete |
| 4 | Carrier-agnostic opaque endpoint & allowed crypto primitives | Q019-Q024 | complete |
| 5 | Backpressure, fan-out rate model, fragmentation, log shipping | Q025-Q030 | complete |

## Batch Results — Summary

| Query | Classification | Key finding |
|-------|----------------|-------------|
| Q001 CDC ordered change propagation | ESTABLISHED | CDC captures committed row changes (insert/update/delete with before/after images, txn/commit IDs) and streams them to sinks; ordering guarantees via LSN and transaction-visibility watermarks (arxiv DBLog 2609.08160/2605.31475; YugabyteDB CDC docs; TiCDC multi-writer CDC; Wikipedia CDC). |
| Q002 Versioned stream / LSN monotonic ordering | ESTABLISHED | Write-ahead logs assign a monotonically increasing log sequence number (LSN) to each record; LSN uniquely identifies records and enables ordered replay; used in VLDB logging papers, BtrLog, Apache DistributedLog, YugabyteDB replication slots, DDIA sequence-number ordering. |
| Q003 Resumable delivery checkpoint resume | ESTABLISHED | Resumable syncs periodically save progress to a checkpoint and resume from it on failure; crash between commit and checkpoint requires dedup/idempotency (Airbyte resumability docs; Machine-Checked Dual-Write Recovery arxiv 2608.00501; JuiceFS resumable sync; PowerSync protocol; scroll cursor protocol). |
| Q004 Log-based commit/ship/apply replication | ESTABLISHED | Log-based replication mirrors writes by shipping the commit log to secondaries (synchronous 2PC or asynchronous); categories: hot standby, logical mirroring; commit log is the source of truth (Cornell CS530 replication lecture; Oracle Streams IEEE; replicated-log pattern). |
| Q005 Ordered stream delivery / out-of-order reassembly | ESTABLISHED | Sequenced delivery within streams via sequence numbers and reassembly buffers; out-of-order messages held in a bounded buffer and gaps trigger repair (RFC 2960 SCTP sequenced delivery; Wireshark TCP/TLS reassembly; EIP resequencer pattern; bounded-window refusal of gap floods). |
| Q006 Version vector causal ordering | ESTABLISHED | Version vectors track per-replica update causality; participants determine whether one update precedes another or they are concurrent; state-based convergence relies on causality info (CMU 17-350 CRDT lectures; arxiv 1511.05010 eventually consistent register; Wikipedia version vector; COPS causal+ SOSP'11). |
| Q007 Independent vs full replication | ESTABLISHED | Full replication copies all data to all nodes (max availability/read scale, write cost on every node); independent/partial replication copies subsets; primary-replica async is the standard method (ScyllaDB, Redis, MariaDB, CMU 15-445 distributed OLTP notes). |
| Q008 Acknowledgement tracking / persistent ACK cursor | ESTABLISHED | Streams assign sequence numbers and track per-consumer delivered/acked state; Pulsar persists individual + cumulative ack marks; Oxia `FollowerCursor` exposes LastPushed/AckOffset; ack watermarks turn out-of-order acks into a persistable cursor (Apache Pulsar PIP-81/PIP-299, NATS JetStream, Oxia). |
| Q009 Replication lag observability | ESTABLISHED | Replication lag = distance between last WAL record on primary and last applied on replica; monitored via CloudWatch/Oracle views, Confluent Replicator metrics, CockroachDB logical replication jobs, MongoDB rs.printSecondaryReplicationInfo, Kafka consumer lag (Google Cloud SQL, AWS RDS, Confluent, CockroachDB, MongoDB docs). |
| Q010 Per-replica ack cursor / resumable stream delivery | ESTABLISHED | Durable named subscriptions remember a cursor and resume from it; consumers track delivered/acked state per message and redeliver after timeout; per-replica progress cursors are atomic per slot (NATS JetStream durable consumers, Kafka replication, melin ReplicaCursors). |
| Q011 At-least-once vs exactly-once replication | ESTABLISHED | Delivery semantics: at-most-once (may lose), at-least-once (may redeliver), exactly-once (processed once); Kafka implements exactly-once via idempotent producer + transactions (per-producer ID, batch sequence numbers, broker dedup); Flink checkpoint choice trades latency for guarantees (Confluent, Kafka docs, Strimzi, Databricks). |
| Q012 Idempotent consumer / dedup window | ESTABLISHED | Idempotent consumer pattern: same message processed multiple times = same effect; implemented via idempotency keys + dedup window; expiry of the window is a duplicate-admission event, not silent cleanup (Azure Idempotent Consumer pattern; Machine-Checked Dual-Write Recovery arxiv 2608.00501; Kafka idempotent producer). |
| Q013 Synchronization checkpoint / incremental sync / missing update detection | ESTABLISHED | Incremental sync tracks changes since last checkpoint (timestamp- or version-based); deletion detection needs separate strategy; state sync discovers new checkpoints via consensus production; DBLog-based incremental snapshots handle orphan rows (SurrealDB sync, Sui StateSync, DBLog arxiv 2611.10810, Nango checkpoint docs). |
| Q014 Bounded backoff / exponential backoff + jitter | ESTABLISHED | Robust retry policy: error classification, bounded attempts, exponential backoff (`delay = min(cap, base * 2^attempt)`), jitter to decorrelate clients and prevent thundering herds/retry storms (AWS Architecture Blog exponential backoff + jitter; multiple distributed-systems references). |
| Q015 Snapshot + checkpoint truncation / resume from checkpoint | ESTABLISHED | Checkpoint = consistent snapshot so recovery replays only post-checkpoint WAL; recovery = load snapshot + replay WAL + truncate torn tail at first invalid frame; truncate sealed segments below checkpoint and advance start offset (Databricks Structured Streaming checkpoints; happy.engineering recovery; SQL Server transaction log; TigerBeetle VSR checkpoints). |
| Q016 State machine replication log / resumable delivery after restart | ESTABLISHED | SMR: replicas apply the same commands in identical order via replicated log; leader orders and replicates; recovery restarts sharded subsystems into equivalent fault-tolerant state; partition handled then resume on repair (Cornell DSN paper; MIT 6.824 Raft notes; Restate architecture). |
| Q017 Raft log replication resume after partition catch-up | ESTABLISHED | Raft leader manages replicated log; followers catch up via log matching + snapshots (InstallSnapshot); partitions trigger leader elections; catch-up resumes when majority reachable (Raft paper raft.pdf; CockroachDB replication layer; etcd-io/raft; implementing Raft part 3). |
| Q018 Kafka consumer offset checkpoint / resume incremental | ESTABLISHED | Consumer tracks max consumed offset per partition and commits offsets to resume after restart; offset reset policies when no committed offset; MirrorMaker 2.0 checkpoint connector copies offsets between clusters (Confluent Kafka consumer design; Redpanda consumer offsets; Confluent offsets guide). |
| Q019 Opaque endpoint / carrier-agnostic addressing | HYPOTHESIS | Transport-layer-agnostic protocol stacks and opaque endpoint references exist (patents, one transport-agnosticism article), but no mainstream carrier-agnostic opaque-endpoint standard matches the IDS `Endpoint` (`CarrierType` + `Vec<u8>`); IDS defines its own via `ids-carrier`. |
| Q020 Ed25519 signature / replication authentication | ESTABLISHED | Ed25519 (EdDSA over Curve25519) provides deterministic, fast-verification signatures; used in BFT broadcast/authenticated channels; standardized RFC 8032, RFC 9591 FROST(Ed25519); NIST and W3C EdDSA cryptosuites recognize it. |
| Q021 XChaCha20Poly1305 nonce misuse resistance | ESTABLISHED | XChaCha20-Poly1305 extends ChaCha20-Poly1305 with a 192-bit nonce, allowing safe random nonces with negligible collision probability (libsodium construction docs; IETF draft-irtf-cfrg-xchacha-03; not nonce-misuse-resistant by itself, but large nonce bounds misuse risk). |
| Q022 BLAKE3 content-addressed chunking / dedup | ESTABLISHED | BLAKE3 is a fast keyed/unkeyed hash used for content-addressed storage and content-defined chunking (FastCDC) with exact deduplication; Borg backup store hash = BLAKE3-256; Xet chunk-level dedup uses BLAKE3 content addressing. |
| Q023 SHA256 integrity hash chain / checkpoint | ESTABLISHED | SHA-256 hash chains make append-only logs tamper-evident (each entry hashes the previous); signed checkpoints (Ed25519) anchor chains; basis for audit-ledger integrity (NIST FIPS 180-4 SHA-256; hash-chained compliance logs). |
| Q024 ChaCha20-Poly1305 vs XChaCha20-Poly1305 nonce generation | ESTABLISHED | ChaCha20-Poly1305 (RFC 7539/8439) uses a 96-bit nonce (counter-based safe); XChaCha20-Poly1305 uses HChaCha20-derived subkey + 192-bit nonce for random nonces; both are AEADs in the allowed set (libsodium, IETF drafts, OpenSSL tracking issue). |
| Q025 Backpressure / flow control on ordered stream | ESTABLISHED | Backpressure = downstream capacity propagated to producers, changing enqueue/production behavior; bounded buffers with thresholds prevent unbounded memory; slow consumer signals producer to wait (Akka Streams boundedness, system-design-patterns backpressure, Flink backpressure). |
| Q026 Bounded backoff retry / jitter for sync replicas | ESTABLISHED | Same family as Q014: exponential backoff with jitter and a ceiling bound; applied at every retry boundary; synchronized retries convert hiccups into outages (AWS Architecture Blog; multiple retry-strategy references). |
| Q027 Change propagation fan-out rate model | HYPOTHESIS | Classic eager vs lazy update propagation and Byzantine update diffusion exist (Stoy et al. 1999; Malkhi/Reiter 2002); concrete fan-out rate models for IDS-scale heterogeneous carriers are not established — IDS-specific model is a hypothesis to validate. |
| Q028 Durable SMR log compaction / checkpoint truncation | ESTABLISHED | Raft snapshotting removes log entries below the snapshot index; compaction tied to snapshotting; Flink incremental checkpoints log state changes plus tables; MultiPaxos underlies Spanner/Chubby SMR (RabbitMQ Ra COMPACTION, hashicorp/raft, Flink FLIP-158). |
| Q029 Stream fragmentation / reassembly bounded memory | ESTABLISHED (problem) | Unbounded out-of-order stream reassembly causes memory-exhaustion DoS (quinn-proto RUSTSEC-2026-0185/0341, Apache bRPC CVE); per-stream reassembly caps are the documented fix; IDS must bound reassembly memory — specific scheme is a design decision. |
| Q030 Log shipping point-to-point catch-up / incremental sync | ESTABLISHED | Log shipping transfers WAL records segment-by-segment from primary to standby; streaming replication continuously ships and applies WAL records; used for incremental catch-up after restart/partition (PostgreSQL warm standby/log shipping; Garnet replication; SQL Server log shipping). |

**Constraint check:** All 30 queries returned usable results; zero conflicts with carrier-agnostic or allowed-crypto constraints. Q019 (opaque endpoint) is the only HYPOTHESIS-level item (no off-the-shelf standard — IDS defines its own). Q027/Q029 carry HYPOTHESIS-level design elements. No SPECULATION findings.

**Details:** results-017.md (one entry per query ID with full source URLs).
