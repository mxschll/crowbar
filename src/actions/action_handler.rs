use crate::action_list_view::ActionListView;
use crate::database::Database;
use gpui::{AnyElement, Context, IntoElement, RenderOnce};
use std::sync::Arc;
use std::usize;

pub trait HandlerFactory {
    fn get_id(&self) -> &'static str;
    fn create_handlers_for_query(
        self: &Self,
        query: &str,
        db: Arc<Database>,
        cx: &mut Context<ActionListView>,
    ) -> Vec<ActionItem>;
}

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
    fn create_action(&self, db: Arc<Database>, cx: &mut Context<ActionListView>) -> ActionItem;
    fn get_id(&self) -> ActionId;
    fn get_name(&self) -> String;

    // Get the relevance score for this action
    fn get_relevance(&self) -> usize {
        0 // Default relevance score
    }
}

#[derive(Clone, IntoElement)]
pub struct ActionItem {
    pub id: ActionId,
    pub handler: Box<dyn ActionHandler>,
    pub render: Box<dyn RenderFn + Send + Sync>,
    pub relevance: usize,
    pub relevance_boost: usize,
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
        other.relevance().cmp(&self.relevance())
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

impl ActionItem {
    pub fn new<H, R>(
        id: ActionId,
        handler: H,
        render: R,
        relevance: usize,
        relevance_boost: usize,
        db: Arc<Database>,
    ) -> Self
    where
        H: ActionHandler + 'static,
        R: RenderFn + 'static,
    {
        ActionItem {
            id,
            handler: Box::new(handler),
            render: Box::new(render),
            relevance,
            relevance_boost,
            db,
        }
    }

    pub fn relevance(&self) -> usize {
        return self.relevance * self.relevance_boost;
    }

    pub fn execute(&self, input: &str) -> anyhow::Result<()> {
        self.db.log_execution(self.id.as_str())?;
        self.handler.execute(input)
    }
}
