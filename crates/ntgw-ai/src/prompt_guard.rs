use regex::Regex;
use std::sync::LazyLock;

use crate::format::ir::{AIContent, AIRequest};

/// Result of a prompt guard check.
#[derive(Debug, Clone)]
pub enum GuardResult {
    /// Message passed all checks.
    Pass,
    /// Message was blocked by a pattern or keyword.
    Block { reason: String, matched: String },
}

/// Injection detection filter for AI requests.
///
/// Checks all messages in a request against configurable regex patterns
/// and keywords. Supports three modes: `block`, `warn`, `log`.
pub struct PromptGuardFilter {
    patterns: Vec<Regex>,
    keywords: Vec<String>,
    enabled: bool,
    mode: String,
}

impl PromptGuardFilter {
    /// Create with default injection detection patterns.
    pub fn new() -> Self {
        Self {
            patterns: Self::default_patterns(),
            keywords: vec![],
            enabled: true,
            mode: "block".into(),
        }
    }

    /// Create with custom settings.
    pub fn with_config(
        enabled: bool,
        mode: &str,
        custom_patterns: Vec<String>,
        keywords: Vec<String>,
    ) -> Self {
        let patterns = if custom_patterns.is_empty() {
            Self::default_patterns()
        } else {
            custom_patterns
                .iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect()
        };
        Self {
            patterns,
            keywords,
            enabled,
            mode: mode.into(),
        }
    }

    fn default_patterns() -> Vec<Regex> {
        static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
            vec![
                Regex::new(
                r"(?i)(ignore|forget|override)\s+(all\s+)?(previous|above|prior)\s+(instructions?|prompts?)",
            )
            .expect("valid prompt-guard regex: ignore-previous"),
            Regex::new(r"(?i)(you\s+are|act\s+as|pretend\s+to\s+be)\s+(DAN|jailbroken)").expect("valid prompt-guard regex: dan"),
            Regex::new(
                r"(?i)respond\s+in\s+(a\s+)?(different|new)\s+(persona|role|character)",
            )
            .expect("valid prompt-guard regex: persona"),
            Regex::new(r"(?i)(do\s+not|don't|never)\s+follow\s+(your|the)\s+(guidelines|rules|instructions)")
                .expect("valid prompt-guard regex: dont-follow"),
            Regex::new(r"(?i)system\s*prompt\s*[:=].*you\s+are").expect("valid prompt-guard regex: system-prompt"),
            ]
        });
        PATTERNS.clone()
    }

    /// Returns the configured mode.
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// Check all messages in a request for injection patterns.
    pub fn check(&self, request: &AIRequest) -> GuardResult {
        if !self.enabled {
            return GuardResult::Pass;
        }

        for msg in &request.messages {
            let text = match &msg.content {
                AIContent::Text(s) => s.clone(),
                AIContent::MultiPart(parts) => parts
                    .iter()
                    .filter_map(|p| p.text.clone())
                    .collect::<Vec<_>>()
                    .join(" "),
                AIContent::None => continue,
            };

            // Check regex patterns
            for pattern in &self.patterns {
                if let Some(matched) = pattern.find(&text) {
                    return GuardResult::Block {
                        reason: "injection_pattern_match".into(),
                        matched: matched.as_str().to_string(),
                    };
                }
            }

            // Check keywords
            for keyword in &self.keywords {
                if text.to_lowercase().contains(&keyword.to_lowercase()) {
                    return GuardResult::Block {
                        reason: format!("blocked_keyword: {keyword}"),
                        matched: keyword.clone(),
                    };
                }
            }
        }

        GuardResult::Pass
    }
}

impl Default for PromptGuardFilter {
    fn default() -> Self {
        Self::new()
    }
}
