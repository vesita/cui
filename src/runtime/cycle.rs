//! RenderCycle 状态机 —— 三阶段渲染周期。
//!
//! 状态流：Idle → Preparing → Rendering → Committing → Idle
//! Abort 可在 Preparing 或 Rendering 时安全回到 Idle。

use state_macro::{states, transitions};

states! {
    enum RenderCycle {
        Idle,
        Preparing,
        Rendering,
        Committing,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prepare;
#[derive(Clone, Debug, PartialEq)]
pub struct DoRenderPlan;
#[derive(Clone, Debug, PartialEq)]
pub struct CommitMsg;
#[derive(Clone, Debug, PartialEq)]
pub struct Abort;

transitions!(RenderCycle, mut, [
    (Idle, Prepare) => Preparing,
    (Preparing, DoRenderPlan) => Rendering,
    (Rendering, CommitMsg) => Idle,
    (Preparing, Abort) => Idle,
    (Rendering, Abort) => Idle,
]);

impl Idle {
    pub fn on_prepare(self, _: Prepare) -> Preparing {
        Preparing {}
    }
}
impl Preparing {
    pub fn on_do_render_plan(self, _: DoRenderPlan) -> Rendering {
        Rendering {}
    }
    pub fn on_abort(self, _: Abort) -> Idle {
        Idle {}
    }
}
impl Rendering {
    pub fn on_commit_msg(self, _: CommitMsg) -> Idle {
        Idle {}
    }
    pub fn on_abort(self, _: Abort) -> Idle {
        Idle {}
    }
}
