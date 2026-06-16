//! 组件树快照 —— 用于调试和测试的序列化格式。

/// 组件树快照 —— 用于调试和测试的序列化格式。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct TreeSnapshot {
    pub roots: Vec<NodeSnapshot>,
    pub stats: TreeStats,
}

/// 节点快照。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct NodeSnapshot {
    pub id: String,
    pub title: String,
    pub level: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub priority: String,
    pub is_static: bool,
    pub is_inert: bool,
    pub is_dirty: bool,
    pub children: Vec<NodeSnapshot>,
}

/// 树统计信息。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TreeStats {
    pub total_nodes: usize,
    pub leaf_nodes: usize,
    pub composite_nodes: usize,
    pub hidden_nodes: usize,
    pub dirty_nodes: usize,
}
