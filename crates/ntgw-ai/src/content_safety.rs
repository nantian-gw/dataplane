use regex::Regex;
use std::sync::LazyLock;

use crate::format::ir::{AIContent, AIRequest};

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
pub struct ContentSafetyFilter {
    patterns: Vec<(String, Regex)>,
    keywords: Vec<(String, String)>,
    enabled: bool,
    block_mode: bool,
}

impl ContentSafetyFilter {
    /// Create a new content safety filter with default patterns and keywords.
    pub fn new() -> Self {
        Self {
            patterns: Self::default_patterns(),
            keywords: Self::default_keywords(),
            enabled: true,
            block_mode: true,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        enabled: bool,
        block_mode: bool,
        custom_patterns: Vec<(String, String)>,
        custom_keywords: Vec<(String, String)>,
    ) -> Self {
        let patterns = if custom_patterns.is_empty() {
            Self::default_patterns()
        } else {
            custom_patterns
                .into_iter()
                .filter_map(|(cat, p)| Regex::new(&p).ok().map(|r| (cat, r)))
                .collect()
        };
        let keywords = if custom_keywords.is_empty() {
            Self::default_keywords()
        } else {
            custom_keywords
        };
        Self {
            patterns,
            keywords,
            enabled,
            block_mode,
        }
    }

    /// Default regex patterns for the five content safety categories.
    /// Compiled once via LazyLock for reuse across all ContentSafetyFilter instances.
    fn default_patterns() -> Vec<(String, Regex)> {
        static PATTERNS: LazyLock<Vec<(String, Regex)>> = LazyLock::new(|| {
            vec![
            // ── Violence ──────────────────────────────────────────
            (
                "violence".into(),
                Regex::new(r"(?i)(how\s+to|teach\s+me\s+to|instructions?\s+for)\s+(kill|murder|assassinate|torture|maim|slaughter)")
                    .expect("valid content-safety regex: violence-1"),
            ),
            (
                "violence".into(),
                Regex::new(r"(?i)(bomb|explosive|weapon|firearm)\s+(making|instructions|manufacturing|construction)")
                    .expect("valid content-safety regex: violence-2"),
            ),
            // ── Hate ──────────────────────────────────────────────
            (
                "hate".into(),
                Regex::new(r"(?i)(ethnic\s+cleansing|racial\s+purity|white\s+supremacy|aryan|pogrom)")
                    .expect("valid content-safety regex: hate-1"),
            ),
            (
                "hate".into(),
                Regex::new(r"(?i)(genocide|holocaust)\s+(denial|is\s+a\s+hoax|never\s+happened)")
                    .expect("valid content-safety regex: hate-2"),
            ),
            // ── Self-harm ─────────────────────────────────────────
            (
                "self_harm".into(),
                Regex::new(r"(?i)(best|easiest|most\s+effective)\s+(way|method)s?\s+to\s+(kill\s+myself|commit\s+suicide|end\s+my\s+life|self[- ]?harm)")
                    .expect("valid content-safety regex: self-harm-1"),
            ),
            (
                "self_harm".into(),
                Regex::new(r"(?i)(suicide|self[- ]?harm)\s+(guide|instructions?|manual|how\s+to)")
                    .expect("valid content-safety regex: self-harm-2"),
            ),
            // ── Exploitation ──────────────────────────────────────
            (
                "exploitation".into(),
                Regex::new(r"(?i)(child\s+(abuse|exploitation|pornography|trafficking)|minor\s+(exploitation|trafficking))")
                    .expect("valid content-safety regex: exploitation-1"),
            ),
            (
                "exploitation".into(),
                Regex::new(r"(?i)(human\s+trafficking|forced\s+labour|modern\s+slavery|sex\s+trafficking)")
                    .expect("valid content-safety regex: exploitation-2"),
            ),
            // ── Illegal ───────────────────────────────────────────
            (
                "illegal".into(),
                Regex::new(r"(?i)(how\s+to|instructions?\s+for)\s+(manufacture|synthesize|produce)\s+(meth|fentanyl|heroin|cocaine|lsd|ecstasy)")
                    .expect("valid content-safety regex: illegal-1"),
            ),
            (
                "illegal".into(),
                Regex::new(r"(?i)(hacking|phishing)\s+(guide|tutorial|instructions?|how\s+to\s+(hack|break\s+into|infiltrate))")
                    .expect("valid content-safety regex: illegal-2"),
            ),
        ]
        });
        PATTERNS.clone()
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
            let text = match &msg.content {
                AIContent::Text(t) => t.as_str(),
                AIContent::MultiPart(_) => continue,
                AIContent::None => continue,
            };

            if text.is_empty() {
                continue;
            }

            // Check regex patterns
            for (category, regex) in &self.patterns {
                if let Some(captured) = regex.find(text) {
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

impl Default for ContentSafetyFilter {
    fn default() -> Self {
        Self::new()
    }
}
