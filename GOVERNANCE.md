# Governance

## Vision

Knotcoin is designed to be deployed once and left alone. The protocol belongs to whoever runs it. Most rules are eternal by design—changing them would create a different system. A narrow governance mechanism exists only to handle genuine operational needs and cryptographic emergencies.

The hardest question in protocol design is not what the rules should be, but who gets to change them. Our answer: almost nobody, almost never.

---

## Eternal Rules

These properties are fixed at genesis. Every node enforces them independently and rejects any block that violates them, regardless of chain length or hashpower.

**Cryptographic Foundation**
- Dilithium3 for signatures (NIST FIPS 204, Security Level 3)
- SHA-512 for address derivation (256-bit quantum preimage resistance)
- SHA3-256 for proof-of-work and Merkle trees (128-bit quantum security)
- PONC puzzle specification (2 MB scratchpad, 512 mixing rounds)

Why permanent: Changing signature schemes invalidates all existing addresses. The blockchain would need to restart from scratch. Hash function diversity (SHA-2 vs SHA-3) provides defense in depth—a weakness in one construction does not imply weakness in the other.

**Emission Schedule**
- Phase 1: 0.1 → 1.0 KOT linear ramp (blocks 0–262,800)
- Phase 2: 1.0 KOT constant (blocks 262,801–525,600)
- Phase 3: 1.0 / log₂(adjusted + 2) decay (blocks 525,601+)

Why permanent: Changing emission alters the social contract. Early miners accepted specific inflation rates. Retroactive changes would be theft.

**Referral Structure**
- 5% maximum bonus, protocol-minted
- Single-hop only (no multi-level)
- Active mining window: 2,880 blocks (~48 hours)

Why permanent: Single-hop prevents MLM exploitation. Multi-level structures create pyramid dynamics and governance manipulation vectors.

**Block Parameters**
- Target block time: 60 seconds
- Minimum block size: 50 KB
- Minimum transaction fee: 1 knot

Why permanent: These define the network's fundamental characteristics. Changing block time requires recalculating all difficulty adjustments historically.

**Fair Launch**
- No pre-mine, no ICO, no admin allocation
- Creator mines genesis under identical rules

Why permanent: Fair launch is a core principle. Cannot be retroactively altered.

---

## Tunable Parameters

A small set of operational parameters can be adjusted by governance vote. Changes require simple majority (>50% of governance weight) and activate after 1,000-block delay.

**Block Size Ceiling**  
Range: 50 KB (floor) to 500 KB (ceiling)  
Use case: If Layer 1 needs more throughput before Layer 2 deployment. Floor prevents spam attacks.

**PONC Scratchpad Size**  
Range: 2 MB to 256 MB  
Use case: If ASICs become economically viable, increase scratchpad. At 256 MB, on-chip SRAM becomes infeasible on current manufacturing nodes.

**State Channel Parameters**  
Dispute window length (currently 288 blocks), maximum channel lifetime, priority transaction slots.  
Use case: Adjust based on network congestion and attack patterns.

**Recommended Fee Levels**  
Not enforced (minimum is always 1 knot). Provides guidance during congestion.

**Peer Discovery Configuration**  
Maximum connections (64 inbound, 8 outbound), handshake timeout (10 seconds).  
Use case: Adjust based on network size and bandwidth availability.

---

## The Hash Escape Hatch

One exception to eternal immutability: hash functions can be upgraded.

**Requirements:**
- 75% supermajority of governance weight
- Mandatory 18-month transition period
- Both old and new functions accepted during transition

**Rationale:** "Forever" is a very long time. Hash function security is ultimately empirical. SHA-512 and SHA3-256 are strong today. The system should be able to adapt its foundation without requiring anyone's permission or presence.

This covers planned, orderly deprecation of a weakening algorithm. A sudden practical break—allowing forgery within hours—cannot be addressed through governance. It would require an emergency hard fork coordinated by the developer and node operator community, the same response any proof-of-work blockchain would require.

---

## Who Governs

**Eligibility:** Any node that has produced at least 100 blocks.

**Weight Calculation:**
```
weight = 100 + 100 × log₁₀(contributions)
```
Where contributions = blocks mined OR miners referred in the last year.

**Examples:**
- 100 blocks: 300 bps (3.0%)
- 1,000 blocks: 400 bps (4.0%)
- 10,000 blocks: 500 bps (5.0%)

**Hard Cap:** 1,000 bps (10%) per entity, regardless of hashrate.

**Why this works:** Even if a massive datacenter controls 40% of network hashrate, their governance influence is capped at 10%. At least six independent entities must agree to change anything. This prevents industrial mining operations from monopolizing protocol decisions.

Weight decays if mining stops. A miner active three years ago but stopped has zero weight today. A miner who started last month and has been producing blocks consistently has real influence. Governance by those who actually show up, in proportion to how much they show up.

---

## Voting Mechanism

**1. Proposal Submission**  
Create governance transaction specifying action, target value, and description.

**2. Voting Period**  
Proposals remain open for 10,080 blocks (~1 week). Miners signal support by including governance_data in transactions. Tally tracked on-chain with vote deduplication.

**3. Threshold**  
Simple majority: >50% of governance weight (5,000 bps)  
Supermajority (hash upgrade): >75% (7,500 bps)

**4. Activation Delay**  
1,000 blocks (~16.7 hours) after passing. Gives nodes time to upgrade if needed.

---

## Security Properties

**Centralization Prevention**  
10% weight cap per entity. Requires 6+ independent entities to change anything. Weight decays if mining stops.

**Vote Manipulation Prevention**  
On-chain vote deduplication. Weight based on actual mining (not coin holdings). 1,000-block activation delay prevents surprise attacks.

**Emergency Response**  
Hash function upgrade path (75% + 18 months). Community can fork if governance fails. No single point of control.

---

## Summary

**Cannot Be Changed:**  
Dilithium3 signatures, SHA-512/SHA3-256 hashing, PONC algorithm, emission schedule (all phases), referral structure (5%, single-hop), block time (60s), minimum block size (50 KB), minimum fee (1 knot), no pre-mine.

**Can Be Changed:**  
Block size ceiling (50-500 KB), PONC scratchpad size (2-256 MB), state channel parameters, recommended fees, connection limits.

The system is designed to be deployed and left alone. Governance exists for operational adaptation and cryptographic emergencies, not continuous tinkering. The protocol belongs to whoever runs it.

---

Version 1.0.0  
February 2026
