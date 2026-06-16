use super::{ComponentStore, Context};
use crate::component::{ComponentNode, ComponentTree};
use crate::data::DataMode;
use crate::RenderLevel;

impl ComponentStore for Context {
    fn register(&mut self, node: ComponentNode) {
        Context::register(self, node);
    }

    fn remove_node(&mut self, id: &str) -> Option<ComponentNode> {
        Context::remove(self, id)
    }

    fn write_data(&mut self, id: &str, mode: DataMode, data: &str) -> bool {
        self.tree.write(id, mode, data)
    }

    fn read_data(&self, id: &str) -> String {
        self.tree.find(id).map(|node| node.render_node(RenderLevel::Detailed)).unwrap_or_default()
    }

    fn find_node(&self, id: &str) -> Option<&ComponentNode> {
        self.tree.find(id)
    }

    fn tree_ref(&self) -> &ComponentTree {
        &self.tree
    }

    fn tree_mut_ref(&mut self) -> &mut ComponentTree {
        &mut self.tree
    }
}
