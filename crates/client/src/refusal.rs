//! CLI.6 — traduire chaque code de refus du daemon en cause et action.
//!
//! Le daemon renvoie un code technique stable (`invalid_request`,
//! `insufficient_headroom`, …). Un utilisateur ne peut pas agir sur un code
//! seul : cette table associe à chaque code une cause lisible **et** une action
//! concrète. Le code technique reste toujours affiché pour un rapport de bug.
//!
//! La liste [`KNOWN_REASONS`] est comparée aux constantes du daemon par le test
//! `every_reason_code_has_an_action` : ajouter un motif sans traduction fait
//! échouer la suite.

/// Tous les motifs de refus d'allocation publiés par `polygoned`.
pub const KNOWN_REASONS: [&str; 8] = [
    polygoned::socket::REASON_INVALID_REQUEST,
    polygoned::socket::REASON_INSUFFICIENT_HEADROOM,
    polygoned::socket::REASON_INSUFFICIENT_CPU,
    polygoned::socket::REASON_INSUFFICIENT_VRAM,
    polygoned::socket::REASON_INSUFFICIENT_STORAGE,
    polygoned::socket::REASON_POLICY_SHRINKING,
    polygoned::socket::REASON_EXCEEDS_MEASURED,
    polygoned::socket::REASON_UNKNOWN_ALLOCATION,
];

/// Cause et action associées à un code de refus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Advice {
    pub cause: &'static str,
    pub action: &'static str,
}

/// Traduit un code de refus. `None` signifie « code inconnu de cette version »,
/// jamais « pas de refus ».
pub fn advice(reason: &str) -> Option<Advice> {
    let advice = match reason {
        polygoned::socket::REASON_INVALID_REQUEST => Advice {
            cause: "la demande d'allocation est mal formée",
            action: "relancez la commande ; si le refus persiste, rapportez le code avec la sortie",
        },
        polygoned::socket::REASON_INSUFFICIENT_HEADROOM => Advice {
            cause: "la marge réservable de la machine est insuffisante",
            action: "réduisez --ram/--cpu, ou réessayez quand un pair rend son bail",
        },
        polygoned::socket::REASON_INSUFFICIENT_CPU => Advice {
            cause: "le nombre de cœurs demandés n'est pas disponible",
            action: "réduisez --cpu et relancez",
        },
        polygoned::socket::REASON_INSUFFICIENT_VRAM => Advice {
            cause: "la VRAM demandée n'est pas disponible",
            action: "réduisez la VRAM demandée ou libérez le GPU",
        },
        polygoned::socket::REASON_INSUFFICIENT_STORAGE => Advice {
            cause: "l'espace de stockage demandé n'est pas disponible",
            action: "libérez de l'espace disque ou réduisez la taille demandée",
        },
        polygoned::socket::REASON_POLICY_SHRINKING => Advice {
            cause: "la politique d'allocation réduit les baux en cours",
            action: "attendez la fin de la réduction, puis redemandez un bail",
        },
        polygoned::socket::REASON_EXCEEDS_MEASURED => Advice {
            cause: "la demande dépasse ce que la machine possède réellement",
            action: "réduisez la demande sous la capacité mesurée par « polygoned status »",
        },
        polygoned::socket::REASON_UNKNOWN_ALLOCATION => Advice {
            cause: "ce bail est inconnu du ledger ou déjà échu",
            action: "redemandez une allocation ; ne réutilisez pas cet identifiant",
        },
        _ => return None,
    };
    Some(advice)
}

/// Ligne affichée à l'utilisateur : code technique conservé, puis cause et
/// action. Un code inconnu est signalé comme tel, jamais masqué.
pub fn describe(reason: &str) -> String {
    match advice(reason) {
        Some(advice) => format!(
            "{reason} — cause : {} ; action : {}",
            advice.cause, advice.action
        ),
        None => format!(
            "{reason} — motif inconnu de cette version ; conservez ce code pour un rapport de bug — \
             le nœud n'a rien ouvert"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_code_has_an_action() {
        // La liste locale doit couvrir exactement les constantes du daemon.
        // Un nouveau REASON_* non traduit casse ce test.
        let daemon_reasons = [
            polygoned::socket::REASON_INVALID_REQUEST,
            polygoned::socket::REASON_INSUFFICIENT_HEADROOM,
            polygoned::socket::REASON_INSUFFICIENT_CPU,
            polygoned::socket::REASON_INSUFFICIENT_VRAM,
            polygoned::socket::REASON_INSUFFICIENT_STORAGE,
            polygoned::socket::REASON_POLICY_SHRINKING,
            polygoned::socket::REASON_EXCEEDS_MEASURED,
            polygoned::socket::REASON_UNKNOWN_ALLOCATION,
        ];
        assert_eq!(KNOWN_REASONS, daemon_reasons, "liste des motifs incomplète");

        for code in KNOWN_REASONS {
            let advice = advice(code).unwrap_or_else(|| panic!("aucune action pour {code}"));
            assert!(!advice.cause.is_empty(), "cause vide pour {code}");
            assert!(!advice.action.is_empty(), "action vide pour {code}");
            let rendered = describe(code);
            assert!(rendered.starts_with(code), "le code technique doit rester");
            assert!(
                rendered.contains(advice.action),
                "l'action doit être affichée"
            );
        }
    }

    #[test]
    fn unknown_denial_keeps_the_technical_code() {
        let rendered = describe("motif_qui_n_existe_pas");
        assert!(rendered.starts_with("motif_qui_n_existe_pas"));
        assert!(rendered.contains("motif inconnu"));
        assert!(rendered.contains("rapport de bug"));
    }
}
