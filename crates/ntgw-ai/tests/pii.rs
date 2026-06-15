use ntgw_ai::pii::{PIIEntityType, PIIMasker, PIIMode};

#[test]
fn test_detect_email() {
    let masker = PIIMasker::new(PIIMode::Mask).expect("default pii regex patterns should compile");
    let matches = masker.detect("Contact user@example.com for help");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].entity_type, PIIEntityType::Email);
    assert_eq!(matches[0].text, "user@example.com");
}

#[test]
fn test_detect_phone() {
    let masker = PIIMasker::new(PIIMode::Mask).expect("default pii regex patterns should compile");
    let matches = masker.detect("Call 13812345678 now");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].entity_type, PIIEntityType::Phone);
    assert_eq!(matches[0].text, "13812345678");
}

#[test]
fn test_mask_removes_pii() {
    let masker = PIIMasker::new(PIIMode::Mask).expect("default pii regex patterns should compile");
    let input = "Email user@test.com and call 13812345678";
    let (masked, count, _) = masker.mask(input);
    assert_eq!(count, 2);
    assert!(!masked.contains("user@test.com"));
    assert!(!masked.contains("13812345678"));
    assert!(masked.contains("[email]"));
    assert!(masked.contains("[phone]"));
}

#[test]
fn test_mode_mask_vs_redact() {
    let input = "user@test.com";
    let mask_result = PIIMasker::new(PIIMode::Mask)
        .expect("default pii regex patterns should compile")
        .mask(input)
        .0;
    let redact_result = PIIMasker::new(PIIMode::Redact)
        .expect("default pii regex patterns should compile")
        .mask(input)
        .0;
    let anon_result = PIIMasker::new(PIIMode::Anonymize)
        .expect("default pii regex patterns should compile")
        .mask(input)
        .0;

    assert_eq!(mask_result, "[email]");
    assert_eq!(redact_result, "[REDACTED]");
    assert_eq!(anon_result, "<email>");
}

#[test]
fn test_no_pii_unchanged() {
    let masker = PIIMasker::new(PIIMode::Mask).expect("default pii regex patterns should compile");
    let input = "Hello, this is a clean message with no PII.";
    let (masked, count, _) = masker.mask(input);
    assert_eq!(count, 0);
    assert_eq!(masked, input);
}

#[test]
fn test_disabled() {
    let masker = PIIMasker::new(PIIMode::Mask)
        .expect("default pii regex patterns should compile")
        .with_enabled(false);
    let original = "alice@example.com";
    let (result, count, _) = masker.mask(original);
    assert_eq!(result, original);
    assert_eq!(count, 0);
}

#[test]
fn test_mask_credit_card() {
    let masker = PIIMasker::new(PIIMode::Mask).expect("default pii regex patterns should compile");
    let (result, count, _) = masker.mask("Card: 4111-1111-1111-1111");
    assert!(result.contains("[credit_card]"));
    assert_eq!(count, 1);
}

#[test]
fn test_default_construction_succeeds() {
    let masker = PIIMasker::new(PIIMode::Mask).expect("default pii regex patterns should compile");
    assert!(masker.is_enabled());
}
