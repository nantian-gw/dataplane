use anyhow::anyhow;
use regex::Regex;
use std::sync::OnceLock;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::AIError;
use crate::format::ir::AIRequest;

/// Verdict from a content safety check.
#[derive(Debug, Clone, PartialEq)]
pub enum SafetyVerdict {
    /// Content passed all safety checks.
    Pass,
    /// Content matched a pattern but policy is set to flag (warn, do not block).
    Flag { category: String, matched: String },
    /// Content matched a pattern and should be blocked.
    Block { category: String, matched: String },
}

/// Content safety filter that scans AI request messages for harmful content
/// across five categories: violence, hate, self-harm, exploitation, and illegal.
///
/// Uses compiled regex patterns and keyword matching. Supports three modes:
/// `block`, `flag`, and `disabled`.
#[derive(Debug)]
pub struct ContentSafetyFilter {
    pub(crate) patterns: Vec<(String, Regex)>,
    pub(crate) keywords: Vec<(String, String)>,
    pub(crate) enabled: bool,
    pub(crate) block_mode: bool,
}

impl ContentSafetyFilter {
    /// Create a new content safety filter with default patterns and keywords.
    pub fn new() -> Result<Self, AIError> {
        Ok(Self {
            patterns: Self::default_patterns()?,
            keywords: Self::default_keywords(),
            enabled: true,
            block_mode: true,
        })
    }

    /// Create with custom configuration.
    pub fn with_config(
        enabled: bool,
        block_mode: bool,
        custom_patterns: Vec<(String, String)>,
        custom_keywords: Vec<(String, String)>,
    ) -> Result<Self, AIError> {
        let patterns = if custom_patterns.is_empty() {
            Self::default_patterns()?
        } else {
            Self::compile_custom_patterns(custom_patterns)?
        };
        let keywords = if custom_keywords.is_empty() {
            Self::default_keywords()
        } else {
            custom_keywords
        };
        Ok(Self {
            patterns,
            keywords,
            enabled,
            block_mode,
        })
    }

    /// Default regex patterns for the five content safety categories.
    fn default_patterns() -> Result<Vec<(String, Regex)>, AIError> {
        static DEFAULT_PATTERNS: OnceLock<Result<Vec<(String, Regex)>, String>> = OnceLock::new();

        match DEFAULT_PATTERNS
            .get_or_init(|| Self::compile_builtin_patterns().map_err(|err| err.to_string()))
        {
            Ok(patterns) => Ok(patterns.clone()),
            Err(message) => Err(AIError::Internal(anyhow!(message.clone()))),
        }
    }

    fn compile_builtin_patterns() -> Result<Vec<(String, Regex)>, AIError> {
        #[cfg(test)]
        BUILTIN_PATTERN_COMPILE_COUNT.fetch_add(1, Ordering::SeqCst);

        [
            (
                "violence-1",
                "violence",
                r"(?i)(how\s+to|teach\s+me\s+to|instructions?\s+for)\s+(kill|murder|assassinate|torture|maim|slaughter)",
            ),
            (
                "violence-2",
                "violence",
                r"(?i)(bomb|explosive|weapon|firearm)\s+(making|instructions|manufacturing|construction)",
            ),
            (
                "hate-1",
                "hate",
                r"(?i)(ethnic\s+cleansing|racial\s+purity|white\s+supremacy|aryan|pogrom)",
            ),
            (
                "hate-2",
                "hate",
                r"(?i)(genocide|holocaust)\s+(denial|is\s+a\s+hoax|never\s+happened)",
            ),
            (
                "self-harm-1",
                "self_harm",
                r"(?i)(best|easiest|most\s+effective)\s+(way|method)s?\s+to\s+(kill\s+myself|commit\s+suicide|end\s+my\s+life|self[- ]?harm)",
            ),
            (
                "self-harm-2",
                "self_harm",
                r"(?i)(suicide|self[- ]?harm)\s+(guide|instructions?|manual|how\s+to)",
            ),
            (
                "exploitation-1",
                "exploitation",
                r"(?i)(child\s+(abuse|exploitation|pornography|trafficking)|minor\s+(exploitation|trafficking))",
            ),
            (
                "exploitation-2",
                "exploitation",
                r"(?i)(human\s+trafficking|forced\s+labour|modern\s+slavery|sex\s+trafficking)",
            ),
            (
                "illegal-1",
                "illegal",
                r"(?i)(how\s+to|instructions?\s+for)\s+(manufacture|synthesize|produce)\s+(meth|fentanyl|heroin|cocaine|lsd|ecstasy)",
            ),
            (
                "illegal-2",
                "illegal",
                r"(?i)(hacking|phishing)\s+(guide|tutorial|instructions?|how\s+to\s+(hack|break\s+into|infiltrate))",
            ),
        ]
        .into_iter()
        .map(|(label, category, pattern)| Self::compile_builtin_pattern(label, category, pattern))
        .collect()
    }

    fn compile_builtin_pattern(
        label: &str,
        category: &str,
        pattern: &str,
    ) -> Result<(String, Regex), AIError> {
        Regex::new(pattern)
            .map(|regex| (category.to_string(), regex))
            .map_err(|err| {
                AIError::Internal(anyhow!(
                    "invalid built-in content safety regex {label}: {err}"
                ))
            })
    }

    fn compile_custom_patterns(
        custom_patterns: Vec<(String, String)>,
    ) -> Result<Vec<(String, Regex)>, AIError> {
        custom_patterns
            .into_iter()
            .map(|(category, pattern)| {
                Regex::new(&pattern)
                    .map(|regex| (category.clone(), regex))
                    .map_err(|err| {
                        AIError::Internal(anyhow!(
                            "invalid custom content safety regex for category {category}: {err}"
                        ))
                    })
            })
            .collect()
    }

    /// Default keyword list for the five content safety categories.
    fn default_keywords() -> Vec<(String, String)> {
        vec![
            ("violence".into(), "how to build a bomb".into()),
            ("violence".into(), "murder instructions".into()),
            ("hate".into(), "ethnic cleansing".into()),
            ("hate".into(), "racial inferiority".into()),
            ("self_harm".into(), "suicide methods".into()),
            ("self_harm".into(), "self harm guide".into()),
            ("exploitation".into(), "child exploitation material".into()),
            ("exploitation".into(), "human trafficking manual".into()),
            ("illegal".into(), "how to make meth".into()),
            ("illegal".into(), "phishing tutorial".into()),
        ]
    }

    /// Scan all messages in an AI request for harmful content.
    ///
    /// Returns the safety verdict based on the configured mode:
    /// - `disabled`: always `Pass`
    /// - `block` mode: `Block` on match
    /// - `flag` mode: `Flag` on match
    pub fn check(&self, request: &AIRequest) -> SafetyVerdict {
        if !self.enabled {
            return SafetyVerdict::Pass;
        }

        for msg in &request.messages {
            let text = match crate::prompt_guard::message_text(&msg.content) {
                Some(t) if !t.is_empty() => t,
                _ => continue,
            };

            // Check regex patterns
            for (category, regex) in &self.patterns {
                if let Some(captured) = regex.find(&text) {
                    let matched = captured.as_str().to_string();
                    if self.block_mode {
                        return SafetyVerdict::Block {
                            category: category.clone(),
                            matched,
                        };
                    } else {
                        return SafetyVerdict::Flag {
                            category: category.clone(),
                            matched,
                        };
                    }
                }
            }

            // Check keywords (case-insensitive substring)
            let lower = text.to_lowercase();
            for (category, keyword) in &self.keywords {
                if lower.contains(&keyword.to_lowercase()) {
                    let matched = keyword.clone();
                    if self.block_mode {
                        return SafetyVerdict::Block {
                            category: category.clone(),
                            matched,
                        };
                    } else {
                        return SafetyVerdict::Flag {
                            category: category.clone(),
                            matched,
                        };
                    }
                }
            }
        }

        SafetyVerdict::Pass
    }

    /// Returns whether the filter is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
static BUILTIN_PATTERN_COMPILE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_patterns_compile_once_across_constructors() {
        assert_eq!(BUILTIN_PATTERN_COMPILE_COUNT.load(Ordering::SeqCst), 0);

        let _first =
            ContentSafetyFilter::new().expect("default content safety filter should build");
        let _second =
            ContentSafetyFilter::new().expect("default content safety filter should build");

        assert_eq!(BUILTIN_PATTERN_COMPILE_COUNT.load(Ordering::SeqCst), 1);
    }
}
