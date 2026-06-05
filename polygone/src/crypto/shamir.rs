//! Shamir secret sharing for POLYGONE: threshold=4, n=7.
//!
//! The serialized encrypted payload is split into 7 fragments.
//! Any 4 are sufficient to reconstruct. No subset of 3 or fewer
//! reveals *any* information about the secret.

use sharks::{Sharks, Share};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::{PolygoneError, Result};

// ── Types ─────────────────────────────────────────────────────────────────────

/// A unique fragment identifier [1..=n].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FragmentId(pub u8);

/// A single Shamir fragment.
#[derive(Clone, Serialize, Deserialize)]
pub struct Fragment {
    pub id: FragmentId,
    pub data: Vec<u8>,
}

// ── Split ─────────────────────────────────────────────────────────────────────

/// Split `secret` into `n` fragments with threshold `t`.
///
/// `t` fragments are necessary AND sufficient to reconstruct.
/// `t - 1` fragments reveal nothing.
///
/// Constraints: 2 ≤ t ≤ n ≤ 255.
pub fn split(secret: &[u8], threshold: u8, n: u8) -> Result<Vec<Fragment>> {
    if threshold < 2 {
        return Err(PolygoneError::ShamirSplit("threshold must be ≥ 2".into()));
    }
    if n < threshold {
        return Err(PolygoneError::ShamirSplit("n must be ≥ threshold".into()));
    }

    let sharks = Sharks(threshold);
    let dealer = sharks.dealer_rng(secret, &mut OsRng);

    let fragments: Vec<Fragment> = dealer
        .take(n as usize)
        .enumerate()
        .map(|(i, share)| Fragment {
            id: FragmentId(i as u8 + 1),
            data: Vec::from(&share),
        })
        .collect();

    Ok(fragments)
}

// ── Reconstruct ───────────────────────────────────────────────────────────────

/// Reconstruct the secret from at least `threshold` fragments.
///
/// Returns `Err(ShamirReconstruct)` if fewer than `threshold` fragments
/// are provided, or if the shares are inconsistent/corrupted.
pub fn reconstruct(fragments: &[Fragment], threshold: u8) -> Result<Vec<u8>> {
    if fragments.len() < threshold as usize {
        return Err(PolygoneError::ShamirReconstruct(format!(
            "need {threshold} fragments, got {}",
            fragments.len()
        )));
    }

    let sharks = Sharks(threshold);

    let shares: Result<Vec<Share>> = fragments
        .iter()
        .map(|f| {
            Share::try_from(f.data.as_slice())
                .map_err(|e| PolygoneError::ShamirReconstruct(e.to_string()))
        })
        .collect();

    let secret = sharks
        .recover(shares?.iter())
        .map_err(|e| PolygoneError::ShamirReconstruct(e.to_string()))?;

    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shamir_4_of_7_round_trip() {
        let secret = b"polygone-shared-secret-32-bytes!";
        let frags = split(secret, 4, 7).unwrap();
        assert_eq!(frags.len(), 7);

        // Any 4 reconstruct
        let recovered = reconstruct(&frags[..4], 4).unwrap();
        assert_eq!(recovered, secret);
    }

    #[test]
    fn shamir_3_of_7_fails() {
        let secret = b"post-quantum-secret";
        let frags = split(secret, 4, 7).unwrap();
        assert!(reconstruct(&frags[..3], 4).is_err());
    }

    #[test]
    fn shamir_any_4_of_7_reconstruct() {
        let secret = b"any four fragments suffice";
        let frags = split(secret, 4, 7).unwrap();

        // Try all C(7,4) = 35 combinations
        for i in 0..7 {
            for j in (i+1)..7 {
                for k in (j+1)..7 {
                    for l in (k+1)..7 {
                        let subset = vec![
                            frags[i].clone(), frags[j].clone(),
                            frags[k].clone(), frags[l].clone(),
                        ];
                        let rec = reconstruct(&subset, 4).unwrap();
                        assert_eq!(rec, secret,
                            "Combination ({i},{j},{k},{l}) failed");
                    }
                }
            }
        }
    }
}
