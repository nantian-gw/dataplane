use std::borrow::Cow;

use anyhow::anyhow;
use regex::Regex;
use std::sync::OnceLock;

use crate::error::AIError;
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
#[derive(Debug)]
pub struct PromptGuardFilter {
    pub(crate) patterns: Vec<Regex>,
    pub(crate) keywords: Vec<PromptGuardKeyword>,
    pub(crate) enabled: bool,
    mode: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PromptGuardKeyword {
    pub(crate) original: String,
    pub(crate) normalized: String,
}

impl PromptGuardKeyword {
    fn new(keyword: String) -> Self {
        Self {
            normalized: keyword.to_lowercase(),
            original: keyword,
        }
    }
}

impl PromptGuardFilter {
    /// Create with default injection detection patterns.
    pub fn new() -> Result<Self, AIError> {
        Ok(Self {
            patterns: Self::default_patterns()?,
            keywords: vec![],
            enabled: true,
            mode: "block".into(),
        })
    }

    /// Create with custom settings.
    pub fn with_config(
        enabled: bool,
        mode: &str,
        custom_patterns: Vec<String>,
        keywords: Vec<String>,
    ) -> Result<Self, AIError> {
        let patterns = if custom_patterns.is_empty() {
            Self::default_patterns()?
        } else {
            custom_patterns
                .iter()
                .map(|pattern| {
                    Regex::new(pattern).map_err(|err| {
                        AIError::Internal(anyhow!("invalid custom prompt guard regex: {err}"))
                    })
                })
                .collect::<Result<Vec<_>, AIError>>()?
        };
        Ok(Self {
            patterns,
            keywords: Self::normalize_keywords(keywords),
            enabled,
            mode: mode.into(),
        })
    }

    fn normalize_keywords(keywords: Vec<String>) -> Vec<PromptGuardKeyword> {
        keywords.into_iter().map(PromptGuardKeyword::new).collect()
    }

    fn compile_builtin_pattern(label: &str, pattern: &str) -> Result<Regex, String> {
        Regex::new(pattern)
            .map_err(|err| format!("invalid built-in prompt guard regex {label}: {err}"))
    }

    fn default_patterns() -> Result<Vec<Regex>, AIError> {
        static PATTERNS: OnceLock<Result<Vec<Regex>, String>> = OnceLock::new();

        PATTERNS
            .get_or_init(|| {
                Ok(vec![
                    Self::compile_builtin_pattern(
                        "ignore-previous",
                        r"(?i)(ignore|forget|override)\s+(all\s+)?(previous|above|prior)\s+(instructions?|prompts?)",
                    )?,
                    Self::compile_builtin_pattern(
                        "dan",
                        r"(?i)(you\s+are|act\s+as|pretend\s+to\s+be)\s+(DAN|jailbroken)",
                    )?,
                    Self::compile_builtin_pattern(
                        "persona",
                        r"(?i)respond\s+in\s+(a\s+)?(different|new)\s+(persona|role|character)",
                    )?,
                    Self::compile_builtin_pattern(
                        "dont-follow",
                        r"(?i)(do\s+not|don't|never)\s+follow\s+(your|the)\s+(guidelines|rules|instructions)",
                    )?,
                    Self::compile_builtin_pattern(
                        "system-prompt",
                        r"(?i)system\s*prompt\s*[:=].*you\s+are",
                    )?,
                ])
            })
            .clone()
            .map_err(|message| AIError::Internal(anyhow!(message)))
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
            let text = match message_text(&msg.content) {
                Some(t) => t,
                None => continue,
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
            let text_lower = text.to_lowercase();
            for keyword in &self.keywords {
                if text_lower.contains(&keyword.normalized) {
                    return GuardResult::Block {
                        reason: format!("blocked_keyword: {}", keyword.original),
                        matched: keyword.original.clone(),
                    };
                }
            }
        }

        GuardResult::Pass
    }
}

/// Extract text content from an AI message part for security scanning.
/// MultiPart content parts are joined with spaces.
pub(crate) fn message_text(content: &AIContent) -> Option<Cow<'_, str>> {
    match content {
        AIContent::Text(s) => Some(Cow::Borrowed(s.as_str())),
        AIContent::MultiPart(parts) => {
            let texts: Vec<&str> = parts.iter().filter_map(|p| p.text.as_deref()).collect();
            if texts.is_empty() {
                None
            } else {
                Some(Cow::Owned(texts.join(" ")))
            }
        }
        AIContent::None => None,
    }
}
