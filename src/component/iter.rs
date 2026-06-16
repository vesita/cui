//! 全树 DFS 迭代器。

use super::node::ComponentNode;

/// 不可变全树 DFS 迭代器，由 [`super::ComponentTree::iter_all`] 返回。
pub struct AllNodes<'a> {
    pub(super) stack: Vec<&'a ComponentNode>,
}

impl<'a> Iterator for AllNodes<'a> {
    type Item = &'a ComponentNode;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        if let ComponentNode::Composite { children, .. } = node {
            self.stack.extend(children.iter().rev());
        }
        Some(node)
    }
}

/// 可变全树 DFS 迭代器，由 [`super::ComponentTree::iter_all_mut`] 返回。
pub struct AllNodesMut<'a> {
    pub(super) stack: Vec<*mut ComponentNode>,
    pub(super) _marker: std::marker::PhantomData<&'a mut ComponentNode>,
}

impl<'a> Iterator for AllNodesMut<'a> {
    type Item = &'a mut ComponentNode;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = self.stack.pop()?;
        // SAFETY: 栈中指针来源于同一可变借用的 &mut ComponentNode 子节点，
        // 在迭代器生命周期内保证有效。
        let node = unsafe { &mut *ptr };
        if let ComponentNode::Composite { children, .. } = node {
            for child in children.iter_mut().rev() {
                self.stack.push(child as *mut ComponentNode);
            }
        }
        Some(node)
    }
}
