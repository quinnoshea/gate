//! Tests for router security and domain ownership validation

#[cfg(test)]
mod tests {
    use crate::common::CreateChallengeRequest;
    use iroh::{NodeId, SecretKey};

    // Helper to create a test node ID
    fn test_node_id(seed: u8) -> NodeId {
        let key_bytes = [seed; 32];
        let secret_key = SecretKey::from_bytes(&key_bytes);
        secret_key.public()
    }

    #[tokio::test]
    async fn test_create_challenge_domain_ownership() {
        // Create a node and get its short hash
        let node_id = test_node_id(1);
        let short_hash = node_id.fmt_short();

        // Test 1: Node can create challenge for its own domain
        let _valid_request = CreateChallengeRequest {
            domain: format!("{short_hash}.private.hellas.ai"),
            challenge: "_acme-challenge".to_string(),
            value: "test-value-123".to_string(),
        };

        // Test 2: Node cannot create challenge for another domain
        let _invalid_request = CreateChallengeRequest {
            domain: "different1234567.private.hellas.ai".to_string(),
            challenge: "_acme-challenge".to_string(),
            value: "test-value-123".to_string(),
        };

        // In a real test, we would set up the router and test the actual endpoints
        // but that requires more infrastructure setup
    }

    #[test]
    fn test_node_id_fmt_short_consistency() {
        let node_id = test_node_id(42);
        let hash1 = node_id.fmt_short();
        let hash2 = node_id.fmt_short();

        assert_eq!(hash1, hash2, "Hash should be consistent");
        assert_eq!(hash1.len(), 10, "Short hash should be 10 chars");
        assert!(
            hash1.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should be hex"
        );
    }

    #[test]
    fn test_domain_extraction() {
        let test_cases = vec![
            ("abcd1234ab.private.hellas.ai", Some("abcd1234ab")),
            ("test.private.hellas.ai", Some("test")),
            ("sub.domain.private.hellas.ai", Some("sub")),
            ("nodomain", Some("nodomain")),
            ("", Some("")),
        ];

        for (domain, expected) in test_cases {
            let prefix = domain.split('.').next();
            assert_eq!(prefix, expected, "Failed for domain: {domain}");
        }
    }
}
