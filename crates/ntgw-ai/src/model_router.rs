use std::collections::HashMap;

use crate::format::ir::AIRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Complexity {
    Simple,
    Medium,
    Complex,
}

#[derive(Debug, Clone)]
pub struct ModelRoute {
    pub model: String,
    pub weight: u32,
    pub max_tokens: Option<u32>,
}

impl ModelRoute {
    pub fn new(model: impl Into<String>, weight: u32, max_tokens: Option<u32>) -> Self {
        Self {
            model: model.into(),
            weight,
            max_tokens,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelRouter {
    routes: HashMap<Complexity, Vec<ModelRoute>>,
}

impl ModelRouter {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn add_routes(&mut self, complexity: Complexity, routes: Vec<ModelRoute>) {
        let mut sorted = routes;
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        self.routes.insert(complexity, sorted);
    }

    pub fn classify(&self, request: &AIRequest) -> Complexity {
        let total_len: usize = request
            .messages
            .iter()
            .map(|msg| msg.content.char_count())
            .sum();

        if total_len < 200 {
            Complexity::Simple
        } else if total_len < 2000 {
            Complexity::Medium
        } else {
            Complexity::Complex
        }
    }

    pub fn route(&self, complexity: Complexity) -> Option<&ModelRoute> {
        self.routes.get(&complexity).and_then(|v| v.first())
    }

    pub fn classify_and_route(&self, request: &AIRequest) -> Option<&ModelRoute> {
        let complexity = self.classify(request);
        self.route(complexity)
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

trait CharCount {
    fn char_count(&self) -> usize;
}

impl CharCount for crate::format::ir::AIContent {
    fn char_count(&self) -> usize {
        match self {
            crate::format::ir::AIContent::Text(s) => s.chars().count(),
            crate::format::ir::AIContent::MultiPart(parts) => parts
                .iter()
                .filter_map(|p| p.text.as_deref())
                .map(|t| t.chars().count())
                .sum(),
            crate::format::ir::AIContent::None => 0,
        }
    }
}
