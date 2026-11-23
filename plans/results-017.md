# IDS Phase 017 — ids-replication: Detailed Search Results

**Topic:** Create the `ids-replication` crate: Change propagation, ordered/versioned streams, resumable delivery.
**Batch:** 30 targeted web searches (Q001-Q030), 10 results per query, executed in parallel.
**Classification:** ESTABLISHED / HYPOTHESIS / SPECULATION (see FINDINGS-017.md for definitions).
**Constraint check:** Every finding checked against carrier-agnostic rule (all traffic via `ids-carrier::Endpoint` only) and allowed-crypto rule (ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 only).

---

## Q001 — change data capture CDC ordered change propagation stream distributed systems
**Classification:** ESTABLISHED
**Key finding:** CDC captures committed row changes (insert/update/delete with before/after images, transaction and commit identifiers) from a database log and streams them to downstream sinks; ordering via LSN and transaction-visibility watermarks prevents lost updates from out-of-LSN-order commits.
**Constraint check:** Satisfies — purely logical change-propagation pattern; no networking-stack assumptions.
**Sources:**
- https://arxiv.org/pdf/2611.10810 (Extending DBLog-Based Incremental Snapshots)
- https://docs.yugabyte.com/stable/architecture/docdb-replication/change-data-capture (YugabyteDB CDC architecture)
- https://pingcap.com/blog/multi-writer-change-data-capture-cdc-architecture-ticdc-tidb (TiCDC multi-writer CDC)
- https://arxiv.org/html/2609.08160v1 (Generalized DBLog verified contract)
- https://wcygan.net/change-data-capture (CDC pattern overview)

---

## Q002 — versioned stream replication log sequence number monotonic ordering
**Classification:** ESTABLISHED
**Key finding:** Write-ahead logs assign a monotonically increasing log sequence number (LSN) to each appended record; the LSN uniquely identifies a record and is the basis for ordered replay, commit indexing, and recovery resume points.
**Constraint check:** Satisfies — versioned ordering concept independent of any transport.
**Sources:**
- https://vldb.org/pvldb/vol17/p3745-xu.pdf (PALF group log sequencing)
- https://arxiv.org/pdf/2606.27051 (BtrLog low-latency WAL logging)
- https://apache.github.io/distributedlog/design/main.html (Apache DistributedLog detail design)
- https://docs.yugabyte.com/stable/additional-features/change-data-capture/using-logical-replication/key-concepts.md (YugabyteDB LSN replication slots)
- https://ebrary.net/64861/computer_science/sequence_number_ordering (DDIA sequence-number ordering)

---

## Q003 — resumable delivery protocol checkpoint resume incremental sync distributed systems
**Classification:** ESTABLISHED
**Key finding:** Resumable syncs periodically persist a progress checkpoint and resume from it after a crash; a crash between committing on the destination and saving the checkpoint requires at-least-once delivery plus idempotent dedup to avoid duplicate effects.
**Constraint check:** Satisfies — checkpoint/resume is a transport-independent delivery pattern.
**Sources:**
- https://docs.airbyte.com/platform/understanding-airbyte/resumability (Airbyte resumability)
- https://arxiv.org/html/2608.00501v5 (Machine-Checked Dual-Write Recovery from a Commit Log)
- https://juicefs.com/en/blog/engineering/resumable-sync-encryption-bandwidth-control (JuiceFS resumable sync)
- https://docs.powersync.com/architecture/powersync-protocol (PowerSync protocol)
- https://github.com/welidev/scroll (scroll: ordered, resumable change replication over line logs)

---

## Q004 — log-based change propagation commit ship apply pattern database replication
**Classification:** ESTABLISHED
**Key finding:** Log-based replication maintains secondaries by shipping the primary's commit log (hot standby / logical mirroring), synchronously (2PC) or asynchronously; the commit log is the source of truth and writes reach replicas as a log stream.
**Constraint check:** Satisfies — log stream is carrier-agnostic payload over `ids-carrier::Endpoint`.
**Sources:**
- https://cs.cornell.edu/courses/cs530/2004sp/lect/Lec15.pdf (Cornell CS530 replication approaches)
- https://ieeexplore.ieee.org/document/4812537 (Oracle Streams replication)
- https://github.com/sanzgiri/patterns/blob/main/reference/data/replicated-log.md (replicated-log pattern)
- https://github.com/djeada/Databases-Notes/blob/main/notes/09_database_replication/01_intro_to_replication.md (replication primer)

---

## Q005 — ordered stream delivery out-of-order reassembly sequence gap detection
**Classification:** ESTABLISHED
**Key finding:** Sequenced delivery within a stream uses sequence numbers; out-of-order units are held in a bounded reassembly buffer and gaps are detected so missing units can be repaired; unbounded reassembly buffers are a memory-exhaustion risk.
**Constraint check:** Satisfies — reassembly works over opaque `Vec<u8>` datagrams from `ids-carrier::Endpoint`.
**Sources:**
- https://docbox.etsi.org/Reference/IETF/RFC/RFC2960.pdf (RFC 2960 SCTP sequenced delivery)
- https://ieeexplore.ieee.org/document/4667430 (SCTP stream buffer reordering)
- https://wireshark.org/docs/wsug_html_chunked/ChAdvReassemblySection.html (packet reassembly)
- https://knowledge.pricefx.com/.../resequencer/java-dsl-resequencer (EIP resequencer pattern)

---

## Q006 — version vector causal ordering replication stream convergent state
**Classification:** ESTABLISHED
**Key finding:** Version vectors track per-replica update counts so participants can tell whether one update causally precedes another or they are concurrent; convergent eventual consistency relies on this causality information plus operation semantics.
**Constraint check:** Satisfies — version vectors are carried inside stream payloads; no network stack needed.
**Sources:**
- https://cs.cmu.edu/~aldrich/courses/17-350/slides/11-testing.pdf (CMU 17-350 CRDT testing/convergence)
- https://arxiv.org/pdf/1511.05010v1 (eventually consistent register / causality)
- https://en.wikipedia.org/wiki/Version_vector (version vector mechanism)
- https://cs.princeton.edu/~mfreed/docs/cops-sosp11.pdf (COPS causal+ consistency)

---

## Q007 — independent vs full replication strategy distributed database
**Classification:** ESTABLISHED
**Key finding:** Full replication keeps an identical copy on every node (maximum availability/read scale, write cost everywhere); independent/partial replication copies subsets of data; primary-replica asynchronous streaming is the standard baseline method.
**Constraint check:** Satisfies — both modes are logical replication topologies over opaque endpoints.
**Sources:**
- https://scylladb.com/glossary/database-replication (ScyllaDB replication overview)
- https://redis.io/blog/what-is-data-replication (Redis data replication)
- https://mariadb.com/docs/server/ha-and-performance/standard-replication/choosing-a-replication-strategy (MariaDB async primary-replica)
- https://15445.courses.cs.cmu.edu/fall2024/notes/23-distributedoltp.pdf (CMU 15-445 distributed OLTP)

---

## Q008 — acknowledgement tracking persistent ACK cursor replication stream
**Classification:** ESTABLISHED
**Key finding:** Streams assign sequence numbers and track per-consumer delivered/acked state; Pulsar persists individual + cumulative ack positions; Oxia exposes per-follower cursors (LastPushed/AckOffset); out-of-order acks are folded into a persistable ack watermark cursor.
**Constraint check:** Satisfies — cursors are logical positions carried in payloads over endpoints.
**Sources:**
- https://github.com/apache/pulsar/wiki/PIP-81%3A-Split-the-individual-acknowledgments-into-multiple-entries (Pulsar PIP-81)
- https://docs.nats.io/concepts/jetstream.md (NATS JetStream stream/consumer ack state)
- https://pkg.go.dev/github.com/oxia-db/oxia/oxiad/dataserver/controller/lead (Oxia FollowerCursor)
- https://pkg.go.dev/github.com/gsoultan/hermod/pkg/infra/ackwatermark (ackwatermark cursor)

---

## Q009 — replication lag observability metrics monitoring distributed systems
**Classification:** ESTABLISHED
**Key finding:** Replication lag is the distance between the last WAL record generated on the primary and the last record applied on a replica; it is monitored via cloud metrics (CloudWatch, RDS views), database consoles (CockroachDB, MongoDB), and Kafka-style consumer lag metrics.
**Constraint check:** Satisfies — lag is a logical position difference observable without any network stack.
**Sources:**
- https://docs.cloud.google.com/sql/docs/postgres/replication/replication-lag (Google Cloud SQL replication lag)
- https://aws.amazon.com/blogs/database/diagnose-and-resolve-replica-lag-in-amazon-rds-for-oracle-replicas-part-2 (AWS RDS Oracle replica lag)
- https://docs.confluent.io/platform/7.3/multi-dc-deployments/replicator/replicator-monitoring.html (Confluent Replicator monitoring)
- https://docs.cockroachlabs.com/docs/v25.2/logical-data-replication-monitoring (CockroachDB logical replication monitoring)
- https://mongodb.com/docs/v8.2/troubleshooting/replication-lag (MongoDB replication lag)

---

## Q010 — per-replica ack cursor resumable stream delivery replication
**Classification:** ESTABLISHED
**Key finding:** Durable named subscriptions remember a cursor and advance it as messages are acked, resuming from the cursor after failure; consumers track per-message delivered/acked state and redeliver after a timeout; per-replica progress cursors are atomic per slot.
**Constraint check:** Satisfies — cursor-based resume is transport-independent.
**Sources:**
- https://docs.nats.io/learn/jetstream/surviving-node-loss.md (NATS JetStream surviving node loss)
- https://fibril.sh/concepts/plexus-streams (Plexus streams durable cursor)
- https://martinuke0.github.io/posts/2026-09-05-inside-nats-jetstream-building-exactly-once- (NATS JetStream exactly-once delivery)
- https://docs.confluent.io/kafka/design/replication.html (Kafka replication and committed messages)

---

## Q011 — at-least-once exactly-once replication idempotent application
**Classification:** ESTABLISHED
**Key finding:** Three delivery semantics exist: at-most-once (may lose), at-least-once (may redeliver), exactly-once (processed exactly once); Kafka implements exactly-once via an idempotent producer (per-producer ID + batch sequence numbers, broker dedup) plus transactions.
**Constraint check:** Satisfies — idempotent application is implemented at the replica over opaque endpoints.
**Sources:**
- https://confluent.io/blog/exactly-once-semantics-are-possible-heres-how-apache-kafka-does-it (Kafka exactly-once)
- https://docs.confluent.io/kafka/design/delivery-semantics.html (Kafka delivery guarantees)
- https://strimzi.io/blog/2023/05/03/kafka-transactions (Kafka transactions exactly-once)
- https://kafka.apache.org/41/design/design (Kafka design: at-least-once/exactly-once)

---

## Q012 — idempotent message processing exactly-once semantics dedup window
**Classification:** ESTABLISHED
**Key finding:** The idempotent consumer pattern makes processing the same message multiple times equivalent to processing it once; implemented with idempotency keys inside a dedup window; window expiry is a duplicate-admission event, not silent cleanup.
**Constraint check:** Satisfies — idempotency keys are opaque payload fields over endpoints.
**Sources:**
- https://learn.microsoft.com/en-us/azure/architecture/patterns/idempotent-consumer (Azure Idempotent Consumer pattern)
- https://arxiv.org/html/2608.00501v5 (Machine-Checked Dual-Write Recovery: dedup window shelf life)
- https://pkg.go.dev/github.com/1mb-dev/ledgerq@v1.4.2/examples/deduplication (deduplication command)
- https://queenmq.com/concepts/dedup (Queen MQ dedup window)

---

## Q013 — synchronization checkpoint incremental sync missing update detection
**Classification:** ESTABLISHED
**Key finding:** Incremental sync captures changes since the last checkpoint (timestamp- or version-based) and needs a separate deletion-detection strategy; state-sync discovers new checkpoints via consensus production; DBLog-based incremental snapshots handle orphan rows during bootstrap/repair.
**Constraint check:** Satisfies — missing-update detection is version comparison over change streams.
**Sources:**
- https://github.com/surrealdb/surreal-sync/blob/main/docs/neo4j.md (SurrealDB incremental sync)
- https://github.com/MystenLabs/sui/blob/main/crates/sui-network/src/state_sync/mod.rs (Sui StateSync checkpoints)
- https://arxiv.org/pdf/2611.10810 (DBLog incremental snapshots orphan rows)
- https://github.com/NangoHQ/nango/commit/cc80fa906213e6af4b94e8df0a22729f73d3bea1 (Nango checkpoint docs)

---

## Q014 — bounded backoff exponential backoff jitter synchronization retry
**Classification:** ESTABLISHED
**Key finding:** A robust retry policy has five components: error classification, bounded attempts, exponential backoff (`delay = min(cap, base * 2^attempt)`), jitter to decorrelate clients, and a budget; jitter prevents synchronized retries from turning hiccups into outages (thundering herd / retry storms).
**Constraint check:** Satisfies — backoff is local retry timing; no transport assumptions.
**Sources:**
- https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter (AWS Architecture Blog)
- https://systemoverflow.com/learn/distributed-primitives/idempotency/retry-policies-exponen (retry policies: backoff, jitter, budgets)
- https://geeksforgeeks.org/system-design/retries-strategies-in-distributed-systems (retry strategies with jitter)

---

## Q015 — snapshot checkpoint truncation resume from checkpoint write ahead log
**Classification:** ESTABLISHED
**Key finding:** A checkpoint is a consistent on-disk snapshot so crash recovery replays only the WAL after the checkpoint; recovery loads the latest snapshot, replays the WAL, truncates the torn tail at the first invalid frame, and reclaims sealed segments below the checkpoint.
**Constraint check:** Satisfies — snapshot/checkpoint/truncation are storage-logic patterns over opaque payloads.
**Sources:**
- https://docs.databricks.com/aws/en/structured-streaming/checkpoints (Structured Streaming checkpoints)
- https://happy.engineering/topics/how-it-works/recovery (crash recovery: snapshot + WAL replay + torn-tail truncation)
- https://learn.microsoft.com/en-us/sql/relational-databases/sql-server-transaction-log-architecture-and-management-guide (SQL Server transaction log active tail)
- https://raw.githubusercontent.com/tigerbeetle/tigerbeetle/0.16.54/docs/internals/vsr.md (TigerBeetle VSR checkpoints)

---

## Q016 — state machine replication log resumable delivery after restart partition
**Classification:** ESTABLISHED
**Key finding:** State machine replication keeps replicas consistent by applying the same commands in the same order via a replicated log led by a sequencer/leader; recovery restarts sharded subsystems into an equivalent fault-tolerant state and resumes after partition repair.
**Constraint check:** Satisfies — SMR log delivery is carried over opaque endpoints.
**Sources:**
- https://cs.cornell.edu/~weijia/papers/edward-dsn.pdf (recovery for replicated subsystems)
- https://docs.restate.dev/references/architecture (Restate log as durability layer)
- http://nil.csail.mit.edu/6.824/2017/notes/l-raft.txt (MIT 6.824 Raft notes)

---

## Q017 — Raft log replication stream resume after network partition catch-up
**Classification:** ESTABLISHED
**Key finding:** Raft gives the leader complete responsibility for the replicated log; followers catch up via log matching plus InstallSnapshot; partitions trigger elections and catch-up resumes once a majority is reachable again.
**Constraint check:** Satisfies — Raft messages are opaque payloads over `ids-carrier::Endpoint`.
**Sources:**
- https://raft.github.io/raft.pdf (In Search of an Understandable Consensus Algorithm)
- https://docs.cockroachlabs.com/docs/stable/architecture/replication-layer (CockroachDB replication layer)
- https://github.com/etcd-io/raft/blob/v3.7.0/doc.go (etcd raft)
- https://mohllal.github.io/implementing-raft-part-3 (Implementing Raft: log replication)

---

## Q018 — Kafka consumer offset checkpoint resume consumer group incremental
**Classification:** ESTABLISHED
**Key finding:** A Kafka consumer tracks the maximum offset consumed per partition and commits offsets so it can resume from them after restart; offset-reset policies apply when no committed offset exists; MirrorMaker 2.0 checkpoints copy offsets between clusters.
**Classification note:** ESTABLISHED
**Constraint check:** Satisfies — offsets are logical positions over opaque endpoints.
**Sources:**
- https://docs.confluent.io/kafka/design/consumer-design.html (Kafka consumer design)
- https://docs.redpanda.com/streaming/25.2/develop/consume-data/consumer-offsets.md (Redpanda consumer offsets)
- https://docs.cloud.google.com/managed-service-for-apache-kafka/docs/connect-cluster/create-mirrormaker-checkpoint-connector (MirrorMaker 2.0 checkpoint connector)

---

## Q019 — opaque endpoint carrier-agnostic transport addressing prior art
**Classification:** HYPOTHESIS
**Key finding:** Transport-layer-agnostic protocol stacks and opaque endpoint references exist in patents and one transport-agnosticism article, but no mainstream standard matches the IDS `Endpoint` (`CarrierType` + `Vec<u8>`); IDS defines its own carrier abstraction via `ids-carrier`.
**Constraint check:** Directly relevant to the carrier-agnostic mandate — IDS must define its own Endpoint; no off-the-shelf standard to adopt.
**Sources:**
- https://qu3ry.net/articles/memory-native-protocol/transport-agnosticism (transport-layer agnosticism)
- https://arxiv.org/abs/2209.04729 (transport-agnostic scheme design space)
- https://patentsgazette.uspto.gov/week45/OG/html/1552-2/US12794945-20261110.html (opaque-addressing patent)

---

## Q020 — Ed25519 signature replication message authentication integrity distributed
**Classification:** ESTABLISHED
**Key finding:** Ed25519 (EdDSA over Curve25519) provides deterministic signatures with the fastest verification; used to make blocks/votes transferable evidence in BFT broadcast; standardized in RFC 8032 and RFC 9591 (FROST-Ed25519).
**Constraint check:** Satisfies — ed25519 is in the allowed crypto set.
**Sources:**
- https://www.rfc-editor.org/rfc/rfc8032 (RFC 8032 EdDSA/ed25519)
- https://datatracker.ietf.org/meeting/99/materials/slides-99-cfrg-collective-edwards-curve-digital-signature-algorithm-draft-f... (CFRG collective EdDSA)
- https://pkg.go.dev/github.com/seabond/go-mpc/frost (FROST threshold Schnorr for Ed25519, RFC 9591)
- https://w3.org/TR/2023/CR-vc-di-eddsa-20231121 (W3C Data Integrity EdDSA cryptosuite)

---

## Q021 — XChaCha20Poly1305 stream encryption random nonce misuse resistance
**Classification:** ESTABLISHED
**Key finding:** XChaCha20-Poly1305 extends ChaCha20-Poly1305 with a 192-bit nonce via HChaCha20-derived subkeys, so random nonces are safe with negligible collision probability; it is not itself nonce-misuse-resistant, but the large nonce bounds misuse risk.
**Constraint check:** Satisfies — xchacha20poly1305 is in the allowed crypto set.
**Sources:**
- https://doc.libsodium.org/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction (libsodium XChaCha20-Poly1305)
- https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03 (IETF XChaCha draft)
- https://crypto.stackexchange.com/questions/53153/is-xchacha20-poly1305-nonce-misuse-resist (nonce-misuse-resistance discussion)

---

## Q022 — BLAKE3 content-addressed chunking deduplication replication streams
**Classification:** ESTABLISHED
**Key finding:** BLAKE3 is a fast cryptographic hash used for content-addressed storage and content-defined chunking (FastCDC) with exact deduplication; Borg backup names objects by BLAKE3-256 store hashes; Xet chunk-level dedup uses BLAKE3 content addressing.
**Constraint check:** Satisfies — blake3 is in the allowed crypto set.
**Sources:**
- https://snix.dev/docs/components/castore/blobstore-chunking-verified-streaming (BLAKE3 content-addressed blobstore)
- https://raw.githubusercontent.com/borgbackup/borg/master/docs/internals/data-structures.rst (Borg BLAKE3 store hash)
- https://huggingface.co/docs/xet/en/deduplication (Xet chunk-level dedup)
- https://github.com/BLAKE3-team/BLAKE3 (BLAKE3 specification)

---

## Q023 — SHA256 integrity hash chain replication checkpoint verification
**Classification:** ESTABLISHED
**Key finding:** SHA-256 hash chains (each entry hashes the previous entry's hash) make append-only logs tamper-evident; signed checkpoints (Ed25519) anchor the chain; used in audit-ledger integrity designs.
**Constraint check:** Satisfies — sha256 is in the allowed crypto set; hash chains are logical payload structures.
**Sources:**
- https://gist.github.com/edvinmolla/675d5d5a69b86cbd38d918c2ee0d0c16 (integrity ledger SHA-256 hash chain)
- https://github.com/rugbedbugg/Lattice-d (SHA-256 chained audit log, Ed25519 checkpoints)
- https://nvlpubs.nist.gov/nistpubs/fips/nist.fips.180-4.pdf (NIST FIPS 180-4 SHA-256)

---

## Q024 — ChaCha20-Poly1305 versus XChaCha20-Poly1305 nonce generation replication
**Classification:** ESTABLISHED
**Key finding:** ChaCha20-Poly1305 (RFC 7539/8439) uses a 96-bit nonce, safely managed as a counter; XChaCha20-Poly1305 derives a subkey via HChaCha20 and accepts a 192-bit nonce, allowing random nonces; both are AEADs in the allowed set.
**Constraint check:** Satisfies — both primitives are in the allowed crypto set.
**Sources:**
- https://ietf.org/archive/id/draft-irtf-cfrg-xchacha-02.xml (IETF XChaCha draft)
- https://doc.libsodium.org/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction (libsodium construction)
- https://datatracker.ietf.org/doc/id/draft-irtf-cfrg-xchacha-03.txt (XChaCha draft -03)

---

## Q025 — backpressure flow control ordered replication stream bounded memory
**Classification:** ESTABLISHED
**Key finding:** Backpressure propagates downstream capacity back to producers and must change enqueue/production behavior; bounded buffers with thresholds signal the producer to slow down before overflow; without it, a fast producer causes unbounded memory growth.
**Constraint check:** Satisfies — backpressure signals are carried in payloads over `ids-carrier::Endpoint`; no network stack.
**Sources:**
- https://doc.akka.io/libraries/akka-core/current/stream/stream-flows-and-basics.html (Akka Streams boundedness)
- https://github.com/babushkai/system-design-patterns/blob/main/06-scaling/07-backpressure.md (backpressure pattern)
- https://arxiv.org/html/1909.09923v4 (Backpressure Flow Control paper)

---

## Q026 — bounded backoff retry synchronization distributed replicas jitter
**Classification:** ESTABLISHED
**Key finding:** Same family as Q014: exponential backoff with jitter and a ceiling, applied at every retry boundary; synchronized retries convert local hiccups into system-wide outages.
**Constraint check:** Satisfies — retry timing is local logic over opaque endpoints.
**Sources:**
- https://aws.amazon.com/blogs/architecture/exponential-backoff-and-jitter (AWS Architecture Blog)
- https://throttle.com/guides/exponential-backoff-and-jitter (exponential backoff + jitter)
- https://geeksforgeeks.org/system-design/retries-strategies-in-distributed-systems (retry strategies)

---

## Q027 — change propagation fan-out multi-target replication rate model
**Classification:** HYPOTHESIS
**Key finding:** Eager vs lazy update propagation and Byzantine update diffusion are established (update log per site; diffusion bounds with f < n/3 Byzantine replicas), but no generalized fan-out rate model for IDS-scale heterogeneous carrier fan-out exists — an IDS-specific model is a hypothesis to validate.
**Constraint check:** Satisfies — fan-out is per-target `ids-carrier::Endpoint` datagrams; rate model is a design parameter.
**Sources:**
- https://doi.org/10.1016/s0166-5316%2899%2900004-8 (On the propagation of updates in distributed replicated systems)
- https://doi.org/10.1016/s0304-3975%2802%2900325-0 (Diffusion without false rumors, Byzantine)
- https://cs-people.bu.edu/mathan/reading-groups/papers-classics/replicas.pdf (eager vs lazy propagation)

---

## Q028 — durable replicated state machine log compaction checkpoint truncation
**Classification:** ESTABLISHED
**Key finding:** Raft snapshotting removes log entries below the snapshot index (compaction tied to snapshotting); Flink incremental checkpoints log state changes plus state tables; MultiPaxos underpins SMR in Spanner/Chubby.
**Constraint check:** Satisfies — compaction/truncation are log-management patterns over opaque payloads.
**Sources:**
- https://pkg.go.dev/github.com/hashicorp/raft (hashicorp/raft replicated log + FSM)
- https://github.com/rabbitmq/ra/blob/main/docs/internals/COMPACTION.md (RabbitMQ Ra compaction)
- https://wiki.apache.org/confluence/spaces/FLINK/pages/173082889/FLIP-158+Generalized+incre (Flink FLIP-158 incremental checkpoints)
- https://arxiv.org/html/2405.11183 (MultiPaxos Made Complete)

---

## Q029 — replication stream fragmentation reassembly bounded memory stream
**Classification:** ESTABLISHED (problem documented)
**Key finding:** Unbounded out-of-order stream reassembly is a documented memory-exhaustion DoS vector (quinn-proto RUSTSEC-2026-0185/0341, Apache bRPC CVE-2026-80463); per-stream reassembly caps are the fix; IDS must bound reassembly memory — the specific bounding scheme is a design decision.
**Constraint check:** Satisfies — reassembly bounds are internal stream logic over opaque endpoints.
**Sources:**
- https://rustsec.org/advisories/RUSTSEC-2026-0185.html (quinn-proto reassembly memory exhaustion)
- https://github.com/quinn-rs/quinn/security/advisories/GHSA-qfwj-vfxf-92j2 (quinn reassembly guard bypass)
- https://docs.stg.fedoraproject.org/en-US/defensive-coding/tasks/Tasks-Serialization (fragment identifiers ≥64 bits recommended)

---

## Q030 — log shipping point-to-point replication catch-up incremental synchronization
**Classification:** ESTABLISHED
**Key finding:** Log shipping transfers WAL records segment-by-segment from primary to standby; streaming replication continuously ships and applies WAL records to standbys to keep them current; used for incremental catch-up after restart or partition.
**Constraint check:** Satisfies — log shipping is WAL payload over opaque endpoints.
**Sources:**
- https://postgresql.org/docs/current/warm-standby.html (PostgreSQL log-shipping standby servers)
- https://microsoft.github.io/garnet/docs/cluster/replication (Garnet log-shipping replication)
- https://wiki.postgresql.org/wiki/Streaming_Replication (PostgreSQL streaming replication)
- https://learn.microsoft.com/en-us/sql/database-engine/log-shipping/log-shipping-and-replication-sql-server (SQL Server log shipping)
