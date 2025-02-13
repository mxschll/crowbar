use gpui::{AnyElement, IntoElement, RenderOnce};

pub trait ActionHandler: Send + Sync {
    fn execute(&self, input: &str) -> Result<(), String>;
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

#[derive(Clone, IntoElement)]
pub struct ActionItem {
    pub name: String,
    pub tags: Vec<String>,
    pub function: String,
    pub handler: Box<dyn ActionHandler>,
    pub context_filter: Box<dyn ContextFilter>,
    pub render: Box<dyn RenderFn + Send + Sync>,
    relevance: usize,
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
        name: String,
        tags: Vec<String>,
        function: String,
        handler: H,
        context_filter: F,
        render: R,
    ) -> Self
    where
        H: ActionHandler + 'static,
        F: ContextFilter + 'static,
        R: RenderFn + 'static,
    {
        ActionItem {
            name,
            tags,
            function,
            handler: Box::new(handler),
            context_filter: Box::new(context_filter),
            render: Box::new(render),
            relevance: 0,
        }
    }

    pub fn should_display(&self, input: &str) -> bool {
        let name_match = self.name.to_lowercase().contains(&input.to_lowercase());
        let tag_match = self
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(&input.to_lowercase()));
        let function_match = self.function.to_lowercase().contains(&input.to_lowercase());

        name_match || tag_match || function_match || self.context_filter.filter(input)
    }

    pub fn execute(&self, input: &str) -> Result<(), String> {
        self.handler.execute(input)
    }
}
