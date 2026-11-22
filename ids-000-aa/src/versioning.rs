///////////////////////////////////////////////////////////////////////////////
// ids-000-aa / src/versioning.rs
//
// Protocol versioning + edge-case version bits as a compile-time bitfield.
//
// Canonical definitions in this file:
//   - ProtocolVersion  — semantic (major.minor.patch) wire/protocol version
//   - EdgeCaseBits     — u128 bitfield, one bit per protocol invariant (40 bits)
//   - VersionRange     — inclusive/exclusive range of acceptable versions
//   - SemanticVersion  — parsed, comparable semantic version
//   - ProtocolFeature  — a named, gated protocol capability
//   - ProtocolRegistry — registry of supported protocol versions/features
//   - CompatibilityMatrix — pairwise version compatibility table
//   - VersionGraph     — DAG of protocol versions with upgrade/downgrade edges
//   - MigrationPath    — a version migration with preconditions and steps
//   - VersionHistory   — append-only log of version changes per node
//   - VersionNegotiation — negotiate a mutually acceptable protocol version
//   - EdgeCaseCompiler — compile edge-case bits into runtime enforcement
//   - EdgeCaseProfile  — named collection of edge-case bits (profiles)
//   - ChangeRecord     — single version change event
//
// Carrier-agnostic: no std::net, tokio::net, libp2p, IP, TCP, UDP, QUIC,
// HTTP, DNS, Wi-Fi, Bluetooth, cellular, VPN, or conventional mesh.
//
// Every invariant in `invariants/` maps to exactly one bit here, so enabling
// or disabling an edge case is a one-line change in this file plus one
// downstream impl.
//
// Design principles:
//   1. All version arithmetic is saturating / checked — no panics on overflow.
//   2. Bitfield operations are constant-time and branch-free where possible.
//   3. Compatibility is monotonic: newer minor versions accept older ones.
//   4. Migration paths are explicit, versioned, and reversible where possible.
//   5. Negotiation always converges to the highest mutually supported version.
//   6. Every bit has a name, a description, and a compile-time constant.

#![allow(dead_code)]

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL VERSION
///////////////////////////////////////////////////////////////////////////////

/// A semantic protocol version: `major.minor.patch`.
///
/// Major versions break wire compatibility. Minor versions add features in a
/// backward-compatible way. Patch versions fix bugs without changing behavior.
///
/// Comparisons follow semantic ordering: `1.2.3 < 1.2.4 < 1.3.0 < 2.0.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ProtocolVersion {
    /// Version `0.0.0` — unset / unknown protocol.
    pub const ZERO: ProtocolVersion = ProtocolVersion::new(0, 0, 0);

    /// Version `1.0.0` — first stable IDS protocol release.
    pub const V1_0_0: ProtocolVersion = ProtocolVersion::new(1, 0, 0);

    /// Version `1.1.0` — edge-case bits introduced.
    pub const V1_1_0: ProtocolVersion = ProtocolVersion::new(1, 1, 0);

    /// Version `1.2.0` — carrier-agnostic Endpoint introduced.
    pub const V1_2_0: ProtocolVersion = ProtocolVersion::new(1, 2, 0);

    /// Version `2.0.0` — current IDS protocol.
    pub const V2_0_0: ProtocolVersion = ProtocolVersion::new(2, 0, 0);

    /// Construct a new protocol version.
    #[inline]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Major component.
    #[inline]
    pub const fn major(&self) -> u16 {
        self.major
    }

    /// Minor component.
    #[inline]
    pub const fn minor(&self) -> u16 {
        self.minor
    }

    /// Patch component.
    #[inline]
    pub const fn patch(&self) -> u16 {
        self.patch
    }

    /// Whether this is the zero (unset) version.
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.major == 0 && self.minor == 0 && self.patch == 0
    }

    /// Whether this is a stable release (patch == 0 and minor == 0).
    #[inline]
    pub const fn is_stable_release(&self) -> bool {
        self.patch == 0 && self.minor == 0
    }

    /// Bump the major version, resetting minor and patch to zero.
    #[inline]
    pub const fn bump_major(&self) -> Self {
        Self {
            major: self.major.saturating_add(1),
            minor: 0,
            patch: 0,
        }
    }

    /// Bump the minor version, resetting patch to zero.
    #[inline]
    pub const fn bump_minor(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor.saturating_add(1),
            patch: 0,
        }
    }

    /// Bump the patch version.
    #[inline]
    pub const fn bump_patch(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch.saturating_add(1),
        }
    }

    /// Whether `self` is wire-compatible with `other`:
    /// same major version, and `self` >= `other` in minor/patch.
    #[inline]
    pub const fn wire_compatible_with(&self, other: ProtocolVersion) -> bool {
        if self.major != other.major {
            return false;
        }
        if self.minor < other.minor {
            return false;
        }
        if self.minor == other.minor && self.patch < other.patch {
            return false;
        }
        true
    }

    /// Whether `other` can speak this protocol (other >= self in ordering).
    #[inline]
    pub const fn speaks(&self, other: ProtocolVersion) -> bool {
        other >= *self
    }

    /// Encode as a single u64 for storage / hashing: major << 40 | minor << 24 | patch.
    #[inline]
    pub const fn encode(&self) -> u64 {
        (self.major as u64) << 40 | (self.minor as u64) << 24 | (self.patch as u64)
    }

    /// Decode from a u64 produced by `encode`.
    #[inline]
    pub const fn decode(encoded: u64) -> Self {
        Self {
            major: ((encoded >> 40) & 0xFFFF) as u16,
            minor: ((encoded >> 24) & 0xFFFF) as u16,
            patch: (encoded & 0xFFFFFF) as u16,
        }
    }

    /// Human-readable string: `major.minor.patch`.
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Display-friendly short form.
    pub fn to_short_string(&self) -> String {
        if self.patch == 0 && self.minor == 0 {
            format!("v{}", self.major)
        } else if self.patch == 0 {
            format!("v{}.{}", self.major, self.minor)
        } else {
            format!("v{}.{}.{}", self.major, self.minor, self.patch)
        }
    }

    /// Whether this version supports edge-case bits (>= 1.1.0).
    #[inline]
    pub const fn supports_edge_case_bits(&self) -> bool {
        *self >= ProtocolVersion::V1_1_0
    }

    /// Whether this version supports carrier-agnostic Endpoints (>= 1.2.0).
    #[inline]
    pub const fn supports_carrier_agnostic_endpoints(&self) -> bool {
        *self >= ProtocolVersion::V1_2_0
    }

    /// Next patch version.
    #[inline]
    pub const fn next_patch(&self) -> Self {
        self.bump_patch()
    }

    /// Previous patch version (saturating at 0).
    #[inline]
    pub const fn prev_patch(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch.saturating_sub(1),
        }
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        ProtocolVersion::V2_0_0
    }
}

///////////////////////////////////////////////////////////////////////////////
// SEMANTIC VERSION PARSER
///////////////////////////////////////////////////////////////////////////////

/// Error returned when a version string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionParseError {
    pub input: String,
    pub reason: VersionParseReason,
}

/// Reason a version string failed to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionParseReason {
    EmptyInput,
    TooManyComponents,
    TooFewComponents,
    NonNumericComponent,
    ComponentTooLarge,
    LeadingZeroRejected,
    NegativeComponent,
    TrailingCharacters,
}

/// Parses `major.minor.patch` strings into `ProtocolVersion`.
#[derive(Debug, Clone, Default)]
pub struct SemanticVersionParser {
    pub reject_leading_zeros: bool,
    pub allow_patch_omission: bool,
    pub allow_minor_omission: bool,
}

impl SemanticVersionParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Strict parser: exactly three numeric components, no leading zeros.
    pub fn strict() -> Self {
        Self {
            reject_leading_zeros: true,
            allow_patch_omission: false,
            allow_minor_omission: false,
        }
    }

    /// Lenient parser: allows omitting patch/minor, accepts leading zeros.
    pub fn lenient() -> Self {
        Self {
            reject_leading_zeros: false,
            allow_patch_omission: true,
            allow_minor_omission: true,
        }
    }

    /// Parse a version string into a `ProtocolVersion`.
    pub fn parse(&self, input: &str) -> Result<ProtocolVersion, VersionParseError> {
        if input.is_empty() {
            return Err(VersionParseError {
                input: input.to_string(),
                reason: VersionParseReason::EmptyInput,
            });
        }
        let trimmed = input.trim();
        if trimmed.starts_with('v') || trimmed.starts_with('V') {
            let _ = &trimmed[1..];
        }
        let parts: Vec<&str> = trimmed.split('.').collect();
        if parts.len() > 3 {
            return Err(VersionParseError {
                input: input.to_string(),
                reason: VersionParseReason::TooManyComponents,
            });
        }
        if parts.len() < 2 && !self.allow_minor_omission {
            return Err(VersionParseError {
                input: input.to_string(),
                reason: VersionParseReason::TooFewComponents,
            });
        }
        let major: u16 = self.parse_component(parts[0], "major")?;
        let minor: u16 = if parts.len() >= 2 {
            self.parse_component(parts[1], "minor")?
        } else {
            0
        };
        let patch: u16 = if parts.len() >= 3 {
            self.parse_component(parts[2], "patch")?
        } else if self.allow_patch_omission {
            0
        } else {
            return Err(VersionParseError {
                input: input.to_string(),
                reason: VersionParseReason::TooFewComponents,
            });
        };
        Ok(ProtocolVersion::new(major, minor, patch))
    }

    fn parse_component(&self, s: &str, name: &str) -> Result<u16, VersionParseError> {
        if s.is_empty() {
            return Err(VersionParseError {
                input: name.to_string(),
                reason: VersionParseReason::NonNumericComponent,
            });
        }
        if self.reject_leading_zeros && s.len() > 1 && s.starts_with('0') {
            return Err(VersionParseError {
                input: name.to_string(),
                reason: VersionParseReason::LeadingZeroRejected,
            });
        }
        let value: u32 = match s.parse::<u32>() {
            Ok(v) => v,
            Err(_) => {
                return Err(VersionParseError {
                    input: name.to_string(),
                    reason: VersionParseReason::NonNumericComponent,
                })
            }
        };
        if value > u16::MAX as u32 {
            return Err(VersionParseError {
                input: name.to_string(),
                reason: VersionParseReason::ComponentTooLarge,
            });
        }
        Ok(value as u16)
    }

    /// Parse many versions, returning the first error.
    pub fn parse_many(
        &self,
        inputs: &[&str],
    ) -> Result<Vec<ProtocolVersion>, VersionParseError> {
        let mut out = Vec::with_capacity(inputs.len());
        for input in inputs {
            out.push(self.parse(input)?);
        }
        Ok(out)
    }

    /// Parse many versions, collecting errors alongside successes.
    pub fn parse_many_collect(
        &self,
        inputs: &[&str],
    ) -> (Vec<ProtocolVersion>, Vec<VersionParseError>) {
        let mut ok = Vec::new();
        let mut err = Vec::new();
        for input in inputs {
            match self.parse(input) {
                Ok(v) => ok.push(v),
                Err(e) => err.push(e),
            }
        }
        (ok, err)
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION RANGE
///////////////////////////////////////////////////////////////////////////////

/// A range of acceptable protocol versions: `[min, max]` inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VersionRange {
    pub min: ProtocolVersion,
    pub max: ProtocolVersion,
}

impl VersionRange {
    pub const fn new(min: ProtocolVersion, max: ProtocolVersion) -> Self {
        Self { min, max }
    }

    /// Range accepting exactly one version.
    pub const fn exact(v: ProtocolVersion) -> Self {
        Self { min: v, max: v }
    }

    /// Range accepting `v` and anything newer (up to u16::MAX).
    pub const fn at_least(v: ProtocolVersion) -> Self {
        Self {
            min: v,
            max: ProtocolVersion::new(u16::MAX, u16::MAX, u16::MAX),
        }
    }

    /// Whether `v` is inside this range.
    #[inline]
    pub const fn contains(&self, v: ProtocolVersion) -> bool {
        v >= self.min && v <= self.max
    }

    /// Whether this range overlaps with `other`.
    #[inline]
    pub const fn overlaps(&self, other: VersionRange) -> bool {
        self.min <= other.max && other.min <= self.max
    }

    /// The intersection of this range and `other`, if non-empty.
    pub fn intersect(&self, other: VersionRange) -> Option<VersionRange> {
        let lo = if self.min > other.min { self.min } else { other.min };
        let hi = if self.max < other.max { self.max } else { other.max };
        if lo <= hi {
            Some(VersionRange::new(lo, hi))
        } else {
            None
        }
    }

    /// The union of this range and `other` (convex hull).
    pub fn union(&self, other: VersionRange) -> VersionRange {
        let lo = if self.min < other.min { self.min } else { other.min };
        let hi = if self.max > other.max { self.max } else { other.max };
        VersionRange::new(lo, hi)
    }

    /// Width of the range in encoded u64 space (approximate).
    pub fn width_encoded(&self) -> u64 {
        self.max.encode().saturating_sub(self.min.encode())
    }

    /// Whether the range is a single point.
    #[inline]
    pub const fn is_point(&self) -> bool {
        self.min == self.max
    }
}

///////////////////////////////////////////////////////////////////////////////
// EDGE CASE BITS
///////////////////////////////////////////////////////////////////////////////

/// Compile-time bitfield of protocol invariants (edge cases).
///
/// One `u128` holds up to 128 edge cases; IDS uses 40 (bits 0..39), leaving
/// bits 32..127 for future edge cases. Each invariant in `invariants/` maps to
/// exactly one bit here, declared as a `pub const` so downstream crates can
/// refer to `EdgeCaseBits::BINARY_COMPLETION` etc.
///
/// Bit assignment (mirrors `invariants/mod.rs` order):
///   0  BINARY_COMPLETION        — 100% done or failed; no partial terminal state
///   1  DEAD_NODE_IDENTITY       — dead node never returns with old identity
///   2  IDLE_ROTATION            — RNI rotates only while node idle
///   3  NO_GLOBAL_MASTER          — dataset-scoped ordering, no permanent master
///   4  OUT_EQ_IN                — bytes-out == bytes-in + acked
///   5  COORDINATED_DELETION      — all holders delete simultaneously or not at all
///   6  FENCING_STONITH            — STONITH/fencing before any failover
///   7  QUORUM_INTERSECTION        — read quorum ∩ write quorum ≠ ∅
///   8  ERASURE_THRESHOLD          — any k of n chunks reconstructs
///   9  CORRELATED_FAILURE          — replicas never share failure domain
///  10  HUNIT_COMPENSATION          — H-unit failure compensated by another H-unit
///  11  REGIONAL_AUTONOMY           — regional clusters operate independently
///  12  HYBRID_REDUNDANCY           — full replication + erasure coding coexist
///  13  OPERATION_LIFECYCLE         — received→ordered→executing→committed→sync→verified→failed
///  14  DURABILITY_ACK              — durability-aware acknowledgement
///  15  ORDERED_READ_VISIBILITY     — reads respect committed position prefix
///  16  NODE_ROSTER                 — fixed node roster at join
///  17  REGION_AFFINITY             — near-to-far routing, no abrupt region change
///  18  IDENTITY_ROTATION           — stable internal identity + rotating RNI
///  19  IDS_STANDARD_TIME           — proprietary time basis, not wall-clock
///  20  DAILY_INTEGRITY             — daily integrity checks, fishy-state detection
///  21  DB_RETIREMENT               — temporary DB retirement during RNI rotation
///  22  READ_FALLBACK               — fallback to next closest eligible replica
///  23  TOPOLOGY_DIVERSITY          — different locations/H-units/regions per copy
///  24  WRITE_FANOUT                — ~30 databases written simultaneously
///  25  CUSTODY_GRAPH               — data custody and responsibility graph
///  26  CHECKSUM_BOUNDARY           — end-to-end checksum verification boundary
///  27  REPLICA_FRESHNESS           — replica freshness validation for reads
///  28  COMMIT_PREFIX               — committed-prefix tracking for ordered reads
///  29  VERSION_VECTOR              — vector clock for causality tracking
///  30  MEMBERSHIP_EPOCH            — membership change epoch tracking
///  31  PLACEMENT_POLICY            — placement policy enforcement
///  32..127 available for future edge cases via compile-time cfg
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EdgeCaseBits(u128);

impl EdgeCaseBits {
    /// No edge cases enabled.
    pub const NONE: EdgeCaseBits = EdgeCaseBits(0);

    /// All 40 IDS edge cases enabled.
    pub const ALL_KNOWN: EdgeCaseBits = EdgeCaseBits(0x0000_0000_0000_00FF_FFFF_FFFF_FFFF_FFFF);

    /// Bit 0 — Binary Completion.
    pub const BINARY_COMPLETION: u128 = 1 << 0;
    /// Bit 1 — Dead Node Identity.
    pub const DEAD_NODE_IDENTITY: u128 = 1 << 1;
    /// Bit 2 — Idle Rotation.
    pub const IDLE_ROTATION: u128 = 1 << 2;
    /// Bit 3 — No Global Master.
    pub const NO_GLOBAL_MASTER: u128 = 1 << 3;
    /// Bit 4 — Out Eq In.
    pub const OUT_EQ_IN: u128 = 1 << 4;
    /// Bit 5 — Coordinated Deletion.
    pub const COORDINATED_DELETION: u128 = 1 << 5;
    /// Bit 6 — Fencing STONITH.
    pub const FENCING_STONITH: u128 = 1 << 6;
    /// Bit 7 — Quorum Intersection.
    pub const QUORUM_INTERSECTION: u128 = 1 << 7;
    /// Bit 8 — Erasure Threshold.
    pub const ERASURE_THRESHOLD: u128 = 1 << 8;
    /// Bit 9 — Correlated Failure.
    pub const CORRELATED_FAILURE: u128 = 1 << 9;
    /// Bit 10 — H-Unit Compensation.
    pub const HUNIT_COMPENSATION: u128 = 1 << 10;
    /// Bit 11 — Regional Autonomy.
    pub const REGIONAL_AUTONOMY: u128 = 1 << 11;
    /// Bit 12 — Hybrid Redundancy.
    pub const HYBRID_REDUNDANCY: u128 = 1 << 12;
    /// Bit 13 — Operation Lifecycle.
    pub const OPERATION_LIFECYCLE: u128 = 1 << 13;
    /// Bit 14 — Durability Ack.
    pub const DURABILITY_ACK: u128 = 1 << 14;
    /// Bit 15 — Ordered Read Visibility.
    pub const ORDERED_READ_VISIBILITY: u128 = 1 << 15;
    /// Bit 16 — Node Roster.
    pub const NODE_ROSTER: u128 = 1 << 16;
    /// Bit 17 — Region Affinity.
    pub const REGION_AFFINITY: u128 = 1 << 17;
    /// Bit 18 — Identity Rotation.
    pub const IDENTITY_ROTATION: u128 = 1 << 18;
    /// Bit 19 — IDS Standard Time.
    pub const IDS_STANDARD_TIME: u128 = 1 << 19;
    /// Bit 20 — Daily Integrity.
    pub const DAILY_INTEGRITY: u128 = 1 << 20;
    /// Bit 21 — DB Retirement.
    pub const DB_RETIREMENT: u128 = 1 << 21;
    /// Bit 22 — Read Fallback.
    pub const READ_FALLBACK: u128 = 1 << 22;
    /// Bit 23 — Topology Diversity.
    pub const TOPOLOGY_DIVERSITY: u128 = 1 << 23;
    /// Bit 24 — Write Fanout.
    pub const WRITE_FANOUT: u128 = 1 << 24;
    /// Bit 25 — Custody Graph.
    pub const CUSTODY_GRAPH: u128 = 1 << 25;
    /// Bit 26 — Checksum Boundary.
    pub const CHECKSUM_BOUNDARY: u128 = 1 << 26;
    /// Bit 27 — Replica Freshness.
    pub const REPLICA_FRESHNESS: u128 = 1 << 27;
    /// Bit 28 — Commit Prefix.
    pub const COMMIT_PREFIX: u128 = 1 << 28;
    /// Bit 29 — Version Vector.
    pub const VERSION_VECTOR: u128 = 1 << 29;
    /// Bit 30 — Membership Epoch.
    pub const MEMBERSHIP_EPOCH: u128 = 1 << 30;
    /// Bit 31 — Placement Policy.
    pub const PLACEMENT_POLICY: u128 = 1 << 31;

    /// Highest defined edge-case bit (31).
    pub const HIGHEST_KNOWN_BIT: u128 = 31;

    /// Total known edge cases.
    pub const KNOWN_EDGE_CASE_COUNT: usize = 40;

    /// Construct from a raw u128.
    #[inline]
    pub const fn from_raw(bits: u128) -> Self {
        Self(bits)
    }

    /// Raw u128 value.
    #[inline]
    pub const fn raw(&self) -> u128 {
        self.0
    }

    /// Whether bit `bit` is set.
    #[inline]
    pub const fn has(&self, bit: u128) -> bool {
        (self.0 & bit) != 0
    }

    /// Enable bit `bit`.
    #[inline]
    pub fn enable(&mut self, bit: u128) {
        self.0 |= bit;
    }

    /// Disable bit `bit`.
    #[inline]
    pub fn disable(&mut self, bit: u128) {
        self.0 &= !bit;
    }

    /// Toggle bit `bit`.
    #[inline]
    pub fn toggle(&mut self, bit: u128) {
        self.0 ^= bit;
    }

    /// Enable a named edge case constant.
    pub fn enable_named(&mut self, bit: u128) {
        self.enable(bit);
    }

    /// Whether any edge case is enabled.
    #[inline]
    pub const fn any(&self) -> bool {
        self.0 != 0
    }

    /// Whether all known edge cases are enabled.
    #[inline]
    pub const fn all_known(&self) -> bool {
        (self.0 & Self::ALL_KNOWN.raw()) == Self::ALL_KNOWN.raw()
    }

    /// Count of enabled known bits.
    pub fn enabled_count(&self) -> u32 {
        (self.0 & Self::ALL_KNOWN.raw()).count_ones()
    }

    /// List of enabled named edge cases.
    pub fn enabled_names(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.has(Self::BINARY_COMPLETION) {
            out.push("binary_completion");
        }
        if self.has(Self::DEAD_NODE_IDENTITY) {
            out.push("dead_node_identity");
        }
        if self.has(Self::IDLE_ROTATION) {
            out.push("idle_rotation");
        }
        if self.has(Self::NO_GLOBAL_MASTER) {
            out.push("no_global_master");
        }
        if self.has(Self::OUT_EQ_IN) {
            out.push("out_eq_in");
        }
        if self.has(Self::COORDINATED_DELETION) {
            out.push("coordinated_deletion");
        }
        if self.has(Self::FENCING_STONITH) {
            out.push("fencing_stonith");
        }
        if self.has(Self::QUORUM_INTERSECTION) {
            out.push("quorum_intersection");
        }
        if self.has(Self::ERASURE_THRESHOLD) {
            out.push("erasure_threshold");
        }
        if self.has(Self::CORRELATED_FAILURE) {
            out.push("correlated_failure");
        }
        if self.has(Self::HUNIT_COMPENSATION) {
            out.push("hunit_compensation");
        }
        if self.has(Self::REGIONAL_AUTONOMY) {
            out.push("regional_autonomy");
        }
        if self.has(Self::HYBRID_REDUNDANCY) {
            out.push("hybrid_redundancy");
        }
        if self.has(Self::OPERATION_LIFECYCLE) {
            out.push("operation_lifecycle");
        }
        if self.has(Self::DURABILITY_ACK) {
            out.push("durability_ack");
        }
        if self.has(Self::ORDERED_READ_VISIBILITY) {
            out.push("ordered_read_visibility");
        }
        if self.has(Self::NODE_ROSTER) {
            out.push("node_roster");
        }
        if self.has(Self::REGION_AFFINITY) {
            out.push("region_affinity");
        }
        if self.has(Self::IDENTITY_ROTATION) {
            out.push("identity_rotation");
        }
        if self.has(Self::IDS_STANDARD_TIME) {
            out.push("ids_standard_time");
        }
        if self.has(Self::DAILY_INTEGRITY) {
            out.push("daily_integrity");
        }
        if self.has(Self::DB_RETIREMENT) {
            out.push("db_retirement");
        }
        if self.has(Self::READ_FALLBACK) {
            out.push("read_fallback");
        }
        if self.has(Self::TOPOLOGY_DIVERSITY) {
            out.push("topology_diversity");
        }
        if self.has(Self::WRITE_FANOUT) {
            out.push("write_fanout");
        }
        if self.has(Self::CUSTODY_GRAPH) {
            out.push("custody_graph");
        }
        if self.has(Self::CHECKSUM_BOUNDARY) {
            out.push("checksum_boundary");
        }
        if self.has(Self::REPLICA_FRESHNESS) {
            out.push("replica_freshness");
        }
        if self.has(Self::COMMIT_PREFIX) {
            out.push("commit_prefix");
        }
        if self.has(Self::VERSION_VECTOR) {
            out.push("version_vector");
        }
        if self.has(Self::MEMBERSHIP_EPOCH) {
            out.push("membership_epoch");
        }
        if self.has(Self::PLACEMENT_POLICY) {
            out.push("placement_policy");
        }
        out
    }

    /// Description of a named edge-case bit.
    pub fn description(bit: u128) -> &'static str {
        match bit {
            Self::BINARY_COMPLETION => "binary_completion: 100% done or failed; no partial terminal state",
            Self::DEAD_NODE_IDENTITY => "dead_node_identity: dead node never returns with old identity",
            Self::IDLE_ROTATION => "idle_rotation: RNI rotates only while node idle",
            Self::NO_GLOBAL_MASTER => "no_global_master: dataset-scoped ordering only",
            Self::OUT_EQ_IN => "out_eq_in: bytes-out == bytes-in + acked",
            Self::COORDINATED_DELETION => "coordinated_deletion: all holders delete simultaneously or not at all",
            Self::FENCING_STONITH => "fencing_stonith: STONITH/fencing before any failover",
            Self::QUORUM_INTERSECTION => "quorum_intersection: read quorum ∩ write quorum ≠ ∅",
            Self::ERASURE_THRESHOLD => "erasure_threshold: any k of n chunks reconstructs",
            Self::CORRELATED_FAILURE => "correlated_failure: replicas never share failure domain",
            Self::HUNIT_COMPENSATION => "hunit_compensation: H-unit failure compensated by another H-unit",
            Self::REGIONAL_AUTONOMY => "regional_autonomy: regional clusters operate independently",
            Self::HYBRID_REDUNDANCY => "hybrid_redundancy: full replication + erasure coding coexist",
            Self::OPERATION_LIFECYCLE => "operation_lifecycle: received→ordered→executing→committed→sync→verified→failed",
            Self::DURABILITY_ACK => "durability_ack: durability-aware acknowledgement",
            Self::ORDERED_READ_VISIBILITY => "ordered_read_visibility: reads respect committed position prefix",
            Self::NODE_ROSTER => "node_roster: fixed node roster at join",
            Self::REGION_AFFINITY => "region_affinity: near-to-far routing",
            Self::IDENTITY_ROTATION => "identity_rotation: stable internal identity + rotating RNI",
            Self::IDS_STANDARD_TIME => "ids_standard_time: proprietary time basis, not wall-clock",
            Self::DAILY_INTEGRITY => "daily_integrity: daily integrity checks, fishy-state detection",
            Self::DB_RETIREMENT => "db_retirement: temporary DB retirement during RNI rotation",
            Self::READ_FALLBACK => "read_fallback: fallback to next closest eligible replica",
            Self::TOPOLOGY_DIVERSITY => "topology_diversity: different locations/H-units/regions per copy",
            Self::WRITE_FANOUT => "write_fanout: ~30 databases written simultaneously",
            Self::CUSTODY_GRAPH => "custody_graph: data custody and responsibility graph",
            Self::CHECKSUM_BOUNDARY => "checksum_boundary: end-to-end checksum verification boundary",
            Self::REPLICA_FRESHNESS => "replica_freshness: replica freshness validation for reads",
            Self::COMMIT_PREFIX => "commit_prefix: committed-prefix tracking for ordered reads",
            Self::VERSION_VECTOR => "version_vector: vector clock for causality tracking",
            Self::MEMBERSHIP_EPOCH => "membership_epoch: membership change epoch tracking",
            Self::PLACEMENT_POLICY => "placement_policy: placement policy enforcement",
            _ => "unknown edge case",
        }
    }

    /// Clear all bits.
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    /// Enable all known edge cases.
    pub fn enable_all_known(&mut self) {
        self.0 |= Self::ALL_KNOWN.raw();
    }

    /// Disable all known edge cases.
    pub fn disable_all_known(&mut self) {
        self.0 &= !Self::ALL_KNOWN.raw();
    }

    /// Whether only future (unknown) bits are set.
    pub const fn only_future_bits(&self) -> bool {
        (self.0 & Self::ALL_KNOWN.raw()) == 0 && self.0 != 0
    }

    /// Whether any future (unknown) bits are set.
    pub const fn has_future_bits(&self) -> bool {
        (self.0 & !Self::ALL_KNOWN.raw()) != 0
    }

    /// Merge two bitfields (union).
    #[inline]
    pub const fn merge(&self, other: EdgeCaseBits) -> EdgeCaseBits {
        EdgeCaseBits(self.0 | other.0)
    }

    /// Common bits between two bitfields (intersection).
    #[inline]
    pub const fn common(&self, other: EdgeCaseBits) -> EdgeCaseBits {
        EdgeCaseBits(self.0 & other.0)
    }

    /// Bits in `self` but not in `other`.
    #[inline]
    pub const fn difference(&self, other: EdgeCaseBits) -> EdgeCaseBits {
        EdgeCaseBits(self.0 & !other.0)
    }

    /// Bits in exactly one of the two bitfields (symmetric difference).
    #[inline]
    pub const fn symmetric_difference(&self, other: EdgeCaseBits) -> EdgeCaseBits {
        EdgeCaseBits(self.0 ^ other.0)
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL FEATURE
///////////////////////////////////////////////////////////////////////////////

/// A named protocol capability gated behind an edge-case bit and a version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolFeature {
    pub name: String,
    pub description: String,
    pub edge_case_bit: Option<u128>,
    pub min_version: ProtocolVersion,
    pub max_version: Option<ProtocolVersion>,
    pub enabled: bool,
}

impl ProtocolFeature {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        min_version: ProtocolVersion,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            edge_case_bit: None,
            min_version,
            max_version: None,
            enabled: false,
        }
    }

    pub fn with_edge_case(mut self, bit: u128) -> Self {
        self.edge_case_bit = Some(bit);
        self
    }

    pub fn with_max_version(mut self, max: ProtocolVersion) -> Self {
        self.max_version = Some(max);
        self
    }

    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Whether this feature is available for `version` given `bits`.
    pub fn is_available(&self, version: ProtocolVersion, bits: EdgeCaseBits) -> bool {
        if !self.enabled {
            return false;
        }
        if version < self.min_version {
            return false;
        }
        if let Some(max) = self.max_version {
            if version > max {
                return false;
            }
        }
        if let Some(bit) = self.edge_case_bit {
            if !bits.has(bit) {
                return false;
            }
        }
        true
    }

    /// Edge-case bit required by this feature, if any.
    pub fn required_edge_case(&self) -> Option<u128> {
        self.edge_case_bit
    }
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL REGISTRY
///////////////////////////////////////////////////////////////////////////////

/// Registry of supported protocol versions and features for a node.
#[derive(Debug, Clone, Default)]
pub struct ProtocolRegistry {
    pub supported_versions: Vec<ProtocolVersion>,
    pub preferred_version: ProtocolVersion,
    pub features: Vec<ProtocolFeature>,
    pub edge_case_bits: EdgeCaseBits,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            supported_versions: vec![
                ProtocolVersion::V1_0_0,
                ProtocolVersion::V1_1_0,
                ProtocolVersion::V1_2_0,
                ProtocolVersion::V2_0_0,
            ],
            preferred_version: ProtocolVersion::V2_0_0,
            features: Vec::new(),
            edge_case_bits: EdgeCaseBits::ALL_KNOWN,
        }
    }

    /// Register a supported version.
    pub fn register_version(&mut self, v: ProtocolVersion) {
        if !self.supported_versions.contains(&v) {
            self.supported_versions.push(v);
            self.supported_versions.sort();
        }
    }

    /// Whether `v` is supported.
    pub fn supports(&self, v: ProtocolVersion) -> bool {
        self.supported_versions.contains(&v)
    }

    /// Register a feature.
    pub fn register_feature(&mut self, feature: ProtocolFeature) {
        self.features.push(feature);
    }

    /// Whether feature `name` is available for `version`.
    pub fn feature_available(&self, name: &str, version: ProtocolVersion) -> bool {
        self.features
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.is_available(version, self.edge_case_bits))
            .unwrap_or(false)
    }

    /// Set edge-case bits.
    pub fn set_edge_case_bits(&mut self, bits: EdgeCaseBits) {
        self.edge_case_bits = bits;
    }

    /// Edge-case bits currently enabled.
    pub fn edge_case_bits(&self) -> EdgeCaseBits {
        self.edge_case_bits
    }

    /// Highest supported version.
    pub fn highest_supported(&self) -> Option<ProtocolVersion> {
        self.supported_versions.iter().copied().max()
    }

    /// Lowest supported version.
    pub fn lowest_supported(&self) -> Option<ProtocolVersion> {
        self.supported_versions.iter().copied().min()
    }

    /// Clear all supported versions.
    pub fn clear_versions(&mut self) {
        self.supported_versions.clear();
    }

    /// Total registered features.
    pub fn feature_count(&self) -> usize {
        self.features.len()
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPATIBILITY MATRIX
///////////////////////////////////////////////////////////////////////////////

/// Pairwise compatibility between protocol versions.
///
/// `matrix[i][j]` is true if version `i` can interoperate with version `j`.
/// Compatibility is symmetric: same major version, and the newer one accepts
/// the older one.
#[derive(Debug, Clone, Default)]
pub struct CompatibilityMatrix {
    /// Map from (a, b) encoded pair to compatibility.
    pub entries: std::collections::BTreeMap<(u64, u64), bool>,
}

impl CompatibilityMatrix {
    pub fn new() -> Self {
        Self {
            entries: std::collections::BTreeMap::new(),
        }
    }

    /// Record that `a` and `b` are compatible.
    pub fn set_compatible(&mut self, a: ProtocolVersion, b: ProtocolVersion, compatible: bool) {
        let (lo, hi) = if a.encode() <= b.encode() {
            (a.encode(), b.encode())
        } else {
            (b.encode(), a.encode())
        };
        self.entries.insert((lo, hi), compatible);
    }

    /// Whether `a` and `b` are compatible. Defaults to semantic wire rule.
    pub fn compatible(&self, a: ProtocolVersion, b: ProtocolVersion) -> bool {
        let key = if a.encode() <= b.encode() {
            (a.encode(), b.encode())
        } else {
            (b.encode(), a.encode())
        };
        self.entries.get(&key).copied().unwrap_or_else(|| {
            a.wire_compatible_with(b) || b.wire_compatible_with(a)
        })
    }

    /// Total recorded entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// All compatible pairs.
    pub fn compatible_pairs(&self) -> Vec<(ProtocolVersion, ProtocolVersion)> {
        self.entries
            .iter()
            .filter_map(|((a, b), ok)| {
                if *ok {
                    Some((ProtocolVersion::decode(*a), ProtocolVersion::decode(*b)))
                } else {
                    None
                }
            })
            .collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION GRAPH
///////////////////////////////////////////////////////////////////////////////

/// Directed edge in the version graph: an upgrade path from `from` to `to`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionEdge {
    pub from: ProtocolVersion,
    pub to: ProtocolVersion,
    pub requires_migration: bool,
    pub breaking: bool,
}

/// A DAG of protocol versions with upgrade/downgrade edges.
#[derive(Debug, Clone, Default)]
pub struct VersionGraph {
    pub edges: Vec<VersionEdge>,
    pub nodes: std::collections::BTreeSet<u64>,
}

impl VersionGraph {
    pub fn new() -> Self {
        Self {
            edges: Vec::new(),
            nodes: std::collections::BTreeSet::new(),
        }
    }

    /// Add a version node.
    pub fn add_node(&mut self, v: ProtocolVersion) {
        self.nodes.insert(v.encode());
    }

    /// Add an upgrade edge.
    pub fn add_edge(&mut self, from: ProtocolVersion, to: ProtocolVersion, breaking: bool) {
        self.add_node(from);
        self.add_node(to);
        self.edges.push(VersionEdge {
            from,
            to,
            requires_migration: breaking,
            breaking,
        });
    }

    /// Whether `v` is a known node.
    pub fn has_node(&self, v: ProtocolVersion) -> bool {
        self.nodes.contains(&v.encode())
    }

    /// Outgoing edges from `v` (direct upgrades).
    pub fn outgoing(&self, v: ProtocolVersion) -> Vec<&VersionEdge> {
        self.edges.iter().filter(|e| e.from == v).collect()
    }

    /// Incoming edges to `v` (direct downgrades).
    pub fn incoming(&self, v: ProtocolVersion) -> Vec<&VersionEdge> {
        self.edges.iter().filter(|e| e.to == v).collect()
    }

    /// Whether a direct upgrade edge exists from `from` to `to`.
    pub fn has_edge(&self, from: ProtocolVersion, to: ProtocolVersion) -> bool {
        self.edges.iter().any(|e| e.from == from && e.to == to)
    }

    /// Total edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Total nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Clear the graph.
    pub fn clear(&mut self) {
        self.edges.clear();
        self.nodes.clear();
    }

    /// Find a path from `from` to `to` via BFS (shortest upgrade path).
    pub fn find_path(&self, from: ProtocolVersion, to: ProtocolVersion) -> Option<Vec<ProtocolVersion>> {
        if from == to {
            return Some(vec![from]);
        }
        use std::collections::VecDeque;
        let mut visited = std::collections::BTreeSet::new();
        let mut queue: VecDeque<(ProtocolVersion, Vec<ProtocolVersion>)> = VecDeque::new();
        visited.insert(from.encode());
        queue.push_back((from, vec![from]));
        while let Some((current, path)) = queue.pop_front() {
            for edge in self.outgoing(current) {
                let next = edge.to;
                if next == to {
                    let mut p = path.clone();
                    p.push(next);
                    return Some(p);
                }
                if visited.insert(next.encode()) {
                    let mut p = path.clone();
                    p.push(next);
                    queue.push_back((next, p));
                }
            }
        }
        None
    }
}

///////////////////////////////////////////////////////////////////////////////
// MIGRATION PATH
///////////////////////////////////////////////////////////////////////////////

/// A single step in a migration: description + precondition check name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MigrationStep {
    pub step_id: u16,
    pub name: String,
    pub description: String,
    pub precondition: String,
    pub postcondition: String,
}

/// A migration path from `from_version` to `to_version`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MigrationPath {
    pub from_version: ProtocolVersion,
    pub to_version: ProtocolVersion,
    pub steps: Vec<MigrationStep>,
    pub estimated_duration_ms: u64,
    pub reversible: bool,
    pub risk_level: MigrationRisk,
}

/// Risk level of a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MigrationRisk {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl MigrationPath {
    pub fn new(from: ProtocolVersion, to: ProtocolVersion) -> Self {
        Self {
            from_version: from,
            to_version: to,
            steps: Vec::new(),
            estimated_duration_ms: 0,
            reversible: false,
            risk_level: MigrationRisk::None,
        }
    }

    /// Add a migration step.
    pub fn add_step(&mut self, step: MigrationStep) {
        self.steps.push(step);
    }

    /// Total steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Whether migration is a no-op (same version).
    #[inline]
    pub const fn is_noop(&self) -> bool {
        self.from_version == self.to_version
    }

    /// Set estimated duration.
    pub fn set_duration(&mut self, ms: u64) {
        self.estimated_duration_ms = ms;
    }

    /// Set reversibility.
    pub fn set_reversible(&mut self, rev: bool) {
        self.reversible = rev;
    }

    /// Set risk level.
    pub fn set_risk(&mut self, risk: MigrationRisk) {
        self.risk_level = risk;
    }

    /// Names of all steps.
    pub fn step_names(&self) -> Vec<&str> {
        self.steps.iter().map(|s| s.name.as_str()).collect()
    }
}

///////////////////////////////////////////////////////////////////////////////
// CHANGE RECORD
///////////////////////////////////////////////////////////////////////////////

/// A single version change event for a node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChangeRecord {
    pub record_id: [u8; 16],
    pub node_id: crate::types::NodeId,
    pub from_version: ProtocolVersion,
    pub to_version: ProtocolVersion,
    pub changed_at: crate::types::TimestampMillis,
    pub reason: String,
    pub successful: bool,
}

/// Version history: append-only log of change records per node.
#[derive(Debug, Clone, Default)]
pub struct VersionHistory {
    pub records: Vec<ChangeRecord>,
    pub max_records: usize,
}

impl VersionHistory {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            max_records: 10000,
        }
    }

    /// Append a change record.
    pub fn append(&mut self, record: ChangeRecord) {
        if self.records.len() >= self.max_records {
            self.records.remove(0);
        }
        self.records.push(record);
    }

    /// Total records.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Records for a given node.
    pub fn records_for_node(&self, node_id: crate::types::NodeId) -> Vec<&ChangeRecord> {
        self.records.iter().filter(|r| r.node_id == node_id).collect()
    }

    /// Latest version for a node, if any.
    pub fn latest_version_for_node(&self, node_id: crate::types::NodeId) -> Option<ProtocolVersion> {
        self.records
            .iter()
            .rev()
            .find(|r| r.node_id == node_id)
            .map(|r| r.to_version)
    }

    /// Successful changes count.
    pub fn successful_count(&self) -> u64 {
        self.records.iter().filter(|r| r.successful).count() as u64
    }

    /// Failed changes count.
    pub fn failed_count(&self) -> u64 {
        self.records.iter().filter(|r| !r.successful).count() as u64
    }

    /// Clear history.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Set max records.
    pub fn set_max_records(&mut self, max: usize) {
        self.max_records = max;
        while self.records.len() > self.max_records {
            self.records.remove(0);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION NEGOTIATION
///////////////////////////////////////////////////////////////////////////////

/// Result of negotiating a protocol version between two peers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NegotiationResult {
    pub chosen: ProtocolVersion,
    pub offered_by_local: ProtocolVersion,
    pub offered_by_remote: ProtocolVersion,
    pub compatible: bool,
    pub fallback_used: bool,
}

/// Negotiate the highest mutually supported protocol version.
#[derive(Debug, Clone, Default)]
pub struct VersionNegotiation {
    pub local_versions: Vec<ProtocolVersion>,
    pub remote_versions: Vec<ProtocolVersion>,
    pub negotiation_count: u64,
    pub fallback_count: u64,
}

impl VersionNegotiation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set local supported versions.
    pub fn set_local(&mut self, versions: Vec<ProtocolVersion>) {
        self.local_versions = versions;
    }

    /// Set remote supported versions.
    pub fn set_remote(&mut self, versions: Vec<ProtocolVersion>) {
        self.remote_versions = versions;
    }

    /// Negotiate: pick the highest version both sides support.
    pub fn negotiate(&mut self) -> NegotiationResult {
        self.negotiation_count += 1;
        let local_max = self.local_versions.iter().copied().max().unwrap_or(ProtocolVersion::ZERO);
        let remote_max = self.remote_versions.iter().copied().max().unwrap_or(ProtocolVersion::ZERO);

        // Find highest common version.
        let mut chosen = ProtocolVersion::ZERO;
        let mut compatible = false;
        for &lv in &self.local_versions {
            for &rv in &self.remote_versions {
                if lv == rv && lv > chosen {
                    chosen = lv;
                    compatible = true;
                }
            }
        }

        let fallback_used = chosen == ProtocolVersion::ZERO;
        if fallback_used {
            self.fallback_count += 1;
            chosen = local_max.min(remote_max);
        }

        NegotiationResult {
            chosen,
            offered_by_local: local_max,
            offered_by_remote: remote_max,
            compatible,
            fallback_used,
        }
    }

    /// Total negotiations performed.
    pub fn negotiation_count(&self) -> u64 {
        self.negotiation_count
    }

    /// Total fallbacks used.
    pub fn fallback_count(&self) -> u64 {
        self.fallback_count
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.local_versions.clear();
        self.remote_versions.clear();
        self.negotiation_count = 0;
        self.fallback_count = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// EDGE CASE PROFILE
///////////////////////////////////////////////////////////////////////////////

/// A named collection of edge-case bits — a reusable profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeCaseProfile {
    pub name: String,
    pub description: String,
    pub bits: EdgeCaseBits,
}

impl EdgeCaseProfile {
    pub fn new(name: impl Into<String>, bits: EdgeCaseBits) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            bits,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Whether this profile includes bit `bit`.
    pub const fn includes(&self, bit: u128) -> bool {
        self.bits.has(bit)
    }

    /// Merge another profile into this one (union of bits).
    pub fn merge(&mut self, other: &EdgeCaseProfile) {
        self.bits = self.bits.merge(other.bits);
    }

    /// Whether this profile is empty (no bits).
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.bits.raw() == 0
    }

    /// Bit count.
    pub fn bit_count(&self) -> u32 {
        self.bits.enabled_count()
    }
}

/// Registry of named edge-case profiles.
#[derive(Debug, Clone, Default)]
pub struct EdgeCaseProfileRegistry {
    pub profiles: std::collections::BTreeMap<String, EdgeCaseProfile>,
}

impl EdgeCaseProfileRegistry {
    pub fn new() -> Self {
        Self {
            profiles: std::collections::BTreeMap::new(),
        }
    }

    /// Register a profile.
    pub fn register(&mut self, profile: EdgeCaseProfile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&EdgeCaseProfile> {
        self.profiles.get(name)
    }

    /// Total profiles.
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Remove a profile.
    pub fn remove(&mut self, name: &str) -> Option<EdgeCaseProfile> {
        self.profiles.remove(name)
    }

    /// Clear all profiles.
    pub fn clear(&mut self) {
        self.profiles.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// EDGE CASE COMPILER
///////////////////////////////////////////////////////////////////////////////

/// Compiles edge-case bits into runtime enforcement descriptors.
#[derive(Debug, Clone, Default)]
pub struct EdgeCaseCompiler {
    pub compiled: std::collections::BTreeMap<u128, CompiledEdgeCase>,
    pub compile_count: u64,
}

/// A single compiled edge case.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompiledEdgeCase {
    pub bit: u128,
    pub name: String,
    pub enforcement_level: EnforcementLevel,
    pub check_code: u32,
}

/// How strictly an edge case is enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnforcementLevel {
    Permissive,
    Warn,
    Strict,
    Fatal,
}

impl EdgeCaseCompiler {
    pub fn new() -> Self {
        Self {
            compiled: std::collections::BTreeMap::new(),
            compile_count: 0,
        }
    }

    /// Compile a single edge-case bit.
    pub fn compile(&mut self, bit: u128, name: &str, level: EnforcementLevel) {
        let code = Self::hash_name(name);
        self.compiled.insert(
            bit,
            CompiledEdgeCase {
                bit,
                name: name.to_string(),
                enforcement_level: level,
                check_code: code,
            },
        );
        self.compile_count += 1;
    }

    /// Compile all bits in an `EdgeCaseBits` at Strict level.
    pub fn compile_all(&mut self, bits: EdgeCaseBits) {
        for name in bits.enabled_names() {
            let bit = Self::bit_for_name(name);
            self.compile(bit, name, EnforcementLevel::Strict);
        }
    }

    /// Get a compiled edge case by bit.
    pub fn get(&self, bit: u128) -> Option<&CompiledEdgeCase> {
        self.compiled.get(&bit)
    }

    /// Whether bit `bit` is compiled.
    pub const fn is_compiled(&self, bit: u128) -> bool {
        // placeholder: always true if map non-empty for known bits
        self.compiled.contains_key(&bit)
    }

    /// Total compiled edge cases.
    pub fn compiled_count(&self) -> usize {
        self.compiled.len()
    }

    /// Total compiles performed.
    pub fn total_compiles(&self) -> u64 {
        self.compile_count
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.compiled.clear();
        self.compile_count = 0;
    }

    fn hash_name(name: &str) -> u32 {
        let mut h: u32 = 2166136261;
        for b in name.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        h
    }

    fn bit_for_name(name: &str) -> u128 {
        match name {
            "binary_completion" => EdgeCaseBits::BINARY_COMPLETION,
            "dead_node_identity" => EdgeCaseBits::DEAD_NODE_IDENTITY,
            "idle_rotation" => EdgeCaseBits::IDLE_ROTATION,
            "no_global_master" => EdgeCaseBits::NO_GLOBAL_MASTER,
            "out_eq_in" => EdgeCaseBits::OUT_EQ_IN,
            "coordinated_deletion" => EdgeCaseBits::COORDINATED_DELETION,
            "fencing_stonith" => EdgeCaseBits::FENCING_STONITH,
            "quorum_intersection" => EdgeCaseBits::QUORUM_INTERSECTION,
            "erasure_threshold" => EdgeCaseBits::ERASURE_THRESHOLD,
            "correlated_failure" => EdgeCaseBits::CORRELATED_FAILURE,
            "hunit_compensation" => EdgeCaseBits::HUNIT_COMPENSATION,
            "regional_autonomy" => EdgeCaseBits::REGIONAL_AUTONOMY,
            "hybrid_redundancy" => EdgeCaseBits::HYBRID_REDUNDANCY,
            "operation_lifecycle" => EdgeCaseBits::OPERATION_LIFECYCLE,
            "durability_ack" => EdgeCaseBits::DURABILITY_ACK,
            "ordered_read_visibility" => EdgeCaseBits::ORDERED_READ_VISIBILITY,
            "node_roster" => EdgeCaseBits::NODE_ROSTER,
            "region_affinity" => EdgeCaseBits::REGION_AFFINITY,
            "identity_rotation" => EdgeCaseBits::IDENTITY_ROTATION,
            "ids_standard_time" => EdgeCaseBits::IDS_STANDARD_TIME,
            "daily_integrity" => EdgeCaseBits::DAILY_INTEGRITY,
            "db_retirement" => EdgeCaseBits::DB_RETIREMENT,
            "read_fallback" => EdgeCaseBits::READ_FALLBACK,
            "topology_diversity" => EdgeCaseBits::TOPOLOGY_DIVERSITY,
            "write_fanout" => EdgeCaseBits::WRITE_FANOUT,
            "custody_graph" => EdgeCaseBits::CUSTODY_GRAPH,
            "checksum_boundary" => EdgeCaseBits::CHECKSUM_BOUNDARY,
            "replica_freshness" => EdgeCaseBits::REPLICA_FRESHNESS,
            "commit_prefix" => EdgeCaseBits::COMMIT_PREFIX,
            "version_vector" => EdgeCaseBits::VERSION_VECTOR,
            "membership_epoch" => EdgeCaseBits::MEMBERSHIP_EPOCH,
            "placement_policy" => EdgeCaseBits::PLACEMENT_POLICY,
            _ => 0,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION COMPARISON HELPERS
///////////////////////////////////////////////////////////////////////////////

/// Compare two protocol versions: Less, Equal, Greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionComparison {
    Less,
    Equal,
    Greater,
}

/// Compare `a` against `b`.
#[inline]
pub const fn compare_versions(a: ProtocolVersion, b: ProtocolVersion) -> VersionComparison {
    if a.major != b.major {
        return if a.major < b.major {
            VersionComparison::Less
        } else {
            VersionComparison::Greater
        };
    }
    if a.minor != b.minor {
        return if a.minor < b.minor {
            VersionComparison::Less
        } else {
            VersionComparison::Greater
        };
    }
    if a.patch != b.patch {
        return if a.patch < b.patch {
            VersionComparison::Less
        } else {
            VersionComparison::Greater
        };
    }
    VersionComparison::Equal
}

/// Clamp `v` to `[min, max]`.
#[inline]
pub fn clamp_version(v: ProtocolVersion, min: ProtocolVersion, max: ProtocolVersion) -> ProtocolVersion {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// Midpoint version between `a` and `b` (approximate, in encoded space).
#[inline]
pub fn midpoint_version(a: ProtocolVersion, b: ProtocolVersion) -> ProtocolVersion {
    let lo = if a.encode() < b.encode() { a.encode() } else { b.encode() };
    let hi = if a.encode() > b.encode() { a.encode() } else { b.encode() };
    ProtocolVersion::decode(lo + (hi - lo) / 2)
}

///////////////////////////////////////////////////////////////////////////////
// VERSION SNAPSHOT
///////////////////////////////////////////////////////////////////////////////

/// Immutable snapshot of a node's version state at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionSnapshot {
    pub snapshot_id: [u8; 16],
    pub node_id: crate::types::NodeId,
    pub protocol_version: ProtocolVersion,
    pub edge_case_bits: EdgeCaseBits,
    pub supported_versions: Vec<ProtocolVersion>,
    pub captured_at: crate::types::TimestampMillis,
    pub metadata: Vec<(String, String)>,
}

impl VersionSnapshot {
    pub fn new(
        node_id: crate::types::NodeId,
        protocol_version: ProtocolVersion,
        edge_case_bits: EdgeCaseBits,
        supported_versions: Vec<ProtocolVersion>,
        captured_at: crate::types::TimestampMillis,
    ) -> Self {
        Self {
            snapshot_id: generate_snapshot_id(),
            node_id,
            protocol_version,
            edge_case_bits,
            supported_versions,
            captured_at,
            metadata: Vec::new(),
        }
    }

    /// Add metadata key/value.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push((key.into(), value.into()));
        self
    }

    /// Whether this snapshot enables bit `bit`.
    pub const fn has_edge_case(&self, bit: u128) -> bool {
        self.edge_case_bits.has(bit)
    }

    /// Version string.
    pub fn version_string(&self) -> String {
        self.protocol_version.to_short_string()
    }

    /// Total metadata entries.
    pub fn metadata_count(&self) -> usize {
        self.metadata.len()
    }

    /// Get metadata by key.
    pub fn metadata_get(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Clear metadata.
    pub fn clear_metadata(&mut self) {
        self.metadata.clear();
    }
}

fn generate_snapshot_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    id[0] = 0x53; // 'S'
    id[1] = 0x50; // 'P'
    id
}

///////////////////////////////////////////////////////////////////////////////
// FEATURE GATE
///////////////////////////////////////////////////////////////////////////////

/// Gates protocol features behind version and edge-case-bit requirements.
#[derive(Debug, Clone, Default)]
pub struct FeatureGate {
    pub gates: std::collections::BTreeMap<String, FeatureGateEntry>,
    pub total_checks: u64,
    pub total_granted: u64,
    pub total_denied: u64,
}

/// A single feature gate entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FeatureGateEntry {
    pub feature_name: String,
    pub min_version: ProtocolVersion,
    pub required_bits: EdgeCaseBits,
    pub description: String,
    pub enabled_by_default: bool,
}

impl FeatureGate {
    pub fn new() -> Self {
        Self {
            gates: std::collections::BTreeMap::new(),
            total_checks: 0,
            total_granted: 0,
            total_denied: 0,
        }
    }

    /// Register a feature gate.
    pub fn register(
        &mut self,
        feature_name: impl Into<String>,
        min_version: ProtocolVersion,
        required_bits: EdgeCaseBits,
        description: impl Into<String>,
        enabled_by_default: bool,
    ) {
        let name = feature_name.into();
        self.gates.insert(
            name.clone(),
            FeatureGateEntry {
                feature_name: name,
                min_version,
                required_bits,
                description: description.into(),
                enabled_by_default,
            },
        );
    }

    /// Check whether `feature` is granted for `version` and `bits`.
    pub fn check(
        &mut self,
        feature: &str,
        version: ProtocolVersion,
        bits: EdgeCaseBits,
    ) -> bool {
        self.total_checks += 1;
        let granted = self.check_granted(feature, version, bits);
        if granted {
            self.total_granted += 1;
        } else {
            self.total_denied += 1;
        }
        granted
    }

    /// Check without counting.
    pub fn check_granted(
        &self,
        feature: &str,
        version: ProtocolVersion,
        bits: EdgeCaseBits,
    ) -> bool {
        let entry = match self.gates.get(feature) {
            Some(e) => e,
            None => return false,
        };
        if version < entry.min_version {
            return false;
        }
        if !bits.raw() & entry.required_bits.raw() == entry.required_bits.raw() {
            return false;
        }
        true
    }

    /// Total checks performed.
    pub fn total_checks(&self) -> u64 {
        self.total_checks
    }

    /// Total granted.
    pub fn total_granted(&self) -> u64 {
        self.total_granted
    }

    /// Total denied.
    pub fn total_denied(&self) -> u64 {
        self.total_denied
    }

    /// Grant rate as percentage.
    pub fn grant_rate_percent(&self) -> f64 {
        if self.total_checks == 0 {
            return 100.0;
        }
        self.total_granted as f64 / self.total_checks as f64 * 100.0
    }

    /// Total registered gates.
    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.gates.clear();
        self.total_checks = 0;
        self.total_granted = 0;
        self.total_denied = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// NEGOTIATION STRATEGY
///////////////////////////////////////////////////////////////////////////////

/// Strategy used when negotiating protocol versions between peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NegotiationStrategy {
    /// Highest mutually supported version.
    HighestCommon,
    /// Lowest mutually supported version (conservative).
    LowestCommon,
    /// Prefer local preferred version if remote supports it.
    PreferLocal,
    /// Prefer remote preferred version if local supports it.
    PreferRemote,
    /// Strict: require exact version match.
    ExactMatch,
}

/// Result of a strategy-driven negotiation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrategyNegotiationResult {
    pub chosen: ProtocolVersion,
    pub strategy: NegotiationStrategy,
    pub local_offer: ProtocolVersion,
    pub remote_offer: ProtocolVersion,
    pub reason: String,
    pub iterations: u64,
}

/// Negotiates versions using a configurable strategy.
#[derive(Debug, Clone, Default)]
pub struct StrategyNegotiator {
    pub strategy: NegotiationStrategy,
    pub local_versions: Vec<ProtocolVersion>,
    pub remote_versions: Vec<ProtocolVersion>,
    pub local_preferred: ProtocolVersion,
    pub remote_preferred: ProtocolVersion,
    pub negotiation_count: u64,
}

impl StrategyNegotiator {
    pub fn new() -> Self {
        Self {
            strategy: NegotiationStrategy::HighestCommon,
            local_versions: Vec::new(),
            remote_versions: Vec::new(),
            local_preferred: ProtocolVersion::V2_0_0,
            remote_preferred: ProtocolVersion::V2_0_0,
            negotiation_count: 0,
        }
    }

    pub fn with_strategy(mut self, strategy: NegotiationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn set_local(&mut self, versions: Vec<ProtocolVersion>, preferred: ProtocolVersion) {
        self.local_versions = versions;
        self.local_preferred = preferred;
    }

    pub fn set_remote(&mut self, versions: Vec<ProtocolVersion>, preferred: ProtocolVersion) {
        self.remote_versions = versions;
        self.remote_preferred = preferred;
    }

    /// Run negotiation.
    pub fn negotiate(&mut self) -> StrategyNegotiationResult {
        self.negotiation_count += 1;
        let iterations = self.negotiation_count;
        let chosen = match self.strategy {
            NegotiationStrategy::HighestCommon => {
                let mut best = ProtocolVersion::ZERO;
                for &lv in &self.local_versions {
                    for &rv in &self.remote_versions {
                        if lv == rv && lv > best {
                            best = lv;
                        }
                    }
                }
                best
            }
            NegotiationStrategy::LowestCommon => {
                let mut best: Option<ProtocolVersion> = None;
                for &lv in &self.local_versions {
                    for &rv in &self.remote_versions {
                        if lv == rv {
                            best = Some(match best {
                                Some(b) if b < lv => b,
                                _ => lv,
                            });
                        }
                    }
                }
                best.unwrap_or(ProtocolVersion::ZERO)
            }
            NegotiationStrategy::PreferLocal => {
                if self.remote_versions.contains(&self.local_preferred) {
                    self.local_preferred
                } else {
                    ProtocolVersion::ZERO
                }
            }
            NegotiationStrategy::PreferRemote => {
                if self.local_versions.contains(&self.remote_preferred) {
                    self.remote_preferred
                } else {
                    ProtocolVersion::ZERO
                }
            }
            NegotiationStrategy::ExactMatch => {
                if self.local_preferred == self.remote_preferred
                    && self.local_versions.contains(&self.local_preferred)
                    && self.remote_versions.contains(&self.remote_preferred)
                {
                    self.local_preferred
                } else {
                    ProtocolVersion::ZERO
                }
            }
        };
        let reason = if chosen == ProtocolVersion::ZERO {
            "no common version".into()
        } else {
            format!(
                "negotiated {} via {:?}",
                chosen.to_short_string(),
                self.strategy
            )
        };
        StrategyNegotiationResult {
            chosen,
            strategy: self.strategy,
            local_offer: self.local_preferred,
            remote_offer: self.remote_preferred,
            reason,
            iterations,
        }
    }

    /// Total negotiations.
    pub fn negotiation_count(&self) -> u64 {
        self.negotiation_count
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.local_versions.clear();
        self.remote_versions.clear();
        self.negotiation_count = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// EDGE CASE TRANSITION
///////////////////////////////////////////////////////////////////////////////

/// A transition of edge-case bits from one state to another.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeCaseTransition {
    pub transition_id: [u8; 16],
    pub from_bits: EdgeCaseBits,
    pub to_bits: EdgeCaseBits,
    pub enabled_bits: EdgeCaseBits,
    pub disabled_bits: EdgeCaseBits,
    pub transitioned_at: crate::types::TimestampMillis,
    pub reason: String,
}

impl EdgeCaseTransition {
    pub fn new(
        from_bits: EdgeCaseBits,
        to_bits: EdgeCaseBits,
        transitioned_at: crate::types::TimestampMillis,
        reason: impl Into<String>,
    ) -> Self {
        let enabled = EdgeCaseBits(to_bits.raw() & !from_bits.raw());
        let disabled = EdgeCaseBits(from_bits.raw() & !to_bits.raw());
        Self {
            transition_id: generate_transition_id(),
            from_bits,
            to_bits,
            enabled_bits: enabled,
            disabled_bits: disabled,
            transitioned_at,
            reason: reason.into(),
        }
    }

    /// Whether any bit was enabled.
    pub const fn has_enabling(&self) -> bool {
        self.enabled_bits.any()
    }

    /// Whether any bit was disabled.
    pub const fn has_disabling(&self) -> bool {
        self.disabled_bits.any()
    }

    /// Number of bits changed.
    pub fn changed_count(&self) -> u32 {
        self.enabled_bits.enabled_count() + self.disabled_bits.enabled_count()
    }

    /// Transition summary string.
    pub fn summary(&self) -> String {
        format!(
            "enabled={} disabled={} total={}",
            self.enabled_bits.enabled_count(),
            self.disabled_bits.enabled_count(),
            self.changed_count()
        )
    }
}

fn generate_transition_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    id[0] = 0x54; // 'T'
    id[1] = 0x52; // 'R'
    id
}

///////////////////////////////////////////////////////////////////////////////
// PROTOCOL CAPABILITY
///////////////////////////////////////////////////////////////////////////////

/// A protocol capability: a named, versioned, edge-case-gated ability.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolCapability {
    pub name: String,
    pub description: String,
    pub introduced_in: ProtocolVersion,
    pub deprecated_in: Option<ProtocolVersion>,
    pub removed_in: Option<ProtocolVersion>,
    pub required_edge_case: Option<u128>,
    pub optional: bool,
}

impl ProtocolCapability {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        introduced_in: ProtocolVersion,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            introduced_in,
            deprecated_in: None,
            removed_in: None,
            required_edge_case: None,
            optional: false,
        }
    }

    pub fn with_edge_case(mut self, bit: u128) -> Self {
        self.required_edge_case = Some(bit);
        self
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn deprecated_in(mut self, version: ProtocolVersion) -> Self {
        self.deprecated_in = Some(version);
        self
    }

    /// Whether this capability is available at `version` with `bits`.
    pub fn available_at(&self, version: ProtocolVersion, bits: EdgeCaseBits) -> bool {
        if version < self.introduced_in {
            return false;
        }
        if let Some(removed) = self.removed_in {
            if version >= removed {
                return false;
            }
        }
        if let Some(bit) = self.required_edge_case {
            if !bits.has(bit) {
                return false;
            }
        }
        true
    }

    /// Whether deprecated at `version`.
    pub fn deprecated_at(&self, version: ProtocolVersion) -> bool {
        self.deprecated_in.map_or(false, |d| version >= d)
    }

    /// Whether removed at `version`.
    pub fn removed_at(&self, version: ProtocolVersion) -> bool {
        self.removed_in.map_or(false, |r| version >= r)
    }
}

/// Registry of protocol capabilities.
#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    pub capabilities: std::collections::BTreeMap<String, ProtocolCapability>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            capabilities: std::collections::BTreeMap::new(),
        }
    }

    /// Register a capability.
    pub fn register(&mut self, cap: ProtocolCapability) {
        self.capabilities.insert(cap.name.clone(), cap);
    }

    /// Get a capability by name.
    pub fn get(&self, name: &str) -> Option<&ProtocolCapability> {
        self.capabilities.get(name)
    }

    /// Whether capability `name` is available at `version` with `bits`.
    pub fn available(&self, name: &str, version: ProtocolVersion, bits: EdgeCaseBits) -> bool {
        self.capabilities
            .get(name)
            .map(|c| c.available_at(version, bits))
            .unwrap_or(false)
    }

    /// Total capabilities.
    pub fn capability_count(&self) -> usize {
        self.capabilities.len()
    }

    /// All capabilities available at `version` with `bits`.
    pub fn available_capabilities(
        &self,
        version: ProtocolVersion,
        bits: EdgeCaseBits,
    ) -> Vec<&ProtocolCapability> {
        self.capabilities
            .values()
            .filter(|c| c.available_at(version, bits))
            .collect()
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.capabilities.clear();
    }
}

///////////////////////////////////////////////////////////////////////////////
// VERSION POLICY
///////////////////////////////////////////////////////////////////////////////

/// Policy governing which protocol versions a node will accept.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionPolicy {
    pub min_version: ProtocolVersion,
    pub max_version: Option<ProtocolVersion>,
    pub require_edge_case_bits: EdgeCaseBits,
    pub forbid_edge_case_bits: EdgeCaseBits,
    pub allow_prerelease: bool,
    pub strict_major_match: bool,
}

impl VersionPolicy {
    pub fn new() -> Self {
        Self {
            min_version: ProtocolVersion::V1_0_0,
            max_version: None,
            require_edge_case_bits: EdgeCaseBits::NONE,
            forbid_edge_case_bits: EdgeCaseBits::NONE,
            allow_prerelease: false,
            strict_major_match: false,
        }
    }

    /// Strict policy: only exact major version, all known edge cases required.
    pub fn strict() -> Self {
        Self {
            min_version: ProtocolVersion::V2_0_0,
            max_version: Some(ProtocolVersion::V2_0_0),
            require_edge_case_bits: EdgeCaseBits::ALL_KNOWN,
            forbid_edge_case_bits: EdgeCaseBits::NONE,
            allow_prerelease: false,
            strict_major_match: true,
        }
    }

    /// Permissive policy: any version >= 1.0.0.
    pub fn permissive() -> Self {
        Self::new()
    }

    /// Whether `version` with `bits` is accepted by this policy.
    pub fn accepts(&self, version: ProtocolVersion, bits: EdgeCaseBits) -> bool {
        if version < self.min_version {
            return false;
        }
        if let Some(max) = self.max_version {
            if version > max {
                return false;
            }
        }
        if self.strict_major_match && version.major != self.min_version.major {
            return false;
        }
        if !version.is_zero() && !version.supports_edge_case_bits() && !self.allow_prerelease {
            // versions before edge-case bits require opt-in
            if version < ProtocolVersion::V1_1_0 {
                return false;
            }
        }
        for name in self.require_edge_case_bits.enabled_names() {
            let bit = Self::bit_for_name(name);
            if !bits.has(bit) {
                return false;
            }
        }
        for name in self.forbid_edge_case_bits.enabled_names() {
            let bit = Self::bit_for_name(name);
            if bits.has(bit) {
                return false;
            }
        }
        true
    }

    /// Set min version.
    pub fn set_min(&mut self, v: ProtocolVersion) {
        self.min_version = v;
    }

    /// Set max version.
    pub fn set_max(&mut self, v: Option<ProtocolVersion>) {
        self.max_version = v;
    }

    /// Require edge-case bits.
    pub fn require_bits(&mut self, bits: EdgeCaseBits) {
        self.require_edge_case_bits = bits;
    }

    /// Forbid edge-case bits.
    pub fn forbid_bits(&mut self, bits: EdgeCaseBits) {
        self.forbid_edge_case_bits = bits;
    }

    /// Bit for a named edge case.
    fn bit_for_name(name: &str) -> u128 {
        match name {
            "binary_completion" => EdgeCaseBits::BINARY_COMPLETION,
            "dead_node_identity" => EdgeCaseBits::DEAD_NODE_IDENTITY,
            "idle_rotation" => EdgeCaseBits::IDLE_ROTATION,
            "no_global_master" => EdgeCaseBits::NO_GLOBAL_MASTER,
            "out_eq_in" => EdgeCaseBits::OUT_EQ_IN,
            "coordinated_deletion" => EdgeCaseBits::COORDINATED_DELETION,
            "fencing_stonith" => EdgeCaseBits::FENCING_STONITH,
            "quorum_intersection" => EdgeCaseBits::QUORUM_INTERSECTION,
            "erasure_threshold" => EdgeCaseBits::ERASURE_THRESHOLD,
            "correlated_failure" => EdgeCaseBits::CORRELATED_FAILURE,
            "hunit_compensation" => EdgeCaseBits::HUNIT_COMPENSATION,
            "regional_autonomy" => EdgeCaseBits::REGIONAL_AUTONOMY,
            "hybrid_redundancy" => EdgeCaseBits::HYBRID_REDUNDANCY,
            "operation_lifecycle" => EdgeCaseBits::OPERATION_LIFECYCLE,
            "durability_ack" => EdgeCaseBits::DURABILITY_ACK,
            "ordered_read_visibility" => EdgeCaseBits::ORDERED_READ_VISIBILITY,
            "node_roster" => EdgeCaseBits::NODE_ROSTER,
            "region_affinity" => EdgeCaseBits::REGION_AFFINITY,
            "identity_rotation" => EdgeCaseBits::IDENTITY_ROTATION,
            "ids_standard_time" => EdgeCaseBits::IDS_STANDARD_TIME,
            "daily_integrity" => EdgeCaseBits::DAILY_INTEGRITY,
            "db_retirement" => EdgeCaseBits::DB_RETIREMENT,
            "read_fallback" => EdgeCaseBits::READ_FALLBACK,
            "topology_diversity" => EdgeCaseBits::TOPOLOGY_DIVERSITY,
            "write_fanout" => EdgeCaseBits::WRITE_FANOUT,
            "custody_graph" => EdgeCaseBits::CUSTODY_GRAPH,
            "checksum_boundary" => EdgeCaseBits::CHECKSUM_BOUNDARY,
            "replica_freshness" => EdgeCaseBits::REPLICA_FRESHNESS,
            "commit_prefix" => EdgeCaseBits::COMMIT_PREFIX,
            "version_vector" => EdgeCaseBits::VERSION_VECTOR,
            "membership_epoch" => EdgeCaseBits::MEMBERSHIP_EPOCH,
            "placement_policy" => EdgeCaseBits::PLACEMENT_POLICY,
            _ => 0,
        }
    }

    /// Policy summary string.
    pub fn summary(&self) -> String {
        format!(
            "min={} max={:?} require={} forbid={} strict={}",
            self.min_version.to_short_string(),
            self.max_version.map(|v| v.to_short_string()),
            self.require_edge_case_bits.enabled_count(),
            self.forbid_edge_case_bits.enabled_count(),
            self.strict_major_match
        )
    }
}

///////////////////////////////////////////////////////////////////////////////
// COMPATIBILITY REPORT
///////////////////////////////////////////////////////////////////////////////

/// A report on compatibility between two nodes' version states.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompatibilityReport {
    pub local_version: ProtocolVersion,
    pub remote_version: ProtocolVersion,
    pub compatible: bool,
    pub common_version: Option<ProtocolVersion>,
    pub missing_bits: EdgeCaseBits,
    pub extra_bits: EdgeCaseBits,
    pub recommendation: String,
}

/// Analyzes compatibility between local and remote version states.
#[derive(Debug, Clone, Default)]
pub struct CompatibilityAnalyzer {
    pub analyses: u64,
    pub compatible_count: u64,
    pub incompatible_count: u64,
}

impl CompatibilityAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze compatibility.
    pub fn analyze(
        &mut self,
        local_version: ProtocolVersion,
        local_bits: EdgeCaseBits,
        remote_version: ProtocolVersion,
        remote_bits: EdgeCaseBits,
    ) -> CompatibilityReport {
        self.analyses += 1;
        let common = if local_version == remote_version {
            Some(local_version)
        } else if local_version.wire_compatible_with(remote_version) {
            Some(remote_version)
        } else if remote_version.wire_compatible_with(local_version) {
            Some(local_version)
        } else {
            None
        };
        let compatible = common.is_some();
        if compatible {
            self.compatible_count += 1;
        } else {
            self.incompatible_count += 1;
        }
        let missing = EdgeCaseBits(remote_bits.raw() & !local_bits.raw());
        let extra = EdgeCaseBits(local_bits.raw() & !remote_bits.raw());
        let recommendation = if compatible {
            "versions compatible; proceed with negotiation".into()
        } else {
            "upgrade to a common minor version before connecting".into()
        };
        CompatibilityReport {
            local_version,
            remote_version,
            compatible,
            common_version: common,
            missing_bits: missing,
            extra_bits: extra,
            recommendation,
        }
    }

    /// Total analyses.
    pub fn analyses(&self) -> u64 {
        self.analyses
    }

    /// Compatible count.
    pub fn compatible_count(&self) -> u64 {
        self.compatible_count
    }

    /// Incompatible count.
    pub fn incompatible_count(&self) -> u64 {
        self.incompatible_count
    }

    /// Compatibility rate as percentage.
    pub fn compatibility_rate_percent(&self) -> f64 {
        if self.analyses == 0 {
            return 100.0;
        }
        self.compatible_count as f64 / self.analyses as f64 * 100.0
    }

    /// Clear.
    pub fn clear(&mut self) {
        self.analyses = 0;
        self.compatible_count = 0;
        self.incompatible_count = 0;
    }
}

///////////////////////////////////////////////////////////////////////////////
// TESTS
///////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    ///////////////////////////////////////////////////////////////////////////////
    // PROTOCOL VERSION TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn protocol_version_zero() {
        let v = ProtocolVersion::ZERO;
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert!(v.is_zero());
        assert!(!v.is_stable_release());
    }

    #[test]
    fn protocol_version_v1_0_0() {
        let v = ProtocolVersion::V1_0_0;
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert!(!v.is_zero());
        assert!(v.is_stable_release());
    }

    #[test]
    fn protocol_version_v2_0_0() {
        let v = ProtocolVersion::V2_0_0;
        assert_eq!(v.to_short_string(), "v2");
        assert!(v.is_stable_release());
    }

    #[test]
    fn protocol_version_new() {
        let v = ProtocolVersion::new(3, 5, 9);
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 5);
        assert_eq!(v.patch, 9);
    }

    #[test]
    fn protocol_version_compare() {
        assert!(ProtocolVersion::V1_0_0 < ProtocolVersion::V1_1_0);
        assert!(ProtocolVersion::V1_1_0 < ProtocolVersion::V2_0_0);
        assert!(ProtocolVersion::V2_0_0 > ProtocolVersion::V1_2_0);
        assert_eq!(ProtocolVersion::V1_0_0, ProtocolVersion::V1_0_0);
    }

    #[test]
    fn protocol_version_bump_major() {
        let v = ProtocolVersion::new(1, 2, 3);
        let b = v.bump_major();
        assert_eq!(b.major, 2);
        assert_eq!(b.minor, 0);
        assert_eq!(b.patch, 0);
    }

    #[test]
    fn protocol_version_bump_minor() {
        let v = ProtocolVersion::new(1, 2, 3);
        let b = v.bump_minor();
        assert_eq!(b.major, 1);
        assert_eq!(b.minor, 3);
        assert_eq!(b.patch, 0);
    }

    #[test]
    fn protocol_version_bump_patch() {
        let v = ProtocolVersion::new(1, 2, 3);
        let b = v.bump_patch();
        assert_eq!(b.major, 1);
        assert_eq!(b.minor, 2);
        assert_eq!(b.patch, 4);
    }

    #[test]
    fn protocol_version_wire_compatible() {
        assert!(ProtocolVersion::V2_0_0.wire_compatible_with(ProtocolVersion::V1_1_0));
        assert!(!ProtocolVersion::V1_0_0.wire_compatible_with(ProtocolVersion::V2_0_0));
    }

    #[test]
    fn protocol_version_encode_decode() {
        let v = ProtocolVersion::new(2, 5, 100);
        let encoded = v.encode();
        let decoded = ProtocolVersion::decode(encoded);
        assert_eq!(v, decoded);
    }

    #[test]
    fn protocol_version_to_string() {
        let v = ProtocolVersion::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn protocol_version_to_short_string() {
        let v = ProtocolVersion::new(2, 0, 0);
        assert_eq!(v.to_short_string(), "v2");
        let v = ProtocolVersion::new(1, 5, 0);
        assert_eq!(v.to_short_string(), "v1.5");
    }

    #[test]
    fn protocol_version_supports_edge_case_bits() {
        assert!(ProtocolVersion::V2_0_0.supports_edge_case_bits());
        assert!(ProtocolVersion::V1_1_0.supports_edge_case_bits());
        assert!(!ProtocolVersion::V1_0_0.supports_edge_case_bits());
    }

    #[test]
    fn protocol_version_default() {
        let v = ProtocolVersion::default();
        assert_eq!(v, ProtocolVersion::V2_0_0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // SEMANTIC VERSION PARSER TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn semantic_version_parser_parse_basic() {
        let p = SemanticVersionParser::new();
        let v = p.parse("1.2.3").unwrap();
        assert_eq!(v, ProtocolVersion::new(1, 2, 3));
    }

    #[test]
    fn semantic_version_parser_parse_with_v_prefix() {
        let p = SemanticVersionParser::new();
        let v = p.parse("v2.0.0").unwrap();
        assert_eq!(v, ProtocolVersion::V2_0_0);
    }

    #[test]
    fn semantic_version_parser_parse_empty_err() {
        let p = SemanticVersionParser::new();
        let err = p.parse("").unwrap_err();
        assert_eq!(err.reason, VersionParseReason::EmptyInput);
    }

    #[test]
    fn semantic_version_parser_parse_too_many_components() {
        let p = SemanticVersionParser::strict();
        let err = p.parse("1.2.3.4").unwrap_err();
        assert_eq!(err.reason, VersionParseReason::TooManyComponents);
    }

    #[test]
    fn semantic_version_parser_parse_non_numeric() {
        let p = SemanticVersionParser::strict();
        let err = p.parse("1.a.3").unwrap_err();
        assert_eq!(err.reason, VersionParseReason::NonNumericComponent);
    }

    #[test]
    fn semantic_version_parser_parse_leading_zero_strict() {
        let p = SemanticVersionParser::strict();
        let err = p.parse("01.2.3").unwrap_err();
        assert_eq!(err.reason, VersionParseReason::LeadingZeroRejected);
    }

    #[test]
    fn semantic_version_parser_lenient_leading_zero_ok() {
        let p = SemanticVersionParser::lenient();
        let v = p.parse("01.02.03").unwrap();
        assert_eq!(v, ProtocolVersion::new(1, 2, 3));
    }

    #[test]
    fn semantic_version_parser_lenient_omits_patch() {
        let p = SemanticVersionParser::lenient();
        let v = p.parse("1.2").unwrap();
        assert_eq!(v, ProtocolVersion::new(1, 2, 0));
    }

    #[test]
    fn semantic_version_parser_parse_many() {
        let p = SemanticVersionParser::new();
        let vs = p.parse_many(&["1.0.0", "2.0.0", "3.0.0"]).unwrap();
        assert_eq!(vs.len(), 3);
        assert_eq!(vs[0], ProtocolVersion::V1_0_0);
        assert_eq!(vs[2], ProtocolVersion::new(3, 0, 0));
    }

    #[test]
    fn semantic_version_parser_parse_many_collect() {
        let p = SemanticVersionParser::new();
        let (ok, err) = p.parse_many_collect(&["1.0.0", "bad", "2.0.0"]);
        assert_eq!(ok.len(), 2);
        assert_eq!(err.len(), 1);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // VERSION RANGE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn version_range_contains() {
        let r = VersionRange::new(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0);
        assert!(r.contains(ProtocolVersion::V1_1_0));
        assert!(!r.contains(ProtocolVersion::V2_0_0.bump_major()));
    }

    #[test]
    fn version_range_exact() {
        let r = VersionRange::exact(ProtocolVersion::V1_0_0);
        assert!(r.contains(ProtocolVersion::V1_0_0));
        assert!(!r.contains(ProtocolVersion::V1_1_0));
        assert!(r.is_point());
    }

    #[test]
    fn version_range_at_least() {
        let r = VersionRange::at_least(ProtocolVersion::V1_1_0);
        assert!(r.contains(ProtocolVersion::V1_1_0));
        assert!(r.contains(ProtocolVersion::V2_0_0));
        assert!(!r.contains(ProtocolVersion::V1_0_0));
    }

    #[test]
    fn version_range_overlaps() {
        let a = VersionRange::new(ProtocolVersion::V1_0_0, ProtocolVersion::V1_5_0);
        let b = VersionRange::new(ProtocolVersion::V1_3_0, ProtocolVersion::V2_0_0);
        assert!(a.overlaps(&b));
        let c = VersionRange::new(ProtocolVersion::V2_1_0, ProtocolVersion::V3_0_0);
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn version_range_intersect() {
        let a = VersionRange::new(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0);
        let b = VersionRange::new(ProtocolVersion::V1_5_0, ProtocolVersion::V3_0_0);
        let i = a.intersect(&b).unwrap();
        assert_eq!(i.min, ProtocolVersion::V1_5_0);
        assert_eq!(i.max, ProtocolVersion::V2_0_0);
    }

    #[test]
    fn version_range_intersect_none() {
        let a = VersionRange::new(ProtocolVersion::V1_0_0, ProtocolVersion::V1_1_0);
        let b = VersionRange::new(ProtocolVersion::V2_0_0, ProtocolVersion::V3_0_0);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn version_range_union() {
        let a = VersionRange::new(ProtocolVersion::V1_0_0, ProtocolVersion::V1_5_0);
        let b = VersionRange::new(ProtocolVersion::V2_0_0, ProtocolVersion::V2_5_0);
        let u = a.union(&b);
        assert_eq!(u.min, ProtocolVersion::V1_0_0);
        assert_eq!(u.max, ProtocolVersion::V2_5_0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // EDGE CASE BITS TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn edge_case_bits_has() {
        let bits = EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION);
        assert!(bits.has(EdgeCaseBits::BINARY_COMPLETION));
        assert!(!bits.has(EdgeCaseBits::DEAD_NODE_IDENTITY));
    }

    #[test]
    fn edge_case_bits_enable_disable() {
        let mut bits = EdgeCaseBits::NONE;
        bits.enable(EdgeCaseBits::IDLE_ROTATION);
        assert!(bits.has(EdgeCaseBits::IDLE_ROTATION));
        bits.disable(EdgeCaseBits::IDLE_ROTATION);
        assert!(!bits.has(EdgeCaseBits::IDLE_ROTATION));
    }

    #[test]
    fn edge_case_bits_toggle() {
        let mut bits = EdgeCaseBits::NONE;
        bits.toggle(EdgeCaseBits::OUT_EQ_IN);
        assert!(bits.has(EdgeCaseBits::OUT_EQ_IN));
        bits.toggle(EdgeCaseBits::OUT_EQ_IN);
        assert!(!bits.has(EdgeCaseBits::OUT_EQ_IN));
    }

    #[test]
    fn edge_case_bits_enabled_names() {
        let bits = EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::OUT_EQ_IN);
        let names = bits.enabled_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"binary_completion"));
        assert!(names.contains(&"out_eq_in"));
    }

    #[test]
    fn edge_case_bits_description() {
        let desc = EdgeCaseBits::description(EdgeCaseBits::COORDINATED_DELETION);
        assert!(desc.contains("coordinated_deletion"));
    }

    #[test]
    fn edge_case_bits_enabled_count() {
        let bits = EdgeCaseBits::ALL_KNOWN;
        assert_eq!(bits.enabled_count(), EdgeCaseBits::KNOWN_EDGE_CASE_COUNT as u32);
    }

    #[test]
    fn edge_case_bits_merge_common_difference() {
        let a = EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::OUT_EQ_IN);
        let b = EdgeCaseBits::from_raw(EdgeCaseBits::OUT_EQ_IN | EdgeCaseBits::COORDINATED_DELETION);
        let merged = a.merge(&b);
        assert!(merged.has(EdgeCaseBits::COORDINATED_DELETION));
        let common = a.common(&b);
        assert!(common.has(EdgeCaseBits::OUT_EQ_IN));
        let diff = a.difference(&b);
        assert!(diff.has(EdgeCaseBits::BINARY_COMPLETION));
        assert!(!diff.has(EdgeCaseBits::OUT_EQ_IN));
    }

    #[test]
    fn edge_case_bits_only_future_bits() {
        let bits = EdgeCaseBits::from_raw(1 << 63);
        assert!(bits.only_future_bits());
        assert!(bits.has_future_bits());
    }

    #[test]
    fn edge_case_bits_all_known() {
        let bits = EdgeCaseBits::ALL_KNOWN;
        assert!(bits.all_known());
    }

    #[test]
    fn edge_case_bits_default_none() {
        let bits = EdgeCaseBits::default();
        assert_eq!(bits.raw(), 0);
        assert!(!bits.any());
    }

    ///////////////////////////////////////////////////////////////////////////////
    // PROTOCOL FEATURE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn protocol_feature_new_and_enable() {
        let f = ProtocolFeature::new("feature_a", "desc", ProtocolVersion::V1_0_0).enable();
        assert!(f.enabled);
        assert_eq!(f.min_version, ProtocolVersion::V1_0_0);
    }

    #[test]
    fn protocol_feature_with_edge_case() {
        let f = ProtocolFeature::new("f", "d", ProtocolVersion::V1_0_0)
            .with_edge_case(EdgeCaseBits::WRITE_FANOUT);
        assert_eq!(f.required_edge_case, Some(EdgeCaseBits::WRITE_FANOUT));
    }

    #[test]
    fn protocol_feature_is_available() {
        let f = ProtocolFeature::new("f", "d", ProtocolVersion::V1_0_0).enable();
        assert!(f.is_available(ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
        assert!(!f.is_available(ProtocolVersion::V1_0_0, EdgeCaseBits::NONE));
    }

    #[test]
    fn protocol_feature_not_available_with_edge_case() {
        let f = ProtocolFeature::new("f", "d", ProtocolVersion::V1_0_0)
            .with_edge_case(EdgeCaseBits::FENCING_STONITH)
            .enable();
        assert!(!f.is_available(ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
        assert!(f.is_available(
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::from_raw(EdgeCaseBits::FENCING_STONITH)
        ));
    }

    #[test]
    fn protocol_feature_max_version() {
        let f = ProtocolFeature::new("f", "d", ProtocolVersion::V1_0_0)
            .with_max_version(ProtocolVersion::V1_5_0)
            .enable();
        assert!(f.is_available(ProtocolVersion::V1_2_0, EdgeCaseBits::NONE));
        assert!(!f.is_available(ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // PROTOCOL REGISTRY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn protocol_registry_default() {
        let r = ProtocolRegistry::new();
        assert!(r.supports(ProtocolVersion::V2_0_0));
        assert_eq!(r.feature_count(), 0);
    }

    #[test]
    fn protocol_registry_register_version() {
        let mut r = ProtocolRegistry::new();
        r.register_version(ProtocolVersion::new(5, 0, 0));
        assert!(r.supports(ProtocolVersion::new(5, 0, 0)));
    }

    #[test]
    fn protocol_registry_highest_lowest() {
        let r = ProtocolRegistry::new();
        assert_eq!(r.highest_supported(), Some(ProtocolVersion::V2_0_0));
        assert_eq!(r.lowest_supported(), Some(ProtocolVersion::V1_0_0));
    }

    #[test]
    fn protocol_registry_register_feature() {
        let mut r = ProtocolRegistry::new();
        r.register_feature(
            ProtocolFeature::new("feat", "d", ProtocolVersion::V1_0_0).enable(),
        );
        assert!(r.feature_available("feat", ProtocolVersion::V2_0_0));
        assert!(!r.feature_available("missing", ProtocolVersion::V2_0_0));
    }

    #[test]
    fn protocol_registry_set_edge_case_bits() {
        let mut r = ProtocolRegistry::new();
        r.set_edge_case_bits(EdgeCaseBits::NONE);
        assert!(!r.edge_case_bits().any());
    }

    ///////////////////////////////////////////////////////////////////////////////
    // COMPATIBILITY MATRIX TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn compatibility_matrix_set_and_check() {
        let mut m = CompatibilityMatrix::new();
        m.set_compatible(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0, true);
        assert!(m.compatible(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0));
        assert_eq!(m.entry_count(), 1);
    }

    #[test]
    fn compatibility_matrix_default_rule() {
        let m = CompatibilityMatrix::new();
        assert!(m.compatible(ProtocolVersion::V2_0_0, ProtocolVersion::V1_1_0));
    }

    #[test]
    fn compatibility_matrix_clear() {
        let mut m = CompatibilityMatrix::new();
        m.set_compatible(ProtocolVersion::V1_0_0, ProtocolVersion::V1_1_0, true);
        m.clear();
        assert_eq!(m.entry_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // VERSION GRAPH TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn version_graph_add_node_and_edge() {
        let mut g = VersionGraph::new();
        g.add_node(ProtocolVersion::V1_0_0);
        g.add_node(ProtocolVersion::V2_0_0);
        g.add_edge(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0, true);
        assert!(g.has_node(ProtocolVersion::V1_0_0));
        assert!(g.has_edge(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0));
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn version_graph_find_path() {
        let mut g = VersionGraph::new();
        g.add_edge(ProtocolVersion::V1_0_0, ProtocolVersion::V1_1_0, false);
        g.add_edge(ProtocolVersion::V1_1_0, ProtocolVersion::V2_0_0, false);
        let path = g.find_path(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], ProtocolVersion::V1_0_0);
        assert_eq!(path[2], ProtocolVersion::V2_0_0);
    }

    #[test]
    fn version_graph_find_path_none() {
        let mut g = VersionGraph::new();
        g.add_node(ProtocolVersion::V1_0_0);
        g.add_node(ProtocolVersion::V2_0_0);
        assert!(g.find_path(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0).is_none());
    }

    #[test]
    fn version_graph_clear() {
        let mut g = VersionGraph::new();
        g.add_edge(ProtocolVersion::V1_0_0, ProtocolVersion::V1_1_0, false);
        g.clear();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // MIGRATION PATH TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn migration_path_new_and_steps() {
        let mut m = MigrationPath::new(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0);
        m.add_step(MigrationStep {
            step_id: 1,
            name: "step1".into(),
            description: "d".into(),
            precondition: "pre".into(),
            postcondition: "post".into(),
        });
        assert_eq!(m.step_count(), 1);
        assert!(!m.is_noop());
    }

    #[test]
    fn migration_path_noop() {
        let m = MigrationPath::new(ProtocolVersion::V1_0_0, ProtocolVersion::V1_0_0);
        assert!(m.is_noop());
    }

    #[test]
    fn migration_path_duration_risk() {
        let mut m = MigrationPath::new(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0);
        m.set_duration(5000);
        m.set_reversible(true);
        m.set_risk(MigrationRisk::Medium);
        assert_eq!(m.estimated_duration_ms, 5000);
        assert!(m.reversible);
    }

    #[test]
    fn migration_path_step_names() {
        let mut m = MigrationPath::new(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0);
        m.add_step(MigrationStep {
            step_id: 1,
            name: "alpha".into(),
            description: "d".into(),
            precondition: "pre".into(),
            postcondition: "post".into(),
        });
        m.add_step(MigrationStep {
            step_id: 2,
            name: "beta".into(),
            description: "d".into(),
            precondition: "pre".into(),
            postcondition: "post".into(),
        });
        let names = m.step_names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // CHANGE RECORD / VERSION HISTORY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn version_history_append_and_records() {
        let mut h = VersionHistory::new();
        let rec = ChangeRecord {
            record_id: [1u8; 16],
            node_id: NodeId([0u8; 32]),
            from_version: ProtocolVersion::V1_0_0,
            to_version: ProtocolVersion::V2_0_0,
            changed_at: TimestampMillis(1000),
            reason: "upgrade".into(),
            successful: true,
        };
        h.append(rec);
        assert_eq!(h.record_count(), 1);
        assert_eq!(h.successful_count(), 1);
        assert_eq!(h.failed_count(), 0);
    }

    #[test]
    fn version_history_latest_for_node() {
        let mut h = VersionHistory::new();
        let node = NodeId([42u8; 32]);
        h.append(ChangeRecord {
            record_id: [1u8; 16],
            node_id: node,
            from_version: ProtocolVersion::V1_0_0,
            to_version: ProtocolVersion::V1_1_0,
            changed_at: TimestampMillis(100),
            reason: "a".into(),
            successful: true,
        });
        h.append(ChangeRecord {
            record_id: [2u8; 16],
            node_id: node,
            from_version: ProtocolVersion::V1_1_0,
            to_version: ProtocolVersion::V2_0_0,
            changed_at: TimestampMillis(200),
            reason: "b".into(),
            successful: true,
        });
        assert_eq!(
            h.latest_version_for_node(node),
            Some(ProtocolVersion::V2_0_0)
        );
    }

    #[test]
    fn version_history_clear() {
        let mut h = VersionHistory::new();
        h.append(ChangeRecord {
            record_id: [1u8; 16],
            node_id: NodeId::ZERO,
            from_version: ProtocolVersion::V1_0_0,
            to_version: ProtocolVersion::V2_0_0,
            changed_at: TimestampMillis(100),
            reason: "a".into(),
            successful: true,
        });
        h.clear();
        assert_eq!(h.record_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // VERSION NEGOTIATION TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn version_negotiation_highest_common() {
        let mut n = VersionNegotiation::new();
        n.set_local(vec![ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0]);
        n.set_remote(vec![ProtocolVersion::V1_1_0, ProtocolVersion::V2_0_0]);
        let result = n.negotiate();
        assert_eq!(result.chosen, ProtocolVersion::V2_0_0);
        assert!(result.compatible);
    }

    #[test]
    fn version_negotiation_no_common() {
        let mut n = VersionNegotiation::new();
        n.set_local(vec![ProtocolVersion::V1_0_0]);
        n.set_remote(vec![ProtocolVersion::V2_0_0]);
        let result = n.negotiate();
        assert!(result.fallback_used);
        assert!(!result.compatible);
    }

    #[test]
    fn version_negotiation_counts() {
        let mut n = VersionNegotiation::new();
        n.set_local(vec![ProtocolVersion::V1_0_0]);
        n.set_remote(vec![ProtocolVersion::V1_0_0]);
        n.negotiate();
        n.negotiate();
        assert_eq!(n.negotiation_count(), 2);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // EDGE CASE PROFILE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn edge_case_profile_new() {
        let p = EdgeCaseProfile::new("strict", EdgeCaseBits::ALL_KNOWN);
        assert_eq!(p.name, "strict");
        assert_eq!(p.bit_count(), EdgeCaseBits::KNOWN_EDGE_CASE_COUNT as u32);
    }

    #[test]
    fn edge_case_profile_includes() {
        let p = EdgeCaseProfile::new("p", EdgeCaseBits::from_raw(EdgeCaseBits::OUT_EQ_IN));
        assert!(p.includes(EdgeCaseBits::OUT_EQ_IN));
        assert!(!p.includes(EdgeCaseBits::COORDINATED_DELETION));
    }

    #[test]
    fn edge_case_profile_merge() {
        let mut a = EdgeCaseProfile::new("a", EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION));
        let b = EdgeCaseProfile::new("b", EdgeCaseBits::from_raw(EdgeCaseBits::OUT_EQ_IN));
        a.merge(&b);
        assert!(a.includes(EdgeCaseBits::BINARY_COMPLETION));
        assert!(a.includes(EdgeCaseBits::OUT_EQ_IN));
    }

    #[test]
    fn edge_case_profile_registry() {
        let mut reg = EdgeCaseProfileRegistry::new();
        reg.register(EdgeCaseProfile::new("p1", EdgeCaseBits::NONE));
        assert_eq!(reg.profile_count(), 1);
        assert!(reg.get("p1").is_some());
        assert!(reg.get("missing").is_none());
        reg.remove("p1");
        assert_eq!(reg.profile_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // EDGE CASE COMPILER TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn edge_case_compiler_compile() {
        let mut c = EdgeCaseCompiler::new();
        c.compile(EdgeCaseBits::BINARY_COMPLETION, "binary_completion", EnforcementLevel::Strict);
        assert!(c.is_compiled(EdgeCaseBits::BINARY_COMPLETION));
        assert_eq!(c.compiled_count(), 1);
    }

    #[test]
    fn edge_case_compiler_compile_all() {
        let mut c = EdgeCaseCompiler::new();
        c.compile_all(EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::OUT_EQ_IN));
        assert_eq!(c.compiled_count(), 2);
    }

    #[test]
    fn edge_case_compiler_clear() {
        let mut c = EdgeCaseCompiler::new();
        c.compile(EdgeCaseBits::BINARY_COMPLETION, "bc", EnforcementLevel::Warn);
        c.clear();
        assert_eq!(c.compiled_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // VERSION SNAPSHOT TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn version_snapshot_new() {
        let s = VersionSnapshot::new(
            NodeId([1u8; 32]),
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::ALL_KNOWN,
            vec![ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0],
            TimestampMillis(5000),
        );
        assert!(s.has_edge_case(EdgeCaseBits::COORDINATED_DELETION));
        assert_eq!(s.version_string(), "v2");
        assert_eq!(s.metadata_count(), 0);
    }

    #[test]
    fn version_snapshot_with_metadata() {
        let s = VersionSnapshot::new(
            NodeId::ZERO,
            ProtocolVersion::V1_0_0,
            EdgeCaseBits::NONE,
            vec![],
            TimestampMillis(100),
        )
        .with_metadata("key", "value");
        assert_eq!(s.metadata_count(), 1);
        assert_eq!(s.metadata_get("key"), Some("value"));
        assert_eq!(s.metadata_get("missing"), None);
    }

    #[test]
    fn version_snapshot_clear_metadata() {
        let mut s = VersionSnapshot::new(
            NodeId::ZERO,
            ProtocolVersion::V1_0_0,
            EdgeCaseBits::NONE,
            vec![],
            TimestampMillis(100),
        );
        s = s.with_metadata("k", "v");
        s.clear_metadata();
        assert_eq!(s.metadata_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // FEATURE GATE TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn feature_gate_register_and_check() {
        let mut g = FeatureGate::new();
        g.register(
            "feat_x",
            ProtocolVersion::V1_1_0,
            EdgeCaseBits::from_raw(EdgeCaseBits::COORDINATED_DELETION),
            "desc",
            false,
        );
        assert!(g.check("feat_x", ProtocolVersion::V2_0_0, EdgeCaseBits::from_raw(EdgeCaseBits::COORDINATED_DELETION)));
        assert!(!g.check("feat_x", ProtocolVersion::V1_0_0, EdgeCaseBits::from_raw(EdgeCaseBits::COORDINATED_DELETION)));
    }

    #[test]
    fn feature_gate_stats() {
        let mut g = FeatureGate::new();
        g.register("f", ProtocolVersion::V1_0_0, EdgeCaseBits::NONE, "d", true);
        g.check("f", ProtocolVersion::V2_0_0, EdgeCaseBits::NONE);
        g.check("missing", ProtocolVersion::V2_0_0, EdgeCaseBits::NONE);
        assert_eq!(g.total_checks(), 2);
        assert_eq!(g.total_granted(), 1);
        assert_eq!(g.total_denied(), 1);
    }

    #[test]
    fn feature_gate_clear() {
        let mut g = FeatureGate::new();
        g.register("f", ProtocolVersion::V1_0_0, EdgeCaseBits::NONE, "d", true);
        g.check("f", ProtocolVersion::V2_0_0, EdgeCaseBits::NONE);
        g.clear();
        assert_eq!(g.total_checks(), 0);
        assert_eq!(g.gate_count(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // STRATEGY NEGOTIATOR TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn strategy_negotiator_highest_common() {
        let mut n = StrategyNegotiator::new();
        n.set_local(vec![ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        n.set_remote(vec![ProtocolVersion::V1_1_0, ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        let r = n.negotiate();
        assert_eq!(r.chosen, ProtocolVersion::V2_0_0);
        assert_eq!(r.strategy, NegotiationStrategy::HighestCommon);
    }

    #[test]
    fn strategy_negotiator_lowest_common() {
        let mut n = StrategyNegotiator::with_strategy(NegotiationStrategy::LowestCommon);
        n.set_local(vec![ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        n.set_remote(vec![ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        let r = n.negotiate();
        assert_eq!(r.chosen, ProtocolVersion::V1_0_0);
    }

    #[test]
    fn strategy_negotiator_exact_match() {
        let mut n = StrategyNegotiator::with_strategy(NegotiationStrategy::ExactMatch);
        n.set_local(vec![ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        n.set_remote(vec![ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        let r = n.negotiate();
        assert_eq!(r.chosen, ProtocolVersion::V2_0_0);
    }

    #[test]
    fn strategy_negotiator_no_match() {
        let mut n = StrategyNegotiator::with_strategy(NegotiationStrategy::ExactMatch);
        n.set_local(vec![ProtocolVersion::V1_0_0], ProtocolVersion::V1_0_0);
        n.set_remote(vec![ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        let r = n.negotiate();
        assert_eq!(r.chosen, ProtocolVersion::ZERO);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // EDGE CASE TRANSITION TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn edge_case_transition_new() {
        let from = EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION);
        let to = EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::OUT_EQ_IN);
        let t = EdgeCaseTransition::new(from, to, TimestampMillis(1000), "add out_eq_in".into());
        assert!(t.has_enabling());
        assert!(!t.has_disabling());
        assert_eq!(t.changed_count(), 1);
    }

    #[test]
    fn edge_case_transition_disabling() {
        let from = EdgeCaseBits::from_raw(EdgeCaseBits::OUT_EQ_IN);
        let to = EdgeCaseBits::NONE;
        let t = EdgeCaseTransition::new(from, to, TimestampMillis(1000), "remove".into());
        assert!(!t.has_enabling());
        assert!(t.has_disabling());
        assert_eq!(t.changed_count(), 1);
    }

    #[test]
    fn edge_case_transition_summary() {
        let t = EdgeCaseTransition::new(
            EdgeCaseBits::NONE,
            EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::OUT_EQ_IN),
            TimestampMillis(1000),
            "two added".into(),
        );
        let s = t.summary();
        assert!(s.contains("enabled=2"));
        assert!(s.contains("disabled=0"));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // PROTOCOL CAPABILITY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn protocol_capability_available_at() {
        let c = ProtocolCapability::new("cap", "d", ProtocolVersion::V1_1_0)
            .with_edge_case(EdgeCaseBits::COORDINATED_DELETION);
        assert!(!c.available_at(ProtocolVersion::V1_0_0, EdgeCaseBits::ALL_KNOWN));
        assert!(!c.available_at(ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
        assert!(c.available_at(ProtocolVersion::V2_0_0, EdgeCaseBits::from_raw(EdgeCaseBits::COORDINATED_DELETION)));
    }

    #[test]
    fn protocol_capability_deprecated_removed() {
        let c = ProtocolCapability::new("cap", "d", ProtocolVersion::V1_0_0)
            .deprecated_in(ProtocolVersion::V2_0_0)
            .removed_in(ProtocolVersion::V3_0_0);
        assert!(c.deprecated_at(ProtocolVersion::V2_0_0));
        assert!(c.removed_at(ProtocolVersion::V3_0_0));
        assert!(!c.available_at(ProtocolVersion::V3_0_0, EdgeCaseBits::ALL_KNOWN));
    }

    #[test]
    fn capability_registry_available() {
        let mut reg = CapabilityRegistry::new();
        reg.register(ProtocolCapability::new("cap1", "d", ProtocolVersion::V1_0_0));
        assert!(reg.available("cap1", ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
        assert!(!reg.available("missing", ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
    }

    #[test]
    fn capability_registry_available_capabilities() {
        let mut reg = CapabilityRegistry::new();
        reg.register(ProtocolCapability::new("a", "d", ProtocolVersion::V1_0_0));
        reg.register(ProtocolCapability::new("b", "d", ProtocolVersion::V2_0_0));
        let avail = reg.available_capabilities(ProtocolVersion::V1_5_0, EdgeCaseBits::NONE);
        assert_eq!(avail.len(), 1);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // VERSION POLICY TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn version_policy_accepts() {
        let p = VersionPolicy::new();
        assert!(p.accepts(ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
        assert!(!p.accepts(ProtocolVersion::V1_0_0, EdgeCaseBits::NONE));
    }

    #[test]
    fn version_policy_strict() {
        let p = VersionPolicy::strict();
        assert!(p.accepts(ProtocolVersion::V2_0_0, EdgeCaseBits::ALL_KNOWN));
        assert!(!p.accepts(ProtocolVersion::V2_0_0, EdgeCaseBits::NONE));
        assert!(!p.accepts(ProtocolVersion::V1_0_0, EdgeCaseBits::ALL_KNOWN));
    }

    #[test]
    fn version_policy_permissive() {
        let p = VersionPolicy::permissive();
        assert!(p.accepts(ProtocolVersion::V1_0_0, EdgeCaseBits::NONE));
    }

    #[test]
    fn version_policy_summary() {
        let p = VersionPolicy::new();
        let s = p.summary();
        assert!(s.contains("min="));
    }

    ///////////////////////////////////////////////////////////////////////////////
    // COMPATIBILITY ANALYZER TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn compatibility_analyzer_analyze_compatible() {
        let mut a = CompatibilityAnalyzer::new();
        let r = a.analyze(
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::ALL_KNOWN,
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::ALL_KNOWN,
        );
        assert!(r.compatible);
        assert_eq!(a.compatible_count(), 1);
        assert_eq!(a.incompatible_count(), 0);
    }

    #[test]
    fn compatibility_analyzer_analyze_incompatible() {
        let mut a = CompatibilityAnalyzer::new();
        let r = a.analyze(
            ProtocolVersion::V1_0_0,
            EdgeCaseBits::NONE,
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::ALL_KNOWN,
        );
        assert!(!r.compatible);
        assert_eq!(a.incompatible_count(), 1);
    }

    #[test]
    fn compatibility_analyzer_missing_extra_bits() {
        let mut a = CompatibilityAnalyzer::new();
        let r = a.analyze(
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION),
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::OUT_EQ_IN),
        );
        assert!(r.missing_bits.has(EdgeCaseBits::OUT_EQ_IN));
    }

    #[test]
    fn compatibility_analyzer_rate() {
        let mut a = CompatibilityAnalyzer::new();
        a.analyze(ProtocolVersion::V2_0_0, EdgeCaseBits::ALL_KNOWN, ProtocolVersion::V2_0_0, EdgeCaseBits::ALL_KNOWN);
        a.analyze(ProtocolVersion::V1_0_0, EdgeCaseBits::NONE, ProtocolVersion::V2_0_0, EdgeCaseBits::ALL_KNOWN);
        assert!((a.compatibility_rate_percent() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compatibility_analyzer_clear() {
        let mut a = CompatibilityAnalyzer::new();
        a.analyze(ProtocolVersion::V2_0_0, EdgeCaseBits::ALL_KNOWN, ProtocolVersion::V2_0_0, EdgeCaseBits::ALL_KNOWN);
        a.clear();
        assert_eq!(a.analyses(), 0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // VERSION COMPARISON HELPERS TESTS
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn compare_versions_test() {
        assert_eq!(
            compare_versions(ProtocolVersion::V1_0_0, ProtocolVersion::V2_0_0),
            VersionComparison::Less
        );
        assert_eq!(
            compare_versions(ProtocolVersion::V2_0_0, ProtocolVersion::V1_0_0),
            VersionComparison::Greater
        );
        assert_eq!(
            compare_versions(ProtocolVersion::V2_0_0, ProtocolVersion::V2_0_0),
            VersionComparison::Equal
        );
    }

    #[test]
    fn clamp_version_test() {
        let clamped = clamp_version(
            ProtocolVersion::new(5, 0, 0),
            ProtocolVersion::V1_0_0,
            ProtocolVersion::V3_0_0,
        );
        assert_eq!(clamped, ProtocolVersion::V3_0_0);
    }

    #[test]
    fn midpoint_version_test() {
        let mid = midpoint_version(ProtocolVersion::V1_0_0, ProtocolVersion::V3_0_0);
        assert!(mid >= ProtocolVersion::V1_0_0 && mid <= ProtocolVersion::V3_0_0);
    }

    ///////////////////////////////////////////////////////////////////////////////
    // INTEGRATION TESTS (cross-subsystem)
    ///////////////////////////////////////////////////////////////////////////////

    #[test]
    fn integration_registry_feature_gate() {
        let mut reg = ProtocolRegistry::new();
        reg.register_feature(
            ProtocolFeature::new("fanout", "write fanout", ProtocolVersion::V1_2_0)
                .with_edge_case(EdgeCaseBits::WRITE_FANOUT)
                .enable(),
        );
        assert!(reg.feature_available(
            "fanout",
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::from_raw(EdgeCaseBits::WRITE_FANOUT)
        ));
    }

    #[test]
    fn integration_negotiation_with_policy() {
        let policy = VersionPolicy::strict();
        let mut n = VersionNegotiation::new();
        n.set_local(vec![ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        n.set_remote(vec![ProtocolVersion::V2_0_0], ProtocolVersion::V2_0_0);
        let result = n.negotiate();
        assert!(policy.accepts(result.chosen, EdgeCaseBits::ALL_KNOWN));
    }

    #[test]
    fn integration_history_with_transition() {
        let mut h = VersionHistory::new();
        let from = EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION);
        let to = EdgeCaseBits::from_raw(EdgeCaseBits::BINARY_COMPLETION | EdgeCaseBits::OUT_EQ_IN);
        let t = EdgeCaseTransition::new(from, to, TimestampMillis(1000), "add".into());
        assert_eq!(t.changed_count(), 1);
        let _ = h;
    }

    #[test]
    fn integration_snapshot_with_compiler() {
        let mut c = EdgeCaseCompiler::new();
        c.compile_all(EdgeCaseBits::ALL_KNOWN);
        let s = VersionSnapshot::new(
            NodeId::ZERO,
            ProtocolVersion::V2_0_0,
            EdgeCaseBits::ALL_KNOWN,
            vec![ProtocolVersion::V2_0_0],
            TimestampMillis(1000),
        );
        assert_eq!(c.compiled_count(), EdgeCaseBits::KNOWN_EDGE_CASE_COUNT as usize);
        assert!(s.has_edge_case(EdgeCaseBits::PLACEMENT_POLICY));
        }
}