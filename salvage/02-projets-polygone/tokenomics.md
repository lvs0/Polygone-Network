# Polygone Tokenomics Model
## POLY Token Design & Economic Analysis

**Document Version:** 1.0 | **Last Updated:** May 2025 | **Status:** Draft

---

## 1. Executive Summary

POLY is the native utility token of the Polygone protocol. It serves three primary functions: (1) securing the network through staking, (2) enabling protocol governance, and (3) facilitating access to premium protocol services. The token economy is designed with a fixed supply of 1 billion tokens, gradual emission through staking rewards, and revenue backing from real protocol and enterprise software services.

**Key parameters:**
- Total Supply: 1,000,000,000 POLY (fixed)
- Initial circulating: ~80M (8%) at TGE
- Annual staking reward: 3% base, up to 80% Year 1 (subsidized)
- Target staking ratio: 60% of circulating supply
- Year 3 target: positive protocol revenue

---

## 2. Token Details

| Parameter | Value |
|---|---|
| Token Name | Polygone |
| Ticker | POLY |
| Standard | ERC-20 (Phase 1) / Native (post-mainnet) |
| Total Supply | 1,000,000,000 |
| decimals | 18 |
| Initial Price (TGE) | $0.020 |
| Market Cap (TGE) | $2,000,000 |

---

## 3. Token Allocation

### 3.1 Allocation Table

| Category | Tokens | % | Lock Schedule | Vesting |
|---|---|---|---|---|
| Ecosystem | 400,000,000 | 40.0% | Variable | Grant-specific |
| Team | 150,000,000 | 15.0% | 1yr cliff | 4yr linear |
| Investors | 150,000,000 | 15.0% | None | 2yr linear |
| Treasury | 150,000,000 | 15.0% | 1yr lock | DAO-controlled |
| Public Sale | 150,000,000 | 15.0% | TGE (50M) + 2yr vest | TGE + 8 quarterly |

**Total:** 1,000,000,000 POLY

### 3.2 Allocation Rationale

**Ecosystem (40%):** A large ecosystem allocation is critical for:
- Liquidity provision (10% of total supply for DEX LP)
- Developer grants (5% of total supply)
- Protocol rewards / staking incentives (15%)
- Bug bounty and security research (2%)
- Community airdrops (3%)
- Ecosystem partnerships (5%)

**Team (15%):** Aligned with long-term protocol health. 4-year vesting with 1-year cliff ensures team members earn tokens only after sustained contribution.

**Investors (15%):** 2-year linear vesting aligns with typical VC fund cycles while preventing immediate dump pressure at TGE.

**Treasury (15%):** DAO-controlled reserve for unexpected expenses, strategic investments in complementary protocols, and operational continuity. Requires 60% governance approval for disbursements.

**Public (15%):** Distribution across TGE (5%) and 8 quarterly tranches (10% total, vesting 2 years). Maximizes decentralization and prevents whale concentration.

### 3.3 Vesting Schedules

```
Team tokens (150M):
- Months 0–11: 0 (cliff)
- Months 12–60: 30M/year (linear)
- Fully unlocked: Month 60

Investor tokens (150M):
- Months 0–2: 0 (TGE lock)
- Months 3–27: 6.25M/month (linear)
- Fully unlocked: Month 27

Public tokens (150M):
- Tranche 1 (50M): TGE release
- Tranches 2–9 (100M): 12.5M/quarter over 2 years
```

---

## 4. Token Utility

### 4.1 Staking (Security)

POLY holders stake tokens to participate in network validation. Staked tokens secure the Polygone protocol and earn rewards.

**Staking parameters:**
- Minimum stake: 100 POLY
- Unbonding period: 21 days
- Slashing conditions: equivocation, downtime (detailed in protocol spec)

**Reward distribution:**
- Validator rewards: 90% to stakers, 10% to treasury
- Reward source: protocol fees (not new token issuance after Year 2)

### 4.2 Governance

POLY is the governance token of the Polygone DAO. Token holders vote on:
- Protocol upgrades and parameter changes
- Treasury disbursements (grants, partnerships, operations)
- Fee structure modifications
- Council member elections

**Governance parameters:**
- Quorum: 10% of circulating supply must participate
- Approval threshold: 60% for treasury, 51% for parameter changes
- Time lock: 48 hours for protocol upgrades after approval

### 4.3 Fee Payment

POLY is used to pay for on-chain services:
- Encrypted storage operations: 0.001 POLY per KB
- Messaging/transfer: 0.01 POLY per message
- API premium access: 1,000 POLY/month for enterprise tier

### 4.4 Premium Access

Staking thresholds unlock additional features:
- 10,000 POLY staked: Rate limit bypass (10x)
- 50,000 POLY staked: Priority support + audit reports
- 100,000 POLY staked: Custom algorithm configurations

---

## 5. Emission Model

### 5.1 Staking Rewards

**Year 1:** 80M POLY (8% of supply)
- Purpose: Bootstrap network security and incentivize early participants
- Source: Token reserves (not protocol revenue)
- Rationale: High rewards necessary to achieve target staking ratio before real revenue is available

**Year 2:** 40M POLY (4% of supply)
- Source: 50% token reserves, 50% protocol fees
- Reduction: 50% from Year 1 as protocol revenue grows

**Year 3:** 20M POLY (2% of supply)
- Source: Protocol fees (100%)
- Target: Staking rewards fully backed by real revenue by Year 3

**Year 4+:** <1% inflation
- Protocol fee revenue funds staking rewards
- Minimal new token issuance

### 5.2 Reward Projection Table

| Year | Staking Rewards | % of Supply | Source | Staking APY (est.) |
|---|---|---|---|---|
| Y1 | 80M POLY | 8.0% | Token reserves | 80% |
| Y2 | 40M POLY | 4.0% | 50% reserves / 50% revenue | 35% |
| Y3 | 20M POLY | 2.0% | Protocol revenue | 12% |
| Y4 | 10M POLY | 1.0% | Protocol revenue | 5% |
| Y5+ | <5M/year | <0.5% | Protocol revenue | 3–5% |

**APY calculation assumes:**
- 60M POLY staked (60% of circulating supply)
- Token price appreciation (modeled at $0.02 at TGE, $0.20 by Year 3)

### 5.3 Burn Mechanism

POLY implements a burn mechanism for fee payment:
- 50% of protocol fees are burned
- 50% go to treasury
- If protocol fees exceed staking rewards, net supply decreases (deflationary)

---

## 6. Revenue Model

### 6.1 Revenue Streams

**1. Protocol Fees (on-chain)**
- Fee per encrypted storage operation: 0.001 POLY
- Fee per message/transfer: 0.01 POLY
- Estimated Year 3 daily operations: 1M+
- Estimated Year 3 protocol revenue: 15M POLY/year (~$3M at $0.20/POLY)

**2. Enterprise SDK Licensing (Company Revenue → Protocol)**
- Mid-market: $50K/year
- Enterprise: $100K–$500K/year
- 2026 target: 30 customers → $2.5M ARR

**3. Managed Cloud Services**
- Per-GB pricing: $0.10/GB/month
- Pay-per-query: $0.0001/query
- Year 3 target: $5M ARR

### 6.2 Revenue Backed Tokenomics

The POLY token benefits from real revenue streams:
- Protocol fees create direct token utility (burn + staking)
- Enterprise revenue funds protocol development (more product → more usage → more fees)
- Managed cloud creates recurring revenue supporting token value

**Model assumptions:**
- Year 3 token price: $0.20 (20x from TGE, conservative)
- Year 3 protocol fees: $3M equivalent in POLY
- Year 3 market cap (fully diluted): $200M
- Revenue / Market Cap ratio: 1.5% — healthy for a utility token

---

## 7. Liquidity & Exchange Strategy

### 7.1 TGE Allocation

| Category | Tokens | % of Total | Notes |
|---|---|---|---|
| Public Sale (DEX) | 30,000,000 | 3% | Immediate liquidity on Uniswap/equivalent |
| Airdrops | 20,000,000 | 2% | Community, early users, bug bounty |
| LP Incentives (Yr 1) | 30,000,000 | 3% | Incentivized liquidity provision |
| **TGE Circulating** | **80,000,000** | **8%** | |

### 7.2 Exchange Strategy

**Tier 1:** Binance, Coinbase (target 12 months post-TGE if milestones met)
**Tier 2:** Kraken, OKX, Bybit (TGE + 30 days)
**DEX:** Uniswap V3 (primary), Curve (if stablecoin pairs dominant)

### 7.3 Liquidity Provision

- Initial LP: $500K (from raise)
- 12-month LP incentives: 30M POLY
- Target liquidity depth: $2M+ on primary trading pair
- Lock LP tokens for 12 months (team allocation excluded)

---

## 8. Risk Factors & Mitigation

### 8.1 Token Inflation Risk

**Risk:** High staking rewards create selling pressure, depressing token price.

**Mitigation:**
- Year 1 rewards (80M) are subsidized from reserves, not freshly printed
- Staking ratio target reduces sell pressure (staked tokens are locked)
- Burn mechanism creates deflationary pressure as protocol usage grows

### 8.2 Low Staking Participation

**Risk:** If staking ratio is below 40%, network security is insufficient.

**Mitigation:**
- Year 1 APY of 80% is competitive with DeFi alternatives
- Governance weight incentivizes holding
- Slashing disincentivizes validator misbehavior

### 8.3 Token Price Volatility

**Risk:** Crypto market cycles can cause POLY to trade far below fundamental value.

**Mitigation:**
- Long vesting schedules prevent instant supply expansion
- Real revenue (SDK licensing, cloud services) provides non-speculative demand
- Revenue-backed tokenomics give POLY intrinsic value floor

### 8.4 Regulatory Risk

**Risk:** Token classified as security in major markets, limiting liquidity.

**Mitigation:**
- Legal opinion that POLY is a utility token (not a security)
- Structured compliance with SEC Howey test analysis
- Favorable jurisdiction for token issuance

---

## 9. Valuation Framework

### 9.1 Token Valuation Model

POLY valuation is derived from three components:

**1. Utility Value (50% weight)**
- Fee payment demand × average token velocity
- Year 5 estimated: $5M annual utility value → $10M network value at 2× P/S

**2. Governance Value (30% weight)**
- DAO treasury value × governance premium
- Year 5 estimated: $20M treasury → $6M governance value

**3. Speculative Premium (20% weight)**
- Crypto market cycle × growth premium
- Directly correlated with market conditions

**Year 5 Base Valuation:** $150M–$300M (2–4× revenue multiple)

### 9.2 Comparative Analysis

| Protocol | Token | FDV (Year 5 est.) | Revenue Multiple |
|---|---|---|---|
| Chainlink | LINK | $20B | 50× |
| The Graph | GRT | $8B | 30× |
| Livepeer | LVP | $1.5B | 20× |
| **Polygone (base)** | **POLY** | **$200M** | **15×** |

*Polygone uses a conservative 15× revenue multiple vs. comparable crypto infrastructure protocols. Conservative due to regulatory uncertainty and youth of project.*

---

## 10. Summary Table

| Metric | Year 1 | Year 2 | Year 3 | Year 4 | Year 5 |
|---|---|---|---|---|---|
| ARR ($) | $0 | $500K | $2M | $8M | $20M |
| Protocol Fees (POLY) | 0 | 5M | 15M | 30M | 60M |
| Staking APY | 80% | 35% | 12% | 7% | 5% |
| Circulating Supply | 150M | 300M | 450M | 550M | 600M |
| Staked Supply | 90M | 180M | 270M | 330M | 360M |
| Token Price | $0.02 | $0.05 | $0.20 | $0.40 | $0.60 |
| Market Cap | $3M | $15M | $90M | $220M | $360M |
| Staking Rewards | 80M | 40M | 20M | 10M | 5M |

---

## 11. Implementation Notes

### Phase 1: ERC-20 on Ethereum
- Deploy standard ERC-20 contract
- Implement timelock and vesting contracts
- Launch on Uniswap V3 for initial liquidity

### Phase 2: Bridge to Native Chain
- Mainnet launch with native token
- Cross-chain bridge for ETH-L1 settlement
- Governance deployment on mainnet

### Phase 3: Full DAO Governance
- Transfer protocol upgrade authority to DAO
- Decentralize block production
- Remove admin keys entirely

---

*This tokenomics model is for informational and planning purposes. Actual token economics may differ based on market conditions, governance decisions, and protocol development. This document does not constitute financial advice.*