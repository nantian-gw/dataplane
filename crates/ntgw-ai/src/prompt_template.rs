use std::collections::HashMap;

use crate::format::ir::{AIContent, AIMessage, AIRequest, AIRole};

#[derive(Debug, Clone, Default)]
pub struct PromptTemplate {
    pub name: String,
    pub system_prompt: String,
    pub variables: HashMap<String, String>,
    pub few_shot_examples: Vec<AIMessage>,
}

impl PromptTemplate {
    pub fn new(name: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            system_prompt: system_prompt.into(),
            variables: HashMap::new(),
            few_shot_examples: Vec::new(),
        }
    }

    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }

    pub fn add_example(&mut self, role: AIRole, content: impl Into<String>) {
        self.few_shot_examples.push(AIMessage {
            role,
            content: AIContent::Text(content.into()),
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
    }

    fn resolve(&self, text: &str) -> String {
        let mut resolved = text.to_string();
        for (key, value) in &self.variables {
            let placeholder = format!("{{{key}}}");
            resolved = resolved.replace(&placeholder, value);
        }
        resolved
    }
}

#[derive(Debug, Clone, Default)]
pub struct PromptInjector {
    templates: HashMap<String, PromptTemplate>,
}

impl PromptInjector {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    pub fn register(&mut self, template: PromptTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

#[must_use]
    pub fn template(&self, name: &str) -> Option<&PromptTemplate> {
        self.templates.get(name)
    }

    pub fn inject(&self, template_name: &str, request: &mut AIRequest) -> bool {
        let template = match self.templates.get(template_name) {
            Some(t) => t,
            None => return false,
        };

        let resolved_system = template.resolve(&template.system_prompt);

        let mut new_messages =
            Vec::with_capacity(1 + template.few_shot_examples.len() + request.messages.len());

        new_messages.push(AIMessage {
            role: AIRole::System,
            content: AIContent::Text(resolved_system),
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
        });

        new_messages.extend(template.few_shot_examples.clone());

        new_messages.append(&mut request.messages);

        request.messages = new_messages;

        true
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }
}
