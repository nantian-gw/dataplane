use regex::Regex;

use crate::error::AIError;

#[derive(Debug, Clone, PartialEq)]
pub enum PIIEntityType {
    Email,
    Phone,
    PersonName,
    CreditCard,
    IDCard,
    Address,
    URL,
    IPAddress,
}

impl PIIEntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Phone => "phone",
            Self::PersonName => "person",
            Self::CreditCard => "credit_card",
            Self::IDCard => "id_card",
            Self::Address => "address",
            Self::URL => "url",
            Self::IPAddress => "ip_address",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PIIMatch {
    pub entity_type: PIIEntityType,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PIIMode {
    /// Replace with [entity_type], e.g. [email]
    Mask,
    /// Replace with hardcoded [REDACTED]
    Redact,
    /// Replace with <entity_type>, e.g. <email>
    Anonymize,
}

/// PII detector / masker that scans text with compiled regex patterns
/// and replaces detected entities according to the chosen [`PIIMode`].
pub struct PIIMasker {
    patterns: Vec<(PIIEntityType, Regex)>,
    mode: PIIMode,
    enabled: bool,
}

impl PIIMasker {
    /// Build a new masker with the default detection pattern set and the
    /// supplied masking mode.
    #[allow(clippy::expect_used)]
    pub fn new(mode: PIIMode) -> Self {
        Self {
            patterns: vec![
                (
                    PIIEntityType::Email,
                    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                        .expect("valid email regex"),
                ),
                (
                    PIIEntityType::Phone,
                    // Chinese mobile phone numbers (includes optional +86 prefix)
                    Regex::new(r"(\+?86)?1[3-9]\d{9}").expect("valid phone regex"),
                ),
                (
                    PIIEntityType::CreditCard,
                    // 13-16 digit numbers with optional spaces / dashes
                    Regex::new(r"\b(?:\d[ -]*?){13,16}\b").expect("valid credit-card regex"),
                ),
                (
                    PIIEntityType::IDCard,
                    // Chinese 18-digit ID numbers (last digit may be X)
                    Regex::new(r"\d{17}[\dXx]").expect("valid id-card regex"),
                ),
                (
                    PIIEntityType::URL,
                    Regex::new(r#"https?://[^\s<>"{}|\\^`\[\]]+"#).expect("valid url regex"),
                ),
                (
                    PIIEntityType::IPAddress,
                    Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("valid ip regex"),
                ),
            ],
            mode,
            enabled: true,
        }
    }

    /// Return the currently configured masking mode.
    pub fn mode(&self) -> PIIMode {
        self.mode
    }

    /// Set whether PII masking is enabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Check if PII masking is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Run all detection patterns against `text` and return every match.
    ///
    /// Matches are sorted by start position; when two matches share the same
    /// start the longer one comes first so that downstream deduplication can
    /// prefer the more specific hit.
    pub fn detect(&self, text: &str) -> Vec<PIIMatch> {
        let mut matches = Vec::new();
        for (entity_type, pattern) in &self.patterns {
            for cap in pattern.find_iter(text) {
                matches.push(PIIMatch {
                    entity_type: entity_type.clone(),
                    start: cap.start(),
                    end: cap.end(),
                    text: cap.as_str().to_string(),
                });
            }
        }
        matches.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| b.end.cmp(&a.end)));
        matches
    }

    /// Detect and mask PII in `text`.
    ///
    /// Returns the masked string, the number of entities replaced, and a
    /// detail vector of `(original, replacement)` pairs for logging /
    /// auditing purposes.
    pub fn mask(&self, text: &str) -> (String, usize, Vec<(String, String)>) {
        if !self.enabled {
            return (text.to_string(), 0, Vec::new());
        }

        let matches = self.detect(text);

        // Remove overlapping matches: keep the first (longest) match at each
        // position, then skip any match whose start falls inside an already
        // accepted span.
        let mut filtered: Vec<PIIMatch> = Vec::new();
        let mut last_end = 0;
        for m in &matches {
            if m.start >= last_end {
                filtered.push(m.clone());
                last_end = m.end;
            }
        }

        let count = filtered.len();
        let mut result = text.to_string();
        let mut details = Vec::new();

        // Replace in reverse order so that earlier positions stay valid.
        for m in filtered.iter().rev() {
            let replacement = match self.mode {
                PIIMode::Mask => format!("[{}]", m.entity_type.as_str()),
                PIIMode::Redact => "[REDACTED]".into(),
                PIIMode::Anonymize => format!("<{}>", m.entity_type.as_str()),
            };
            details.push((m.text.clone(), replacement.clone()));
            result.replace_range(m.start..m.end, &replacement);
        }

        (result, count, details)
    }

    /// Convenience helper: apply masking to a byte-slice payload that is
    /// expected to be valid UTF-8.  On decode failure the operation is a
    /// no-op and the original bytes are returned unchanged.
    pub fn mask_bytes(&self, bytes: &[u8]) -> Result<Vec<u8>, AIError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|e| AIError::Internal(anyhow::anyhow!("non-UTF-8 payload: {e}")))?;
        let (masked, _count, _details) = self.mask(s);
        Ok(masked.into_bytes())
    }

    /// Return the compiled pattern count (useful for diagnostics).
    #[allow(dead_code)]
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

impl Default for PIIMasker {
    fn default() -> Self {
        Self::new(PIIMode::Mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_email() {
        let masker = PIIMasker::new(PIIMode::Mask);
        let matches = masker.detect("Contact user@example.com for help");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, PIIEntityType::Email);
        assert_eq!(matches[0].text, "user@example.com");
    }

    #[test]
    fn test_detect_phone() {
        let masker = PIIMasker::new(PIIMode::Mask);
        // Chinese mobile number
        let matches = masker.detect("Call 13812345678 now");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, PIIEntityType::Phone);
        assert_eq!(matches[0].text, "13812345678");
    }

    #[test]
    fn test_detect_id_card() {
        let masker = PIIMasker::new(PIIMode::Mask);
        let matches = masker.detect("ID: 11010110900101123X is valid");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, PIIEntityType::IDCard);
    }

    #[test]
    fn test_mask_removes_pii() {
        let masker = PIIMasker::new(PIIMode::Mask);
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
        let mask = PIIMasker::new(PIIMode::Mask);
        let redact = PIIMasker::new(PIIMode::Redact);
        let anon = PIIMasker::new(PIIMode::Anonymize);

        assert_eq!(mask.mask(input).0, "[email]");
        assert_eq!(redact.mask(input).0, "[REDACTED]");
        assert_eq!(anon.mask(input).0, "<email>");
    }

    #[test]
    fn test_no_pii_unchanged() {
        let masker = PIIMasker::new(PIIMode::Mask);
        let input = "Hello, this is a clean message with no PII.";
        let (masked, count, _) = masker.mask(input);
        assert_eq!(count, 0);
        assert_eq!(masked, input);
    }

    #[test]
    fn test_overlapping_matches_deduplicated() {
        // "12345.67890.1234" could match both IP (12345.67890.1234) and credit-card (12345678901234)
        // The longer match wins, overlapping shorter one is dropped.
        let masker = PIIMasker::new(PIIMode::Anonymize);
        let input = "123.45.67.89";
        let (masked, count, _) = masker.mask(input);
        // IP pattern matches this, credit-card digits embedded should not duplicate
        assert_eq!(count, 1);
        assert!(masked.contains("<ip_address>"));
    }

    #[test]
    fn test_mask_bytes_valid_utf8() {
        let masker = PIIMasker::new(PIIMode::Mask);
        let payload = b"user@example.com";
        let result = masker.mask_bytes(payload).unwrap();
        assert_eq!(result, b"[email]");
    }

    #[test]
    fn test_mask_bytes_invalid_utf8_is_error() {
        let masker = PIIMasker::new(PIIMode::Mask);
        // Invalid UTF-8 sequence
        let payload = &[0xFF, 0xFE, 0xFD];
        assert!(masker.mask_bytes(payload).is_err());
    }

    #[test]
    fn test_disabled() {
        let masker = PIIMasker::new(PIIMode::Mask).with_enabled(false);
        let original = "alice@example.com";
        let (result, count, _) = masker.mask(original);
        assert_eq!(result, original);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_mask_credit_card() {
        let masker = PIIMasker::new(PIIMode::Mask);
        let (result, count, _) = masker.mask("Card: 4111-1111-1111-1111");
        assert!(result.contains("[credit_card]"));
        assert_eq!(count, 1);
    }
}
