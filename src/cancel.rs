//! 取消信号：让一轮正在跑的测试停下来。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/cancel.rs`。真实项目那边是四个全局
//! `AtomicBool`（`RUN_CANCELLED` / `PROCESS_SHUTDOWN_REQUESTED` /
//! `STOP_REQUESTED` / `SKIP_CURRENT_UNIT`）加一个 Ctrl+C 处理器；
//! 这里只留最核心的一个，但**语义和检查时机完全一致**。
//!
//! ## 为什么是 `Arc<AtomicBool>` 而不是 `&mut bool`
//!
//! 因为置位的人和读取的人不在同一个线程里：
//!
//! ```text
//!   Ctrl+C 处理器 ─┐
//!   Web 界面「停止」─┼─→  AtomicBool  ←─ 执行循环每跑完一个单元读一次
//!   超时看门狗     ─┘
//! ```
//!
//! `&mut bool` 同一时刻只能有一个持有者，这个形状根本写不出来。
//! `Arc` 负责「多个地方都持有」，`AtomicBool` 负责「同时读写不算数据竞争」。
//!
//! ## 为什么只在单元边界检查
//!
//! 不是省事，是收尾要跑完。真实项目在一个单元结束时要停掉对端作业、
//! 回收端口、收日志——这条路径已经被证明能干净地收场。在半路上硬停，
//! 这些都不会发生，下一次跑就会撞上「端口被占」。
//!
//! **取消是「跑完手头这个就停」，不是「立刻断电」。**

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// 一个可以跨线程共享的「停下来」信号。
///
/// `clone()` 出来的副本和原件是**同一个**标志，不是复制一份：
///
/// ```
/// use cpe_mini::cancel::CancelFlag;
///
/// let flag = CancelFlag::new();
/// let handle = flag.clone();
/// std::thread::spawn(move || handle.request_cancel()).join().unwrap();
/// assert!(flag.is_cancelled());
/// ```
#[derive(Debug, Clone, Default)]
pub struct CancelFlag {
    flag: Arc<AtomicBool>,
    /// 教学用的自动扳机：轮询到第 n 次时自动置位。
    ///
    /// 真实项目**没有**这个字段——那边置位的是 Ctrl+C 处理器和 Web 界面的
    /// 「停止」按钮，都在别的线程里，什么时候按下取决于操作员。
    ///
    /// 这里要的是**可复现**：同一份计划、同一个 n，报告永远一模一样。
    /// 靠 `thread::sleep` 定时去触发的演示，在慢机器上会停在不同的单元上，
    /// 那种「跑两次不一样」的例子教不了东西。
    trip_after: Option<usize>,
    polls: Arc<AtomicUsize>,
}

impl CancelFlag {
    /// 一个永远不会置位的标志。
    pub fn new() -> Self {
        Self::default()
    }

    /// 演示/测试用：执行循环第 `n` 次检查时自动置位。
    ///
    /// `n = 0` 表示还没开跑就已经取消了。
    pub fn trip_after_polls(n: usize) -> Self {
        Self {
            trip_after: Some(n),
            ..Self::default()
        }
    }

    /// 请求停下来。任何线程都可以调，调多少次都一样。
    pub fn request_cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// 现在是不是已经被要求停下来了。
    ///
    /// 注意这个方法**有副作用**（推进轮询计数），所以执行循环里每个单元
    /// 只该问一次。真实项目那边是纯读，没有这个顾虑。
    pub fn is_cancelled(&self) -> bool {
        let seen = self.polls.fetch_add(1, Ordering::SeqCst);
        if let Some(n) = self.trip_after {
            if seen >= n {
                self.request_cancel();
            }
        }
        self.flag.load(Ordering::SeqCst)
    }

    /// 不推进计数的读法，给断言和日志用。
    pub fn peek(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
// 测试名用中文是为了让失败信息直接说清楚"哪条规则被破坏了"。
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn 默认不取消() {
        let flag = CancelFlag::new();
        assert!(!flag.is_cancelled());
        assert!(!flag.is_cancelled());
    }

    #[test]
    fn 别的线程置位主线程看得见() {
        let flag = CancelFlag::new();
        let handle = flag.clone();
        std::thread::spawn(move || handle.request_cancel())
            .join()
            .unwrap();
        // clone 出来的是同一个标志，不是副本——这正是 Arc 的作用
        assert!(flag.peek());
    }

    #[test]
    fn 扳机在第n次轮询生效() {
        let flag = CancelFlag::trip_after_polls(2);
        assert!(!flag.is_cancelled()); // 第 0 次
        assert!(!flag.is_cancelled()); // 第 1 次
        assert!(flag.is_cancelled()); // 第 2 次，扳机
        assert!(flag.is_cancelled()); // 置位之后一直是真
    }

    #[test]
    fn 扳机为零表示开跑前就取消了() {
        let flag = CancelFlag::trip_after_polls(0);
        assert!(flag.is_cancelled());
    }

    #[test]
    fn 取消是不可逆的() {
        let flag = CancelFlag::new();
        flag.request_cancel();
        flag.request_cancel();
        assert!(flag.peek());
    }
}
