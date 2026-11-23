# IDS Phase 015 — ids-query: Research Findings Tracker

**Topic:** Create the `ids-query` crate: Query decomposition across partitions, nodes, regions (distributed query planning and execution).
**Source prompt:** C:/idfs/prompts.txt entry 15 (lines 220-234): "Phase 015 — Create the ids-query crate: Query decomposition across partitions, nodes, regions."
**Output document:** C:/idfs/plans/plan-015.md
**Methodology:** 30 targeted web searches executed in one parallel batch (Q001-Q030). Each result classified ESTABLISHED / HYPOTHESIS / SPECULATION and checked against constraints:
- Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC, HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or mesh. All traffic via ids-carrier::Endpoint (CarrierType + Vec<u8>).
- Allowed crypto only: ed25519, xchacha20poly1305, sha256, blake3, chacha20poly1305.
- ids-000-aa is the master crate with all canonical types and sealed trait contracts.
- Research only — no code, no source modifications, only the plan document.

## Evidence classification definitions

- **ESTABLISHED** — peer-reviewed / textbook consensus, reproducible, widely implemented.
- **HYPOTHESIS** — proposed in literature or implemented in production systems but not conclusively generalized.
- **SPECULATION** — conjectural, no strong experimental or production basis.

## Batch index

| Batch | Theme | Query range | Status |
|-------|-------|-------------|--------|
| 1 | Distributed query planning & execution (partitions, nodes, regions) | Q001-Q030 | complete |

## Batch Results

(Details recorded in results.md, one entry per query ID.)
