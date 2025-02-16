use crate::database::Database;
use gpui::{AnyElement, IntoElement, RenderOnce};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionId {
    /// Built-in actions with string identifiers
    Builtin(&'static str),
    /// Dynamic actions with database IDs
    Dynamic(usize),
}

impl ActionId {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Builtin(id) => id,
            Self::Dynamic(id) => Box::leak(format!("{}", id).into_boxed_str()),
        }
    }
}

pub trait ActionHandler: Send + Sync {
    fn execute(&self, input: &str) -> anyhow::Result<()>;
    fn clone_box(&self) -> Box<dyn ActionHandler>;
}

pub trait ContextFilter: Send + Sync {
    fn filter(&self, input: &str) -> bool;
    fn clone_box(&self) -> Box<dyn ContextFilter>;
}

impl<F> ContextFilter for F
where
    F: Fn(&str) -> bool + Send + Sync + Clone + 'static,
{
    fn filter(&self, input: &str) -> bool {
        (*self)(input)
    }

    fn clone_box(&self) -> Box<dyn ContextFilter> {
        Box::new(self.clone())
    }
}

pub trait RenderFn: Send + Sync {
    fn render(&self) -> AnyElement;
    fn clone_box(&self) -> Box<dyn RenderFn + Send + Sync>;
}

impl<F> RenderFn for F
where
    F: Fn() -> AnyElement + Send + Sync + Clone + 'static,
{
    fn render(&self) -> AnyElement {
        self()
    }

    fn clone_box(&self) -> Box<dyn RenderFn + Send + Sync> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn RenderFn + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait ActionDefinition: Send + Sync {
    fn create_action(&self, db: Arc<Database>) -> ActionItem;
    fn get_id(&self) -> ActionId;
    fn get_name(&self) -> String;
}

#[derive(Clone, IntoElement)]
pub struct ActionItem {
    pub id: ActionId,
    pub name: String,
    pub tags: Vec<String>,
    pub function: String,
    pub handler: Box<dyn ActionHandler>,
    pub context_filter: Box<dyn ContextFilter>,
    pub render: Box<dyn RenderFn + Send + Sync>,
    pub relevance: usize,
    pub db: Arc<Database>,
}

impl Eq for ActionItem {}

impl PartialEq for ActionItem {
    fn eq(&self, other: &Self) -> bool {
        self.relevance == other.relevance
    }
}

impl PartialOrd for ActionItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ActionItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.relevance.cmp(&self.relevance)
    }
}

impl RenderOnce for ActionItem {
    fn render(self, _window: &mut gpui::Window, _cx: &mut gpui::App) -> impl IntoElement {
        (self.render).render()
    }
}

impl Clone for Box<dyn ActionHandler> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl Clone for Box<dyn ContextFilter> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl ActionItem {
    pub fn new<H, F, R>(
        id: ActionId,
        name: String,
        tags: Vec<String>,
        function: String,
        handler: H,
        context_filter: F,
        render: R,
        relevance: usize,
        db: Arc<Database>,
    ) -> Self
    where
        H: ActionHandler + 'static,
        F: ContextFilter + 'static,
        R: RenderFn + 'static,
    {
        ActionItem {
            id,
            name,
            tags,
            function,
            handler: Box::new(handler),
            context_filter: Box::new(context_filter),
            render: Box::new(render),
            relevance,
            db,
        }
    }

    const FUZZY_MATCH_THRESHOLD: f64 = 0.8;
    const MAX_LENGTH_RATIO: f64 = 1.5;

    pub fn should_display(&self, input: &str) -> bool {
        if input.is_empty() {
            return true;
        }

        let input_lower = input.to_lowercase();
        let name_lower = self.name.to_lowercase();

        // Exact substring match gets priority
        if name_lower.contains(&input_lower) {
            return true;
        }

        // Check if input is disproportionately long compared to the name
        let length_ratio = input_lower.len() as f64 / name_lower.len() as f64;
        if length_ratio > Self::MAX_LENGTH_RATIO {
            // Skip fuzzy matching if input is too long, but still check other criteria
            let tag_match = self
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&input_lower));
            let function_match = self.function.to_lowercase().contains(&input_lower);
            return tag_match || function_match || self.context_filter.filter(input);
        }

        // For shorter inputs, try fuzzy matching on word boundaries
        let words: Vec<&str> = name_lower.split_whitespace().collect();
        let matches_word_start = words.iter().any(|word| {
            if input_lower.len() <= word.len() {
                let similarity = strsim::jaro_winkler(&input_lower, &word[..input_lower.len()]);
                similarity >= Self::FUZZY_MATCH_THRESHOLD
            } else {
                false
            }
        });

        if matches_word_start {
            return true;
        }

        // If no word-start matches, try full fuzzy match
        let name_similarity = strsim::jaro_winkler(&input_lower, &name_lower);
        if name_similarity >= Self::FUZZY_MATCH_THRESHOLD {
            return true;
        }

        // Fall back to other matching criteria
        let tag_match = self
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(&input_lower));
        let function_match = self.function.to_lowercase().contains(&input_lower);

        tag_match || function_match || self.context_filter.filter(input)
    }

    pub fn execute(&self, input: &str) -> anyhow::Result<()> {
        self.db.log_execution(self.id.as_str())?;
        self.handler.execute(input)
    }
}
