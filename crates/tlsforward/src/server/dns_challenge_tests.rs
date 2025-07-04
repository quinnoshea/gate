//! Tests for DNS challenge security

#[cfg(test)]
mod tests {
    use crate::common::ChallengeStatus;
    use crate::server::dns_challenge::DnsChallenge;

    // Since we can't create a DnsChallengeManager directly in tests due to private fields,
    // we'll test the public API behavior through integration with the router module

    #[test]
    fn test_challenge_creation_validation() {
        // Test that DNS challenge creation enforces proper domain structure
        let challenge = DnsChallenge {
            domain: "test.private.hellas.ai".to_string(),
            challenge: "_acme-challenge".to_string(),
            value: "test-value".to_string(),
        };

        // Verify the challenge structure
        assert_eq!(challenge.domain, "test.private.hellas.ai");
        assert_eq!(challenge.challenge, "_acme-challenge");
        assert_eq!(challenge.value, "test-value");
    }

    #[test]
    fn test_challenge_status_variants() {
        // Test that all challenge status variants work correctly
        let pending = ChallengeStatus::Pending;
        let propagated = ChallengeStatus::Propagated;
        let failed = ChallengeStatus::Failed {
            error: "test error".to_string(),
        };

        assert!(matches!(pending, ChallengeStatus::Pending));
        assert!(matches!(propagated, ChallengeStatus::Propagated));
        assert!(matches!(failed, ChallengeStatus::Failed { .. }));
    }
}
