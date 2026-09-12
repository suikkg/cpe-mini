//! cpe-mini：cpe-test 的教学缩微版。
//!
//! 完整链路，和真实项目同一个形状：
//!
//! ```text
//! plan.json
//!   → plan::load        读配置 + 校验
//!   → builder::build_units   展开成测试单元
//!   → executor::execute_plan 执行（教学版用固定样本）
//!       └─ rate_window   算有效窗口 RX 平均
//!       └─ verdict       判定 + 聚合
//!   → report             落 rows.jsonl + 渲染摘要
//! ```
//!
//! 报告存下来之后还有两个出口，都只读 `rows.jsonl`、不碰前面的链路：
//!
//! ```text
//! rows.jsonl → compare   两轮对比，回归测试要的那张表
//! rows.jsonl → report::to_csv   导出给 Excel
//! ```
//!
//! 真实项目对应：`src/master/plan.rs` → `src/master/builder.rs` →
//! `src/master/executor/` → `src/master/rate_window.rs` → `src/verdict.rs` →
//! `src/report/`。

pub mod builder;
pub mod compare;
pub mod executor;
pub mod plan;
pub mod rate_window;
pub mod reason;
pub mod report;
pub mod verdict;

use std::path::{Path, PathBuf};

/// 内置样例计划的位置。
pub fn demo_plan_path() -> PathBuf {
    PathBuf::from("fixtures/plan.json")
}

/// 默认报告落盘位置。
pub fn default_output() -> PathBuf {
    PathBuf::from("output/rows.jsonl")
}

/// 读计划 → 展开 → 执行 → 出行。一条龙。
pub fn run_plan(path: &Path) -> Result<Vec<report::Row>, String> {
    let plan = plan::load(path)?;
    let units = builder::build_units(&plan);
    let outcomes = executor::execute_plan(&units);
    Ok(report::rows_from(&outcomes))
}

/// 只展开不执行，用来预览计划会跑成什么样。
pub fn preview_plan(path: &Path) -> Result<String, String> {
    let plan = plan::load(path)?;
    let units = builder::build_units(&plan);

    let mut out = format!(
        "计划 {}：{} 条规格 × {} 轮 = {} 个测试单元\n\n",
        plan.plan_id,
        plan.specs.len(),
        plan.rounds,
        units.len()
    );

    for u in &units {
        out.push_str(&format!("{}  [{}]  {} 腿\n", u.id, u.title, u.legs.len()));
        for leg in &u.legs {
            let tag = if leg.tag.is_empty() {
                "单向"
            } else {
                &leg.tag
            };
            out.push_str(&format!(
                "    {tag}  门限 {:.0} Mbps  样本 {} 秒\n",
                leg.target_mbps,
                leg.samples.len()
            ));
        }
    }

    Ok(out)
}
