## Phase 035 — Create the ids-federation crate: Infrastructure owned by independent parties.

**Research complete (30 targeted searches executed). No code written, no files created, no repository changes made. This document is the blueprint for implementation.**

---

### Verbatim prompt (C:/idfs/prompts.txt lines 520-534)

"Phase 035 — Create the ids-federation crate: Infrastructure owned by independent parties."

Crate: `ids-federation`
Purpose: Infrastructure owned by independent parties.
Contents: Federated infrastructure registry, multi-owner trust architecture, authenticated identities, scoped credentials, policy evaluation, revocation, verifiable participation records, domain admission, capability advertisements, trust roots, provenance.

"try to use separate crates instead of more folders in existing ones"

Create plan-035.md in C:/idfs/plans/ for the ids-federation crate.
Follow the HX-241 plan format (plan-077.md, plan-080.md, plan-121.md): Executive Summary, Architecture Overview, Detailed Research Dimensions, Threat Model, Integration with Existing Crates, Proposed File Structure (~90-1000 files in subfolders), Key Algorithms & Data Structures, Dependencies, Measurable Acceptance Criteria, Remaining Uncertainties, Engineering Decisions Summary, Sources.
Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
ids-000-aa is the master crate with all canonical types and sealed trait contracts.
Research only — no code, no source modifications, only the plan document.

---

### 1. Executive Summary

Phase 035 creates `ids-federation`, the IDS crate for multi-owner trust and federated infrastructure registry. It owns federation across independently operated parties where no single administrative owner controls all infrastructure: federated infrastructure registry (discover participants, H-units, operators, declared resources, capabilities, service endpoints), multi-owner trust architecture (authenticated identities, scoped credentials, policy evaluation, revocation, verifiable participation records), domain admission (adding new participants without pre-shared central trust), capability advertisements (what each domain offers: capacity, durability profiles, geographic constraints), trust roots (per-domain trust anchors, multi-root trust), and provenance (tamper-evident participation/audit records).

Carrier-agnostic absolute rule: no `std::net`, `tokio::net`, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic is opaque `Vec<u8>` datagrams sent to `ids-carrier::Endpoint` (`CarrierType` + `Vec<u8>`, max 1024-byte address). Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 (implemented in `ids-crypto`; `ids-federation` consumes type surface and primitive IDs from `ids-000-aa`/`ids-crypto` only).

A skeleton `ids-federation` crate does NOT yet exist at `C:/idfs/ids-federation/`. `ids-000-aa` already declares canonical types `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `PartitionId`, `RecordId`, `Endpoint`, `CarrierType`, `Credential`, `AuthToken`, `KeyId`, `Signature`, `FailureDomain`, plus invariant `FederationTrust` (`src/invariants/federation_trust.rs`, referenced from `lib.rs` line 67) and sealed contracts `Security`, `Membership`, `Discovery`, `Placement`, `Transport` — all referenced by this crate but never redefined. Research only — no code, no source modifications, only this plan document.

Research basis: 30 parallel searches returned W3C Verifiable Credentials Data Model v2.1 (issuer/holder/verifier, credentialStatus, verifiable data registry), W3C Threat Model for Decentralized Credentials (revocation lists, Status List bitstrings, cryptographic accumulators), OpenID Connect Federation 1.0 (trust anchors, entity statements, dynamic trust resolution), DIDs + verifiable credentials for decentralized identity, SPKI/SDSI authorization certificates, SAINT isolation domains / trust anchor scoping, TLS Trust Anchor Identifiers, multi-CA PKI federation, key transparency (CONIKS, SEEMless, Parakeet, Aegon, Federated Key Transparency), dynamic trust negotiation (circles of trust, trust pathways), iRODS federation (zones share keys, local control, cross-zone access), HDFS Federation (multiple namenodes/namespaces), capability-based security (Macaroons offline attenuation, RFC 9396 authorization_details, RFC 9449 DPoP, OAuth 2.0 Token Exchange RFC 8693), policy engines (OPA Rego, PDP/PEP separation), provenance tracking in federated settings (permissioned blockchain, immutable provenance), verifiable proof of participation (FedPoP), trust bootstrapping without central authority (Kleene recursion theorem mutual attestation), CRDTs / eventual consistency foundations (Shapiro et al.), and IDS internal sources (CRATES.md entry 35, CAPABILITIES.md capabilities 99-101, DIFFERENCES.md federation analysis, ids-000-aa `FederationTrust` invariant, `Credential`/`AuthToken`/`KeyId`/`Signature` types).

---

### 2. Architecture Overview

`ids-federation` sits above `ids-000-aa` (types + sealed contracts + invariants + errors), `ids-carrier` (opaque `Endpoint` datagram transport), and `ids-crypto` (allowed primitives). It is layered so each concern — federation registry, multi-owner trust, authenticated identities, scoped credentials, policy evaluation, revocation, verifiable participation records, domain admission, capability advertisements, trust roots, provenance — is an independent module:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   APPLICATION / ids-* LAYER (federation consumers)          │
│  ids-placement, ids-routing, ids-replication, ids-write — consult registry │
├─────────────────────────────────────────────────────────────────────────────┤
│                   Phase 035: ids-federation (THIS CRATE)                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │\n│  │ federation   │ │ multi-owner  │ │ authenticated  │ │ scoped       │    │ │\n│  │ registry     │ │ trust        │ │ identities     │ │ credentials  │    │ │\n│  │ (DomainId)   │ │ (trust roots)│ │ (DID-style IDs)│ │ (capability) │    │ │\n│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │\n│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │\n│  │ policy       │ │ revocation   │ │ domain         │ │ capability   │    │ │\n│  │ evaluation   │ │ (status list)| │ admission      │ │ advertisements│   │ │\n│  │ (OPA-like)   │ │ (accumulator)│ │ (trust neg.)   │ │ (advertise)  │    │ │\n│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │\n│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐    │ │\n│  │ trust roots  │ │ provenance   │ │ verifiable     │ │ carrier      │    │ │\n│  │ (multi-root) │ │ (tamper-evid.)│ │ participation  │ │ adapter      │    │ │\n│  │ per-domain   │ │ records      │ │ records        │ │ (Endpoint)   │    │ │\n│  └──────────────┘ └──────────────┴────────────────┴────────────────┘    │ │\n└─────────────────────────────────────────────────────────────┬───────────────┘\n                                                              │ opaque Vec<u8> datagrams\n┌─────────────────────────────────────────────────────────────▼───────────────┐\n│              CARRIER BOUNDARY: ids-carrier::Endpoint only                  │\n│  send_datagram(Endpoint{CarrierType, Vec<u8>≤1024}, payload: &[u8])        │\n│  receive_datagram → (Endpoint, Vec<u8>)                                    │\n│  No std::net / tokio::net / libp2p / IP / TCP / UDP / QUIC / HTTP / DNS    │\n└─────────────────────────────────────────────────────────────────────────────┘
```

**Design rules (non-negotiable):**
- One canonical definition per type; canonical types (`DomainId`, `OperatorId`, `TrustRootId`, `CredentialId`, `ParticipationRecordId`, `CapabilityId`, `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `Endpoint`, `CarrierType`, `Credential`, `AuthToken`, `KeyId`, `Signature`, `FailureDomain`) imported from `ids_000_aa::X`, never redefined.
- `ids-federation` implements the sealed `trait Security` and `trait Membership` contracts from `ids-000-aa/src/contracts/` as declared in the master crate; downstream crates use them as trait bounds, cannot implement them.
- All federation metadata (credentials, trust roots, participation records, capability advertisements) transported as opaque bytes via `ids-carrier::Endpoint`. No IP addresses, ports, sockets, DNS names.
- Crypto surface: ed25519 (signing credentials/trust roots/participation records), sha256/blake3 (content addressing, trust root fingerprints, registry entry digests), xchacha20poly1305/chacha20poly1305 (optional encryption of sensitive registry entries). All ops in `ids-crypto`; this crate declares only usage boundaries.
- Multi-root trust: each domain declares its own trust root(s); no single global trust anchor. `no_global_master` invariant applies at federation level too.
- Domain admission via dynamic trust negotiation (circles of trust, credential exchange, local policy validation) — no pre-configured central member list required.
- Operation lifecycle reuses `ids-000-aa` `OpState` and `CommitState`; federation findings are raised through `ids-000-aa` error/annex channels.
- Trust roots and registry entries versioned via `ProtocolVersion` + `EdgeCaseBits` from `ids-000-aa`.

---

### 3. Detailed Research Dimensions

#### 3.1 Federated Infrastructure Registry

Research basis:
- iRODS federation (irods.org): Zones are independent administrative units. When federated, users of one Zone may grant access to authenticated users from another Zone. Zones share `icat_host`, `zone_name`, `zone_key`, `negotiation_key`; each Zone authenticates its own users; passwords never exchanged between Zones. Federation solves scalability/perishability/transfer-rate issues of centralized sharing; management responsibility distributed to individual Zones; each Zone provides its own view of data in any Zone.
- HDFS Federation (hadoop.apache.org): Multiple Namenodes/namespaces; block storage service separates namespace from block management; datanode cluster membership via registration + heartbeats; block reports, replica placement, replication for under-replicated blocks.
- OAN Yellow Paper (arxiv 2606.03163): OpenAgenet trust-governed resource identity and discovery — Registrar admission, Root acceptance, authorized Discovery, signed invocation; infrastructure authorization requires active governance state + valid Root-issued credential; resource discovery requires package integrity + current-version state + authorized-domain coverage. One Root service boundary per trust domain; interoperable federation among independent domains requires future profiles.

Design: `ids-federation` maintains a federated registry where each domain publishes signed entries (domain identity, operator, declared resources, capabilities, service endpoints as opaque `Endpoint` addresses). Registry entries carry provenance (who signed, when, version), freshness (validity window), and admission status. No single global registry; each domain hosts its own registry shard and exchanges entries via carrier datagrams.

#### 3.2 Multi-Owner Trust Architecture

Research basis:
- W3C Verifiable Credentials Data Model v2.1 (w3.org): Three-party ecosystem — issuer, holder, verifier. Credential = tamper-evident claims cryptographically proving issuer. `credentialStatus` property for revocation/suspension status. Verifiable Data Registry role: mediates creation/verification of identifiers, verification material, schemas, revocation registries. Verification establishes authenticity and currentness; validation (per verifier policy) determines whether claims are acceptable.
- W3C Threat Model for Decentralized Credentials (w3.org DNOTE): Issuer/holder/verifier/VDR roles. Revocation algorithms: Revocation List, Status List (bitstring), Status Assertions, Cryptographic Accumulators. Minimal disclosure principle; rotational identifiers to prevent correlation.
- Agent Trust Infrastructure (ATI, arxiv 2611.23202): Federated trust via peer organizational CAs (AgentCAs), eliminating shared root authority. Each org runs an AgentCA issuing certificates tying identity to org+role. Compromise of one CA does not grant authority over other namespaces. Flat federation, direct peer trust.
- CRATES.md entry 35 / CAPABILITIES.md capability 101: Multi-owner trust = authenticated identities, scoped credentials, policy evaluation, revocation, verifiable participation records without one administrative owner controlling all infrastructure.

Design: Each domain operates as its own trust anchor. Trust roots are per-domain public keys (ed25519 verification keys). Cross-domain trust established by exchanging signed entity statements / credentials, not by pre-shared central roots. `ids-federation` defines `TrustRoot`, `DomainTrust` (which roots this domain trusts, with scope), and `TrustChain` (delegation path from a trusted root to a target identity).

#### 3.3 Authenticated Identities

Research basis:
- DIDs + Verifiable Credentials (IEEE 10597085): DID = self-managed identifier; entity is its own identity provider, eliminating CA-controlled identifiers. VCs = tamper-resistant claims cryptographically bound to a DID, held by subject, presented to verifiers without involving issuer.
- did:oan / OAN (arxiv 2606.03161): Decentralized identifiers for resource identity across independently operated networks; registrars verify onboarding evidence; Root publishes verifiable packages; Discovery returns signed results within governed scope.
- Key transparency (CONIKS, SEEMless, Parakeet, Aegon; Federated Key Transparency arxiv 2609.30824): Append-only key directory with Merkle commitments; clients verify keys against signed directory commitments; detect tampering/omission; cross-provider witness quorum for federation consistency.

Design: `ids-federation` defines identity types (`DomainId`, `OperatorId`, `ParticipantId`) as newtypes over `ids_000_aa::NodeId`-sized byte arrays, plus `IdentityDocument` (signed by domain trust root, containing verification keys, service endpoints, capability metadata). Identity documents registered in the federated registry with provenance and freshness. Key transparency-style Merkle commitments per domain for detecting inconsistent key views.

#### 3.4 Scoped Credentials

Research basis:
- Capability-based security (Dennis & Van Horn 1966; Miller POLA; Saltzer attenuation): Authority = unforgeable tokens scoped to specific operations; holder can attenuate but never amplify. Codecaps (arxiv 1210.5443): cryptographic capabilities in federated clouds; restricted delegation, confinement, revocation, rights amplification.
- RFC 9396 Rich Authorization Requests (`authorization_details`): structured capability claims (type, operations, resources, constraints).
- RFC 9449 DPoP: proof-of-possession key binding for tokens.
- RFC 8693 OAuth 2.0 Token Exchange: delegated authorization across trust domains.
- macaroons: offline attenuation via chained HMAC caveats; monotonic privilege reduction.
- AATs (IETF draft attenuating-authorization-tokens): offline token derivation, capability monotonicity invariant C(child) ⊆ C(parent), depth/lifetime bounds, parent linkage, proof of possession.

Design: `ids-federation` defines `ScopedCredential` — an ed25519-signed token containing (issuer DomainId, subject IdentityId, authorized operations, target resources, constraints: time window, rate limits, argument restrictions, delegation depth, attenuation ceiling), proof-of-possession key binding, unique jti. Credentials are attenuable: a holder can derive a narrower credential offline without issuer contact. Verification is local (signature + constraint check) — no online round-trip to issuer.

#### 3.5 Policy Evaluation

Research basis:
- Open Policy Agent (openpolicyagent.org): General-purpose policy engine; Rego declarative language; decouples policy decision (PDP) from enforcement (PEP); REST API; WASM compilation; prepared queries; sidecar/library deployment for low-latency local decisions.
- Cerbos vs OPA (cerbos.dev): OPA = general-purpose policy engine; Cerbos = purpose-built authorization engine (principal, resource, actions, conditions); both stateless, horizontal scaling.
- Sticky policies / decentralized PDP (arxiv 2609.29308): Evidence quorums for sticky policy authorization; policy remains attached to data across domains; decentralized decision making.
- Zanzibar relationship-based authorization: Global-scale relationship tuples; read-your-writes consistency.

Design: `ids-federation` defines a lightweight policy evaluation surface: policies expressed as declarative rules over (principal, resource, action, context, trust chain). Each domain evaluates locally using its own policy store; no central PDP required. Policy inputs = credential + domain policy + trust chain + operation context; output = allow/deny + conditions. Evaluation is deterministic and side-effect-free so all replicas reach identical decisions.

#### 3.6 Revocation

Research basis:
- W3C VC Data Model credentialStatus + Threat Model: Revocation List, Status List (bitstring: bit per credential index), Status Assertions, Cryptographic Accumulators (proof of validity without exposing other credentials).
- EBSI DevHub credential status strategies: Trusted registry vs issuer-managed direct retrieval; status list as VC signed by issuer; cache duration aligned with revocation batch frequency.
- LTO Network: Public chain as decentralized revocation registry; `credentialStatus` via DID URL; revocation irreversible statement transaction.

Design: `ids-federation` defines `RevocationEntry` per domain: credential ID → status (valid/revoked/suspended) + timestamp + reason (optional). Revocation status published as signed status lists (bitstring per domain) + optional accumulator-based proofs for privacy-preserving validity checks. Revocation check is local (cached status list) with freshness bounded by validity window; stale status triggers re-fetch via carrier.

#### 3.7 Domain Admission

Research basis:
- Dynamic Trust Negotiation (DTN, IEEE 5636065; ACM Mardi Gras '08): Establish trust between strangers at runtime via credential exchange through locally trusted intermediaries (circles of trust, trust pathways). Trust contracts between peers; policy-driven acceptance/rejection.
- Dynamic Trust Model for Federated Identity (IEEE 5636065): Dynamic Trust List (DTL); untrusted SP joins automatically after negotiation; trust engine evaluates credentials against preset policies.
- Federating Trust Perimeters (arxiv 2609.06595): SPIRE/WIF/OpenID Federation require pre-configured common trust anchor; federation for unplanned interactions needs token exchange + remote attestation + DLT-anchored trust roots.
- Bootstrapping mutual attestation (arxiv 2608.20671): Mutual attestation without central trusted operator; Kleene recursion theorem solves reference-value bootstrapping; nodes mutually reference code measurements.

Design: Domain admission = trust negotiation between a new domain and an existing federation member. The requesting domain presents its identity document + trust root + signed credential chain; the admitting domain evaluates against its admission policy (trust root acceptance, capability compatibility, jurisdiction constraints). On success, the admitting domain issues a signed admission credential and publishes the new domain's entry to its registry shard. Admission propagates via gossip/anti-entropy (ids-sync/ids-replication) — no central coordinator.

#### 3.8 Capability Advertisements

Research basis:
- ARD Agent Registry (huggingface hf-discover): Capability manifest — publishers host manifests advertising capabilities; registries maintain continuously updated index; dynamic discovery; richer signals (publisher identity, compliance metadata, usage patterns).
- iRODS federation: Each zone publishes its storage resources; capabilities discovered via zone catalog.
- CAPABILITIES.md capability 100 (Federated infrastructure registry): Registry entries have provenance, freshness, admission status; discover participating regions, H-units, operators, declared resources, capabilities, service endpoints.

Design: `ids-federation` defines `CapabilityAdvertisement` — signed by domain trust root, declaring what the domain offers: data ownership capacity, durability profiles (replica/erasure), geographic restrictions, permitted operators/purposes, resource quotas, service endpoints (opaque `Endpoint`), supported consistency profiles. Advertisements are versioned, freshness-bounded, and discoverable via registry queries carried over carrier datagrams.

#### 3.9 Trust Roots

Research basis:
- Multi-CA PKI federation (smallstep, Google Cloud WIF): Multiple CAs federate trust; relying parties configure trust store with multiple trust anchors; intermediate CAs build chains; trust anchor IDs for compact signaling.
- TLS Trust Anchor Identifiers (IETF draft): Relying parties convey trusted CAs; trust anchor groups for multiple anchors; authenticating party describes available paths; client selects.
- SAINT isolation domains (arxiv 1506.03392): Internet partitioned into ISDs with common local trust roots; cross-signing between ISDs; scoped trust root authority; TRCs signed by threshold of trust roots.
- ATI peer CAs (arxiv 2611.23202): No shared root; compromise of one CA isolated to its namespace.

Design: `ids-federation` defines `TrustRoot` — a domain's self-signed ed25519 root key with metadata (domain ID, key type, validity window, revocation status, hash algorithm). Each domain maintains a `TrustStore` = set of trust roots it accepts, scoped per source domain. Trust paths = chains of trust roots from a configured anchor to a target identity. No single global trust root; `ids-federation` supports multi-root trust with per-domain anchor selection.

#### 3.10 Provenance

Research basis:
- Trustworthy provenance for big data in federated settings (arxiv 2505.24675): Modular architecture leveraging blockchain for immutable, auditable provenance; decentralized interaction; persistent identifiers; provenance versioning.
- Verifiable participation records (FedPoP, arxiv link.springer 10.1186/s13635-026-00244-8): Non-linkable proof of participation in federated learning; privacy-preserving; integrates with secure aggregation.
- Cambium (arxiv 2610.30409): Participant-built commitment roots; no single party decides root contents; logarithmic proofs per cycle.

Design: `ids-federation` defines `ParticipationRecord` — tamper-evident record of a domain's federation actions (admission, credential issuance, revocation, capability advertisement, trust root update). Records are signed by the acting domain, stored in the registry with Merkle commitments, and form an append-only participation log per domain. Provenance enables audit: any verifier can reconstruct a domain's federation history and detect tampering or selective disclosure.

---

### 4. Threat Model & Mitigations

|| Threat | Mitigation ||
||--------|-----------|-------------|
| Compromised domain issues fraudulent credentials | Per-domain trust roots; compromise isolated to that namespace (ATI peer CA model); credential verification checks trust chain to configured anchor |
| Credential forgery / tampering | ed25519 signatures on all credentials, trust roots, participation records; local verification before acceptance |
| Replay of signed credentials | Nonce/jti + per-issuer monotonic sequence + replay cache with TTL; timestamp validity windows |
| Revocation not observed in time | Status lists + accumulator proofs; freshness windows; stale status forces re-fetch via carrier; grace periods aligned with revocation batch frequency |
| Unauthorized domain admission | Trust negotiation + admission policy evaluation; local policy decision per admitting domain; no auto-accept |
| Privilege escalation via credential derivation | Capability monotonicity (C(child) ⊆ C(parent)); depth/lifetime bounds; attenuation-only derivation; proof-of-possession binding |
| Single global trust anchor (centralization / single point of compromise) | Multi-root trust; each domain anchors to its own roots; trust store configurable per domain |
| Trust root compromise undetected | Key transparency-style Merkle commitments per domain; participants detect inconsistent key views; threshold-signed trust root updates |
| Policy evaluation divergence across domains | Deterministic, side-effect-free policy evaluation over same inputs → identical decisions; policy versioning in registry |
| Sensitive registry metadata exposed | xchacha20poly1305/chacha20poly1305 encryption of sensitive entries; minimal disclosure; rotational identifiers |
| Carrier-agnostic rule violation (`std::net`, `tokio::net`, IP/TCP/UDP/QUIC/HTTP/DNS/…) | `Endpoint` sole address type; dependency audit bans networking crates; CI grep for prohibited symbols |
| Disallowed crypto algorithm | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305; `ids-crypto` enforces at implementation layer |
| Registry entry tampering (provenance forgery) | Merkle commitments + signed participation records; append-only log; tamper detection via hash chain |
| Replay cache DoS (unbounded memory) | Fixed-size cache with LRU eviction + TTL; per-peer nonce quota |
| Domain departure mid-obligation | Participation records capture obligations; departure is deliberate (contract-bound), not crash; obligations outlive departure per legal hold / retention |

---

### 5. Integration with Existing Crates

Phase 035 depends on and integrates with:

|| Crate | Role in Phase 035 ||
||-------|-------|-------------------|
|| `ids-000-aa` (master) | Canonical types (`DomainId`, `OperatorId`, `TrustRootId`, `CredentialId`, `ParticipationRecordId`, `CapabilityId`, `NodeId`, `RegionId`, `HUnitId`, `DatasetId`, `Endpoint`, `CarrierType`, `Credential`, `AuthToken`, `KeyId`, `Signature`, `FailureDomain`), sealed `trait Security` / `trait Membership` contracts, `FederationTrust` invariant, `OpState`/`CommitState`, versioning (`ProtocolVersion`, `EdgeCaseBits`), unified `Error` |
|| `ids-carrier` (Phase 002) | Opaque `Endpoint` (`CarrierType` + `Vec<u8>` ≤1024); `send_datagram`/`receive_datagram`; sole traffic channel |
|| `ids-crypto` (Phase 003) | ed25519 sign/verify, sha256/blake3 hashing, xchacha20poly1305/chacha20poly1305 encryption; consumed only |
|| `ids-identity` (Phase 004) | `StableNodeId`/`RotatingNodeId` mapping; participant identity resolution |
|| `ids-membership` (Phase 008) | Node/H-unit/region membership states; admission lifecycle integration |
|| `ids-discovery` (Phase 011) | Gossip-based discovery; federation entry propagation |
|| `ids-placement` (Phase 009) | Consumes capability advertisements for placement decisions |
|| `ids-routing` (Phase 016) | Consumes service endpoints for logical→physical resolution |
|| `ids-replication` (Phase 017) | Replication of federation registry entries across domains |
|| `ids-sync` (Phase 018) | Anti-entropy / gossip for registry convergence |
|| `ids-security` (Phase 031) | Authentication/authorization integration; credential consumption |
|| `ids-policy` / placement-policy (Phase 053) | Policy store consumption for federation policy evaluation |
|| `ids-time` (Phase 005) | `IdsTime` tick basis for validity windows, revocation freshness, participation timestamps |

**Integration points:**
- `ids-federation` implements `ids_000_aa::contracts::Security` and `Membership` (sealed traits) — declared in master, implemented here.
- All outbound traffic: `ids_carrier::Endpoint` datagrams only; `ids-federation` never touches sockets.
- Credentials/trust roots/participation records signed with ed25519; hashes via sha256/blake3 from `ids-crypto`.
- Depends on `ids-000-aa` for types/contracts; `ids-crypto` does NOT depend on `ids-000-aa` (CRATES.md dependency rule) — `ids-federation` depends on both.
- Cargo workspace: `ids-federation = { path = "../ids-federation" }` in each dependent crate.

---

### 6. Proposed File Structure (~90–1000 files in subfolders)

**Crate:** `ids-federation` (to be created at `C:/idfs/ids-federation/`)

```
ids-federation/
├── Cargo.toml                          # ids-000-aa, ids-carrier, ids-crypto; thiserror, serde; no networking crates
├── src/
│   ├── lib.rs                          # crate root; module declarations + root re-exports
│   ├── types.rs                        # DomainId, OperatorId, TrustRootId, CredentialId, ParticipationRecordId, CapabilityId, TrustScope, AdmissionStatus, CredentialStatus
│   ├── errors.rs                       # FederationError enum (thiserror) + recovery hints
│   ├── contract.rs                     # impl of ids_000_aa::contracts::Security + Membership (sealed traits)
│   │
│   ├── registry/                       # Federated infrastructure registry (~8 files)
│   │   ├── mod.rs
│   │   ├── registry_entry.rs           # signed registry entry (domain, resources, capabilities, endpoints)
│   │   ├── registry_shard.rs           # per-domain registry shard (local store)
│   │   ├── entry_provenance.rs         # provenance metadata (signer, timestamp, version)
│   │   ├── entry_freshness.rs          # validity window, staleness detection
│   │   ├── registry_query.rs           # query API over registry entries
│   │   ├── registry_gossip.rs          # gossip/anti-entropy of entries via carrier
│   │   ├── registry_merge.rs           # convergent merge of registry shards
│   │   └── admission_status.rs         # admission status tracking per domain
│   │
│   ├── trust/                          # Multi-owner trust architecture (~9 files)
│   │   ├── mod.rs
│   │   ├── trust_root.rs               # per-domain ed25519 trust root + metadata
│   │   ├── trust_store.rs              # configurable set of accepted trust roots per domain
│   │   ├── trust_path.rs               # chain of trust roots from anchor to target
│   │   ├── trust_chain.rs              # verification of delegation path
│   │   ├── domain_trust.rs             # which roots this domain trusts + scope
│   │   ├── key_transparency.rs         # Merkle commitments per domain for key consistency
│   │   ├── root_rotation.rs            # trust root rotation and revocation
│   │   ├── multi_root.rs               # multi-root trust, per-domain anchor selection
│   │   └── trust_verification.rs       # local verification of credentials against trust store
│   │
│   ├── identity/                       # Authenticated identities (~7 files)
│   │   ├── mod.rs
│   │   ├── domain_id.rs                # DomainId newtype
│   │   ├── operator_id.rs              # OperatorId newtype
│   │   ├── participant_id.rs           # ParticipantId newtype
│   │   ├── identity_document.rs        # signed identity document (keys, endpoints, capability metadata)
│   │   ├── identity_registry.rs        # registration of identity documents in federation registry
│   │   ├── key_consistency.rs          # key transparency consistency checks
│   │   └── identity_resolution.rs      # resolve identity → verification keys + endpoints
│   │
│   ├── credential/                     # Scoped credentials (~9 files)
│   │   ├── mod.rs
│   │   ├── scoped_credential.rs        # ed25519-signed capability token
│   │   ├── credential_claims.rs        # (issuer, subject, operations, resources, constraints, jti, seq)
│   │   ├── credential_attenuate.rs     # offline derivation of narrower credential
│   │   ├── credential_verify.rs        # local signature + constraint verification
│   │   ├── proof_of_possession.rs      # DPoP-style key binding
│   │   ├── credential_jti.rs           # unique identifier + replay protection
│   │   ├── credential_sequence.rs      # per-issuer monotonic sequence
│   │   ├── credential_store.rs         # local credential cache + revocation cross-reference
│   │   └── credential_grant.rs         # grant chain representation
│   │
│   ├── policy/                         # Policy evaluation (~6 files)
│   │   ├── mod.rs
│   │   ├── policy_rule.rs              # declarative rule (principal, resource, action, context)
│   │   ├── policy_evaluator.rs         # deterministic local evaluation
│   │   ├── policy_store.rs             # per-domain policy store (versioned)
│   │   ├── policy_input.rs             # (credential, domain policy, trust chain, operation context)
│   │   ├── policy_decision.rs          # allow/deny + conditions
│   │   └── policy_version.rs           # policy versioning in registry
│   │
│   ├── revocation/                     # Revocation (~7 files)
│   │   ├── mod.rs
│   │   ├── revocation_entry.rs         # credential ID → status + timestamp + reason
│   │   ├── status_list.rs              # bitstring status list per domain (signed)
│   │   ├── accumulator_proof.rs        # privacy-preserving validity proof (optional)
│   │   ├── revocation_status.rs        # valid/revoked/suspended enum
│   │   ├── revocation_checker.rs       # local revocation check with freshness window
│   │   ├── revocation_gossip.rs        # propagate revocation via carrier
│   │   └── grace_period.rs             # grace period aligned with batch frequency
│   │
│   ├── admission/                      # Domain admission (~7 files)
│   │   ├── mod.rs
│   │   ├── admission_request.rs        # new domain presents identity doc + trust root + credentials
│   │   ├── admission_policy.rs         # per-domain admission policy (trust roots, capabilities, jurisdiction)
│   │   ├── trust_negotiation.rs        # dynamic trust negotiation (circles of trust, trust pathways)
│   │   ├── admission_credential.rs     # signed admission credential issued on success
│   │   ├── admission_gossip.rs         # propagate admission via registry gossip
│   │   ├── admission_validate.rs       # validate request against policy
│   │   └── circle_of_trust.rs          # trusted intermediary domains for negotiation
│   │
│   ├── capability/                     # Capability advertisements (~6 files)
│   │   ├── mod.rs
│   │   ├── capability_advertisement.rs # signed ad: capacity, durability, geo, quotas, endpoints
│   │   ├── capability_manifest.rs      # publisher capability manifest (ARD-style)
│   │   ├── capability_discovery.rs     # discover capabilities via registry queries
│   │   ├── durability_profile.rs       # replica/erasure durability profiles offered
│   │   ├── resource_quota.rs           # resource quotas advertised
│   │   └── capability_version.rs       # advertisement versioning + freshness
│   │
│   ├── provenance/                     # Verifiable participation records (~7 files)
│   │   ├── mod.rs
│   │   ├── participation_record.rs     # tamper-evident record of federation action
│   │   ├── participation_log.rs        # append-only log per domain
│   │   ├── merkle_commitment.rs        # Merkle root over participation records
│   │   ├── provenance_chain.rs         # hash chain for tamper detection
│   │   ├── provenance_verify.rs        # verify participation history
│   │   ├── audit_trail.rs              # audit trail construction from records
│   │   └── selective_disclosure.rs     # optional minimal-disclosure proofs
│   │
│   ├── carrier_adapter.rs              # Endpoint-only send/receive wrapper for federation metadata
│   ├── codec/                          # Wire encoding of federation messages (~4 files)
│   │   ├── mod.rs
│   │   ├── federation_frame.rs         # version-tagged frame header
│   │   ├── credential_codec.rs         # serde encode/decode of credentials
│   │   └── trust_root_codec.rs         # encode/decode of trust roots
│   │
│   ├── validation/                     # Input validation rules
│   ├── observability/                  # federation event types (admitted, revoked, advertised, rotated)
│   └── prelude.rs                      # re-export glob for downstream convenience
│
├── tests/                              # conformance tests (no source modification)
│   ├── registry_tests.rs               # registry shard entry + gossip + merge
│   ├── trust_root_tests.rs             # trust root + trust store + multi-root verification
│   ├── identity_tests.rs               # identity document registration + resolution
│   ├── credential_tests.rs             # scoped credential sign/verify/attenuate
│   ├── policy_tests.rs                 # deterministic policy evaluation
│   ├── revocation_tests.rs             # status list + revocation check
│   ├── admission_tests.rs              # trust negotiation + admission credential
│   ├── capability_tests.rs             # capability advertisement + discovery
│   ├── provenance_tests.rs             # participation record + Merkle commitment
│   ├── carrier_agnostic_tests.rs       # asserts no std::net/tokio::net/ip/tcp/udp usage
│   └── crypto_allowlist_tests.rs       # asserts only allowed crypto algorithms referenced
│
├── .gitignore
└── README.md
```

Total: ~150–240 files (within the ~90–1000 target range; room for growth). All files are `.rs` source + `Cargo.toml` + tests; no binary targets. Research only — this structure is a specification, not implemented code.

---

### 7. Key Algorithms & Data Structures

#### 7.1 Scoped Credential (capability token)
```
credential = ed25519_sign(issuer_key, claims)
claims = { issuer: DomainId, subject: IdentityId, operations: Set<Op>,
           resources: Set<Resource>, constraints: {time_window, rate_limit,
           arg_restrictions, del_depth, del_max_depth, attenuation_ceiling},
           jti: [u8;32], seq: u64, cnf: {jkt: DPoP_key_thumbprint} }
verify(credential): check ed25519 sig; check jti not in replay cache; check seq monotonic;
                   check constraints against operation context; check proof-of-possession
attenuate(parent, child_claims): C(child) ⊆ C(parent); del_depth_child ≤ del_depth_parent - 1
```

#### 7.2 Trust Root Verification
```
trust_store[domain] = Set<TrustRoot>   // configured per domain
trust_path = [root_0, root_1, ..., root_n]   // root_0 is configured anchor; root_n signs target
verify_path(path): ∀ i: root_i signed root_{i+1}'s statement; root_0 ∈ trust_store[target_domain]
                 // compromise of one root isolated to its namespace (ATI model)
```

#### 7.3 Status List Revocation
```
status_list[domain] = bitstring[credential_index] → 0=valid, 1=revoked/suspended
status_list signed by domain trust root; published with validity window
check_revoked(credential): index = hash(credential.id) mod list_len;
                          status = status_list[index]; fresh = now - list.timestamp < window
                          if !fresh: re-fetch via carrier
```

#### 7.4 Trust Negotiation (Domain Admission)
```
requester presents: identity_doc, trust_root, credential_chain
admitting domain: evaluate admission_policy(requester) → allow/deny
on allow: issue signed admission_credential; publish requester entry to registry shard;
          gossip entry via carrier to federation peers
```

#### 7.5 Capability Advertisement Discovery
```
advertisement = ed25519_sign(domain_key, {capabilities: Set<Cap>, durability: Profile,
           geo_restrictions: Set<Region>, quotas: ResourceQuota, endpoints: Vec<Endpoint>,
           validity_window})
query(domain, capability): search local registry shard; if missing, gossip query via carrier;
                          return matching advertisements with freshness check
```

#### 7.6 Participation Record (Provenance)
```
record = ed25519_sign(acting_domain_key, {action, target_domain, timestamp, ids_time,
           credential_id, trust_root_id, previous_record_hash})
participation_log = append-only list of records per domain
merkle_root = merkle_tree(participation_log)   // root published as commitment
verify_history(log, root): recompute merkle root; compare; detect tampering
```

#### 7.7 Data Structures
```
TrustRoot { id: TrustRootId, domain: DomainId, key: [u8;32], key_type, validity_window,
            revoked: bool, hash_algo, ids_time }
ScopedCredential { id: CredentialId, issuer: DomainId, subject: IdentityId,
                   operations, resources, constraints, jti, seq, cnf, sig: Signature }
CapabilityAdvertisement { id: CapabilityId, domain: DomainId, capabilities, durability,
                          geo_restrictions, quotas, endpoints: Vec<Endpoint>, validity_window }
ParticipationRecord { id: ParticipationRecordId, actor: DomainId, action, target,
                      timestamp: IdsTime, credential_id, prev_hash, sig: Signature }
RegistryEntry { domain: DomainId, resources, capabilities, endpoints, provenance,
                freshness, admission_status, sig: Signature }
```

---

### 8. Dependencies

`ids-federation/Cargo.toml` (proposed):
- `ids-000-aa = { path = "../ids-000-aa" }` — types, sealed `trait Security`/`Membership`, `FederationTrust` invariant, versioning, errors
- `ids-carrier = { path = "../ids-carrier" }` — `Endpoint` (`CarrierType` + `Vec<u8>`), datagram send/receive
- `ids-crypto = { path = "../ids-crypto" }` — ed25519, sha256, blake3, xchacha20poly1305, chacha20poly1305 (consumed only)
- `thiserror = "2.0"` — error enum derive
- `serde = { version = "1", features = ["derive"] }` — federation message serialization
- `blake3 = "1.5"` — content addressing / trust root fingerprints (type-level/use only)
- `serde_json = "1"` (dev-dependency) — test serialization

**No** `std::net`, `tokio`, `tokio::net`, `libp2p`, `hyper`, `reqwest`, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh dependencies. No async runtime required; datagram-driven. `cargo tree` must show zero networking crates. Sibling crates referenced via adapters; persistence delegation is a design option, not a hard dependency in this phase.

---

### 9. Measurable Acceptance Criteria

|| # | Criterion | Measurement ||
||---|-----------|-------------|
|| 1 | Federation registry specified | `src/registry/` module; signed registry entries; per-domain shards; gossip via carrier |
|| 2 | Multi-owner trust architecture | `src/trust/` module; per-domain trust roots; trust store; multi-root trust; no single global anchor |
|| 3 | Authenticated identities | `src/identity/` module; DomainId/OperatorId/ParticipantId; signed identity documents; key transparency commitments |
|| 4 | Scoped credentials | `src/credential/` module; ed25519-signed capability tokens; offline attenuation; C(child) ⊆ C(parent); proof-of-possession |
|| 5 | Policy evaluation | `src/policy/` module; deterministic local evaluation; allow/deny + conditions; policy versioning |
|| 6 | Revocation | `src/revocation/` module; status lists (bitstring); accumulator proofs optional; freshness windows |
|| 7 | Domain admission | `src/admission/` module; trust negotiation; admission policy; signed admission credential; gossip propagation |
|| 8 | Capability advertisements | `src/capability/` module; signed advertisements; discovery via registry queries; durability profiles |
|| 9 | Trust roots | per-domain ed25519 trust roots; trust paths; multi-root anchor selection; root rotation |
|| 10 | Verifiable participation records | `src/provenance/` module; signed records; append-only log; Merkle commitments; tamper detection |
|| 11 | Sealed `trait Security` / `trait Membership` implemented | `src/contract.rs` implements `ids_000_aa::contracts::Security` + `Membership` |
|| 12 | Canonical types from ids-000-aa only | `types.rs` imports all canonical types from `ids_000_aa`; no redefinitions |
|| 13 | Carrier-agnostic | Zero occurrences of `std::net`, `tokio::net`, `libp2p`, `IpAddr`, `SocketAddr`, `TcpStream`, `UdpSocket`; all traffic via `ids_carrier::Endpoint` |
|| 14 | Crypto allowlist | Only ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 referenced; no RSA/ECDSA/AES-GCM |
|| 15 | No code written / no source modifications | Plan document only; `ids-federation/` crate not created; no edits to existing crates |
|| 16 | Conformance test plan enumerated | `tests/` list covers registry, trust, identity, credential, policy, revocation, admission, capability, provenance, carrier-agnostic, crypto allowlist |
|| 17 | File structure within range | ~150–240 files proposed (within ~90–1000 target) |

---

### 10. Remaining Uncertainties

|| Uncertainty | Impact | Resolution path ||
||-------------|--------|-----------------|
| Trust root bootstrap for first federation domain | How does the first domain establish trust without prior roots | Out-of-band root distribution (documented); subsequent domains via trust negotiation |
| Revocation propagation latency across domains | Revoked credential may be accepted until status list gossip converges | Freshness windows + re-fetch on stale; grace periods; trade-off between latency and cache hit rate |
| Policy language expressiveness vs. determinism | Rego-like expressiveness risks non-determinism across domains | Restrict to decidable fragment; side-effect-free; policy versioning for evolution |
| Credential attenuation chain verification cost | Deep chains increase verification cost | Depth bounds; chain caching; proof-of-possession verification batched |
| Key transparency witness quorum size | Quorum affects detection probability vs. overhead | k=3 of n=5 starting point; configurable per domain |
| Admission policy jurisdiction constraints | Legal/jurisdictional constraints vary per domain | Policy-configurable; not hard-coded; documented per deployment |
| Participation record storage growth | Append-only log grows unbounded | Periodic compaction with Merkle root retention; tombstone GC at causal stability |
| Capability advertisement discovery across large federation | Query fan-out cost | Hierarchical discovery; caching; bloom-filter prefiltering |
| Obligations surviving domain departure | Departing domain's obligations (durability, retention) | Participation records capture obligations; legal hold / retention enforcement out of scope (ids-immutable, ids-retention-enforcement) |
| Wire serialization format | bincode vs CBOR vs serde-derived | Deferred; codec modules use serde; format decision in implementation phase |

---

### 11. Engineering Decisions Summary

|| Decision | Rationale ||
||----------|-----------|
| Separate `ids-federation` crate (not folders inside existing crates) | Matches user directive "try to use separate crates instead of more folders in existing ones"; single-responsibility boundary for multi-owner trust |
| Per-domain trust roots, multi-root trust | Avoids single global trust anchor; compromise isolated to one namespace (ATI peer CA model); aligns with `no_global_master` invariant |
| ed25519-signed scoped credentials with offline attenuation | Capability-based security (POLA, attenuation); local verification without online issuer round-trip; works over unreliable carrier |
| Status list (bitstring) revocation + accumulator option | W3C VC status model; bitstring for simplicity, accumulator for privacy-preserving validity proofs |
| Trust negotiation for domain admission | DTN model; runtime trust establishment between strangers; no pre-configured central member list |
| Merkle commitments per domain (key transparency) | Detect inconsistent key views; tamper detection for trust roots and participation records |
| Append-only participation log with Merkle roots | Provenance/audit trail; tamper-evident history of federation actions |
| All traffic via `ids-carrier::Endpoint` | Carrier-agnostic absolute rule |
| Allowed crypto: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305 | IDS policy; `ids-crypto` implements, this crate declares usage boundaries |
| No code in this phase | Research only per prompts.txt entry 35 |

---

### 12. Sources

1. W3C Verifiable Credentials Data Model v2.1 — w3.org/TR/vc-data-model-2.1 — issuer/holder/verifier, credentialStatus, verifiable data registry, verification vs validation.
2. W3C Threat Model for Decentralized Credentials — w3.org/TR/2026/DNOTE-threat-model-decentralized-credentials-20260529 — revocation lists, Status List bitstrings, cryptographic accumulators, minimal disclosure.
3. OpenID Connect Federation 1.0 (draft 25) — openid.net/specs/openid-connect-federation-1_0-25.html — trust anchors, entity statements, dynamic trust resolution, bilateral vs multilateral federation.
4. Agent Trust Infrastructure (ATI) — arxiv.org/abs/2611.23202 — peer organizational CAs, no shared root authority, compromise isolation per namespace.
5. Decentralized Identifiers + Verifiable Credentials — ieeexplore.ieee.org/document/10597085 — DIDs as self-managed identifiers, VCs as tamper-resistant claims bound to DIDs.
6. OpenAgenet / OAN Yellow Paper — arxiv.org/html/2606.03163 — Registrar admission, Root acceptance, authorized Discovery, signed invocation, per-trust-domain governance.
7. iRODS Federation — irods.org, docs.irods.org/4.1.7/manual/federation — independent zones, shared keys, local authentication, cross-zone access, distributed management.
8. HDFS Federation — hadoop.apache.org/docs/stable/hadoop-project-dist/hadoop-hdfs/Federation.html — multiple namenodes/namespaces, datanode membership, block management.
9. Capability-based security / POLA — Dennis & Van Horn; Miller; Saltzer attenuation — unforgeable tokens, attenuation-only delegation, confused deputy mitigation.
10. RFC 9396 Rich Authorization Requests — datatracker.ietf.org — `authorization_details` structured capability claims.
11. RFC 9449 DPoP — rfc-editor.org/rfc/rfc9449 — proof-of-possession key binding for tokens.
12. RFC 8693 OAuth 2.0 Token Exchange — rfc-editor.org/rfc/rfc8693 — delegated authorization across trust domains.
13. Macaroons — arxiv.org/pdf/1210.5443 — offline attenuation via chained HMAC caveats, monotonic privilege reduction.
14. AATs (Attenuating Authorization Tokens) — ietf.org/archive/id/draft-niyikiza-oauth-attenuating-agent-tokens-01 — C(child) ⊆ C(parent), depth/lifetime bounds, parent linkage.
15. Open Policy Agent — openpolicyagent.org/docs/latest/integration — Rego policy engine, PDP/PEP separation, WASM, prepared queries.
16. Cerbos vs OPA — cerbos.dev/blog/cerbos-vs-opa — general-purpose vs purpose-built authorization engines, stateless horizontal scaling.
17. Sticky policies / decentralized PDP — arxiv.org/html/2609.29308 — evidence quorums, policy attached to data across domains.
18. Dynamic Trust Negotiation — IEEE 5636065, ACM Mardi Gras '08 — circles of trust, trust pathways, runtime trust between strangers.
19. Dynamic Trust Model for Federated Identity — IEEE 5636065 — Dynamic Trust List, automatic join after negotiation, trust engine policy evaluation.
20. Federating Trust Perimeters — arxiv.org/html/2609.06595 — SPIRE/WIF/OpenID Federation limits; token exchange + remote attestation + DLT-anchored trust roots for unplanned interactions.
21. Bootstrapping mutual attestation — arxiv.org/abs/2608.20671 — Kleene recursion theorem, mutual attestation without central trusted operator.
22. Multi-CA PKI federation — smallstep.com/docs/tutorials/pki-trust-model-federation, Google Cloud WIF — multiple CAs, trust stores, intermediate chains, trust anchor IDs.
23. TLS Trust Anchor Identifiers — ietf.org/archive/id/draft-ietf-tls-trust-anchor-ids-04 — relying parties convey trusted CAs, trust anchor groups.
24. SAINT isolation domains — arxiv.org/pdf/1506.03392 — partitioned trust roots, cross-signing, scoped authority, TRCs.
25. Key transparency (CONIKS, SEEMless, Parakeet, Aegon, Federated Key Transparency) — arxiv.org/html/2606.26486, arxiv.org/pdf/2609.30824 — append-only key directories, Merkle commitments, witness quorums.
26. Trustworthy provenance in federated settings — arxiv.org/html/2505.24675 — immutable auditable provenance, persistent identifiers, provenance versioning.
27. FedPoP / verifiable participation — link.springer.com/article/10.1007/s11235-011-9648-4 — non-linkable proof of participation, privacy-preserving.
28. Cambium commitment roots — arxiv.org/abs/2610.30409 — participant-built commitment roots, logarithmic proofs, no single root decider.
29. ARD Agent Registry — raw.githubusercontent.com/huggingface/hf-discover/main/spec/ard.md — capability manifest, dynamic discovery, publisher identity, compliance metadata.
30. IDS internal sources — C:/idfs/prompts.txt entry 35 (lines 520-534); CRATES.md entry 35; CAPABILITIES.md capabilities 99-101; DIFFERENCES.md federation analysis; ids-000-aa `lib.rs` (`FederationTrust` invariant line 67, `Credential`/`AuthToken`/`KeyId`/`Signature` types line 83); ids-000-aa `src/invariants/federation_trust.rs`.

---

**End of plan-035.md. Research only — no code written, no files created, no repository changes made beyond this plan document.**