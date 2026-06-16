use super::{Context, Renderer};
use crate::render::RenderStats;

impl Renderer for Context {
    fn render(&mut self) -> String {
        Context::render(self)
    }

    fn last_render_stats(&self) -> Option<&RenderStats> {
        Context::last_render_stats(self)
    }
}
