//! 两轮运行的对比：这次比上次差在哪。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/report/compare.rs`。`UnitDelta`、`UnitSnapshot`、
//! `DeltaKind`、`RunComparison`、`compare`、`SIGNIFICANT_RATE_CHANGE` 六个名字
//! 和真实项目完全一致，`DeltaKind` 的六个变体和排序语义也照抄。
//!
//! ## 为什么存在
//!
//! 「新固件比旧固件掉了多少」是版本验收唯一要回答的问题。在此之前它只能靠
//! 开两个窗口、人眼比对几百行 —— 一次回归跑完十几个小时，结论却卡在这一步。
//!
//! 数据侧的前提本来就齐了：`rows.jsonl` 是结构化的。缺的只是一个消费它们的出口。
//!
//! ## 对齐键：这一课真正的难点
//!
//! 两份报告怎么知道哪一行对哪一行？
//!
//! **不能用行号（第 1 行对第 1 行）**：两轮之间只要少跑或多跑一条，后面全体
//! 错位，于是每一行都被报成「变了」，而实际什么都没变。
//!
//! **也不能用 [`crate::report::Row::task_id`]**，尽管它看起来正好合适。它长
//! 这样：`{plan_id}-{specs 下标}-r{轮次}`。两个问题：
//!
//! - `plan_id` 改一个字（换个计划名）→ 两轮一条都对不上
//! - 在 `specs` 中间插一条新规格 → 后面所有下标位移 → 全部报成「新增 + 缺失」
//!
//! 真实项目踩的是同一个坑的另一个版本：它的 `Unit.id` 把网卡**协商速率**算进了
//! 身份，Wi-Fi 一重协商，同一条测试在两轮里就成了两个 ID，整张表变成
//! 「全部新增 + 全部缺失」。这是拿真实历史数据跑出来的。
//!
//! 所以这里自己拼一把**只含不变条件**的键，见 [`comparison_key`]。它回答的是
//! 「这是不是同一个测试」，而不是「这一轮的条件是不是和上一轮完全一样」。
//!
//! ## 判定翻转比数字变化更重要
//!
//! 一条从 950 掉到 902 的链路（门限 900）和一条从 910 掉到 890 的，数字上差得
//! 差不多，但后者跨过了门限。所以排序按**先翻转、后跌幅**。

use crate::report::Row;
use crate::verdict::Verdict;
use std::collections::BTreeMap;

/// 一轮里某一条的结论。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitSnapshot {
    pub verdict: Verdict,
    pub rx_avg: Option<f64>,
    pub target_mbps: Option<f64>,
}

/// 一条测试在两轮之间的变化。
#[derive(Debug, Clone, PartialEq)]
pub struct UnitDelta {
    /// 两轮对齐用的键，见 [`comparison_key`]。**不是** `task_id`。
    pub unit_id: String,
    /// 展示标题，优先取新的那一轮。
    pub title: String,
    pub before: Option<UnitSnapshot>,
    pub after: Option<UnitSnapshot>,
}

/// 这一条在两轮之间**发生了什么**。
///
/// 顺序就是严重程度：判定翻坏 → 掉速 → 消失 → 新增 → 提升 → 没变。
/// 报告按它排序，读的人从上往下看就是「先处理最要紧的」。
///
/// `Ord` 是 derive 出来的，按变体的声明顺序比大小 —— 所以**调整变体顺序会直接
/// 改变报告的排序**，不是无害的重排。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeltaKind {
    /// 上一轮 PASS，这一轮不是了。**回归测试要找的就是这一类。**
    Regressed,
    /// 判定没变，但接收速率明显下降。
    SlowerButStillSameVerdict,
    /// 这一轮没有这一条（计划改了，或者跑到一半停了）。
    Disappeared,
    /// 上一轮没有这一条。
    Added,
    /// 上一轮不是 PASS，这一轮是了。
    Fixed,
    /// 判定和速率都没有实质变化。
    Unchanged,
}

impl DeltaKind {
    pub fn label(self) -> &'static str {
        match self {
            DeltaKind::Regressed => "判定变坏",
            DeltaKind::SlowerButStillSameVerdict => "速率下降",
            DeltaKind::Disappeared => "本轮缺失",
            DeltaKind::Added => "本轮新增",
            DeltaKind::Fixed => "判定转好",
            DeltaKind::Unchanged => "无实质变化",
        }
    }
}

/// 速率变化到多少才算「明显」。
///
/// 5% 是测量噪声之上的一条线：同一条链路连跑两次，网卡计数器口径下的差异通常
/// 在 1~2%。取 5% 是为了让这张表**不刷屏** —— 把每一次 0.3% 的抖动都报成
/// 「下降」，等于没有这张表。真要看逐条数字，表里每一行都带着两轮的原值。
pub const SIGNIFICANT_RATE_CHANGE: f64 = 0.05;

impl UnitDelta {
    /// 相对变化率（`after / before - 1`）。任一轮没有速率时为 `None`。
    pub fn rate_change(&self) -> Option<f64> {
        let before = self.before?.rx_avg?;
        let after = self.after?.rx_avg?;
        // 上一轮是 0 或者非有限数，算不出有意义的百分比
        if !before.is_finite() || !after.is_finite() || before <= 0.0 {
            return None;
        }
        Some(after / before - 1.0)
    }

    pub fn kind(&self) -> DeltaKind {
        let (Some(before), Some(after)) = (self.before, self.after) else {
            return if self.after.is_some() {
                DeltaKind::Added
            } else {
                DeltaKind::Disappeared
            };
        };

        // 判定翻转优先于数字：跨过门限和没跨过，是两件不同性质的事。
        if before.verdict == Verdict::Pass && after.verdict != Verdict::Pass {
            return DeltaKind::Regressed;
        }
        if before.verdict != Verdict::Pass && after.verdict == Verdict::Pass {
            return DeltaKind::Fixed;
        }

        match self.rate_change() {
            Some(change) if change <= -SIGNIFICANT_RATE_CHANGE => {
                DeltaKind::SlowerButStillSameVerdict
            }
            _ => DeltaKind::Unchanged,
        }
    }

    /// 排序键：先按严重程度，同级里跌得多的排前面。
    fn sort_key(&self) -> (DeltaKind, i64, String) {
        let change = self.rate_change().unwrap_or(0.0);
        // 浮点不能直接进排序键（`f64` 没有 `Ord`，因为 NaN 和谁都比不出大小）。
        // 放大成整数，跌幅越大（负得越多）越靠前。
        let scaled = (change * 10_000.0).round().clamp(-1e9, 1e9) as i64;
        (self.kind(), scaled, self.unit_id.clone())
    }
}

/// 一次对比的完整结果。
#[derive(Debug, Clone, Default)]
pub struct RunComparison {
    pub deltas: Vec<UnitDelta>,
    /// 两轮跑的是不是同一份计划。
    ///
    /// **不一致不是错误**，只是意味着「不能比总数，只能逐条看」：计划变了以后，
    /// 「新增」和「缺失」说的是计划差异而不是设备表现。报告顶部必须把这句话说
    /// 出来，否则读的人会把计划差异当成回归。
    ///
    /// 真实项目靠 `Row` 上的 `plan_hash` 字段判断，这里没有那个字段，由调用方
    /// 给一个近似值（见 `main.rs` 的 `compare` 分支）。
    pub same_plan: bool,
}

impl RunComparison {
    pub fn count(&self, kind: DeltaKind) -> usize {
        self.deltas.iter().filter(|d| d.kind() == kind).count()
    }

    /// 有没有值得拦下来的变化（判定变坏或明显掉速）。给退出码用。
    pub fn has_regression(&self) -> bool {
        self.count(DeltaKind::Regressed) > 0 || self.count(DeltaKind::SlowerButStillSameVerdict) > 0
    }
}

/// 两轮对齐用的键：**只含不变条件**。
///
/// 取 `task`（标题 + 腿标签）、`transport`、`round` 三样。
///
/// 刻意**不含门限**：门限从 900 提到 950 的时候，我们要看到的是
/// 「同一条测试，门限变了所以判定变了」，而不是「旧的那条消失了 + 新增一条」。
/// 真实项目把下发参数算进了键，因为它那边「参数变了就是另一个测试」；
/// 这里规模小，看得到门限的变化更有用。**这是一个有取舍的选择，不是唯一解。**
pub fn comparison_key(row: &Row) -> String {
    format!("{}|{}|r{}", row.task, row.transport, row.round)
}

fn snapshot(row: &Row) -> UnitSnapshot {
    UnitSnapshot {
        verdict: row.verdict,
        rx_avg: row.rx_avg,
        // param 的最后一段是「900 Mbps」，取数字部分；解析不出来就当没有门限
        target_mbps: row
            .param
            .rsplit('/')
            .next()
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<f64>().ok()),
    }
}

/// 对比两份报告。
///
/// `same_plan` 由调用方给：两轮跑的是不是同一份计划。
pub fn compare(before: &[Row], after: &[Row], same_plan: bool) -> RunComparison {
    // BTreeMap 而不是 HashMap：键有序，同分的行在报告里顺序稳定，
    // 两次跑同样的输入得到同样的输出（对比报告要能进 diff）。
    let mut map: BTreeMap<String, UnitDelta> = BTreeMap::new();

    for row in before {
        let key = comparison_key(row);
        map.entry(key.clone())
            .or_insert_with(|| UnitDelta {
                unit_id: key,
                title: row.task.clone(),
                before: None,
                after: None,
            })
            .before = Some(snapshot(row));
    }

    for row in after {
        let key = comparison_key(row);
        let delta = map.entry(key.clone()).or_insert_with(|| UnitDelta {
            unit_id: key,
            title: row.task.clone(),
            before: None,
            after: None,
        });
        // 标题优先取新的那一轮：改过文案的话，用户更认得出新的那个
        delta.title = row.task.clone();
        delta.after = Some(snapshot(row));
    }

    let mut deltas: Vec<UnitDelta> = map.into_values().collect();
    deltas.sort_by_key(|d| d.sort_key());

    RunComparison { deltas, same_plan }
}

/// 渲染成终端表格。
pub fn render(diff: &RunComparison) -> String {
    let mut out = String::from("两轮对比\n\n");

    if !diff.same_plan {
        // 这句话必须在最上面。少了它，读的人会把计划差异当成设备回归。
        out.push_str(
            "注意：两轮跑的不是同一份计划，「新增」和「缺失」说的是计划差异，不是设备表现。\n\n",
        );
    }

    out.push_str(&format!(
        "{}{}{}{}\n",
        pad("变化", 12),
        pad("测试项", 26),
        pad("上一轮", 20),
        "这一轮"
    ));
    out.push_str(&"─".repeat(72));
    out.push('\n');

    for d in &diff.deltas {
        out.push_str(&format!(
            "{}{}{}{}\n",
            pad(d.kind().label(), 12),
            pad(&d.title, 26),
            pad(&side(d.before.as_ref()), 20),
            side(d.after.as_ref())
        ));
        if let Some(change) = d.rate_change() {
            if change.abs() >= SIGNIFICANT_RATE_CHANGE {
                out.push_str(&format!("{}速率 {:+.1}%\n", " ".repeat(12), change * 100.0));
            }
        }
    }

    out.push('\n');
    out.push_str(&format!(
        "共 {} 条：判定变坏 {}，速率下降 {}，判定转好 {}，无实质变化 {}",
        diff.deltas.len(),
        diff.count(DeltaKind::Regressed),
        diff.count(DeltaKind::SlowerButStillSameVerdict),
        diff.count(DeltaKind::Fixed),
        diff.count(DeltaKind::Unchanged),
    ));
    let added = diff.count(DeltaKind::Added);
    let gone = diff.count(DeltaKind::Disappeared);
    if added > 0 || gone > 0 {
        out.push_str(&format!("，新增 {added}，缺失 {gone}"));
    }
    out.push('\n');

    if diff.has_regression() {
        out.push_str("\n有回归，别放行。\n");
    }

    out
}

fn side(snap: Option<&UnitSnapshot>) -> String {
    match snap {
        None => "—".to_string(),
        Some(s) => match s.rx_avg {
            Some(v) => format!("{} {:.0}", s.verdict.label(), v),
            None => s.verdict.label().to_string(),
        },
    }
}

/// 按显示宽度补空格。中文字符占 2 列，直接用 `len()` 会排歪。
fn pad(s: &str, width: usize) -> String {
    let w: usize = s
        .chars()
        .map(|c| if (c as u32) > 0x2000 { 2 } else { 1 })
        .sum();
    if w >= width {
        format!("{s} ")
    } else {
        format!("{}{}", s, " ".repeat(width - w))
    }
}

#[cfg(test)]
// 测试名用中文是为了让失败信息直接说清楚"哪条规则被破坏了"。
// 中文名里夹着 PASS / AB / JSON 这类大写 ASCII 会触发 non_snake_case，
// 这里按模块限定地关掉——allow 要贴在最小范围上并写明理由，
// 不要图省事加在 crate 根上。
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::reason::ReasonCode;
    use crate::verdict::ExecutionStatus;

    fn 行(task: &str, verdict: Verdict, rx: Option<f64>, target: f64) -> Row {
        Row {
            time: "0".into(),
            task_id: format!("plan-0-r1#{task}"),
            parent_id: "plan-0-r1".into(),
            task: task.into(),
            transport: "tcp".into(),
            param: format!("tcp / ab / {target:.0} Mbps"),
            verdict,
            execution_status: ExecutionStatus::Completed,
            reason_code: ReasonCode::None,
            reason_detail: String::new(),
            diagnostics: Vec::new(),
            round: 1,
            rx_avg: rx,
            udp_loss: None,
        }
    }

    #[test]
    fn 判定变坏排在最前面() {
        let before = vec![
            行("A", Verdict::Pass, Some(950.0), 900.0),
            行("B", Verdict::Pass, Some(950.0), 900.0),
        ];
        let after = vec![
            // A 只是小幅波动
            行("A", Verdict::Pass, Some(940.0), 900.0),
            // B 掉到门限以下
            行("B", Verdict::RateFail, Some(880.0), 900.0),
        ];

        let diff = compare(&before, &after, true);
        assert_eq!(diff.deltas.len(), 2);
        assert_eq!(diff.deltas[0].title, "B", "判定变坏的必须排第一");
        assert_eq!(diff.deltas[0].kind(), DeltaKind::Regressed);
        assert_eq!(diff.deltas[1].kind(), DeltaKind::Unchanged);
        assert!(diff.has_regression());
    }

    #[test]
    fn 掉速但没跨过门限算速率下降() {
        // 950 → 890 是 -6.3%，超过 5% 的线，但门限是 500，两轮都 PASS
        let before = vec![行("A", Verdict::Pass, Some(950.0), 500.0)];
        let after = vec![行("A", Verdict::Pass, Some(890.0), 500.0)];

        let diff = compare(&before, &after, true);
        assert_eq!(diff.deltas[0].kind(), DeltaKind::SlowerButStillSameVerdict);
        assert!(diff.has_regression(), "明显掉速也要拦");
    }

    #[test]
    fn 小幅波动不报() {
        // -2%，在噪声范围内
        let before = vec![行("A", Verdict::Pass, Some(950.0), 900.0)];
        let after = vec![行("A", Verdict::Pass, Some(931.0), 900.0)];

        let diff = compare(&before, &after, true);
        assert_eq!(
            diff.deltas[0].kind(),
            DeltaKind::Unchanged,
            "2% 的抖动报成「下降」的话，这张表就没法看了"
        );
        assert!(!diff.has_regression());
    }

    #[test]
    fn 转好和新增和缺失() {
        let before = vec![
            行("A", Verdict::RateFail, Some(880.0), 900.0),
            行("只在上一轮", Verdict::Pass, Some(950.0), 900.0),
        ];
        let after = vec![
            行("A", Verdict::Pass, Some(910.0), 900.0),
            行("只在这一轮", Verdict::Pass, Some(950.0), 900.0),
        ];

        let diff = compare(&before, &after, false);
        assert_eq!(diff.count(DeltaKind::Fixed), 1);
        assert_eq!(diff.count(DeltaKind::Added), 1);
        assert_eq!(diff.count(DeltaKind::Disappeared), 1);
        // 转好和增删都不算回归
        assert!(!diff.has_regression());
    }

    #[test]
    fn 门限变了仍然对得上() {
        // 同一条测试，门限从 900 提到 950。对齐键里不含门限，
        // 所以这里要看到「判定变坏」，而不是「缺失 + 新增」。
        let before = vec![行("A", Verdict::Pass, Some(930.0), 900.0)];
        let after = vec![行("A", Verdict::RateFail, Some(930.0), 950.0)];

        let diff = compare(&before, &after, true);
        assert_eq!(diff.deltas.len(), 1, "不该拆成两条");
        assert_eq!(diff.deltas[0].kind(), DeltaKind::Regressed);
        assert_eq!(diff.deltas[0].before.unwrap().target_mbps, Some(900.0));
        assert_eq!(diff.deltas[0].after.unwrap().target_mbps, Some(950.0));
    }

    #[test]
    fn 不同轮次不会被混在一起() {
        let mut r2 = 行("A", Verdict::Pass, Some(950.0), 900.0);
        r2.round = 2;
        let before = vec![行("A", Verdict::Pass, Some(950.0), 900.0), r2.clone()];
        let after = vec![行("A", Verdict::Pass, Some(950.0), 900.0), r2];

        let diff = compare(&before, &after, true);
        assert_eq!(diff.deltas.len(), 2, "第 1 轮和第 2 轮是两条，不能合并");
    }

    #[test]
    fn 没有速率时算不出变化率() {
        let before = vec![行("A", Verdict::NotEvaluated, None, 900.0)];
        let after = vec![行("A", Verdict::Pass, Some(950.0), 900.0)];

        let diff = compare(&before, &after, true);
        assert_eq!(
            diff.deltas[0].rate_change(),
            None,
            "上一轮没数，除不出百分比"
        );
        // 但判定翻转照样看得出来
        assert_eq!(diff.deltas[0].kind(), DeltaKind::Fixed);
    }

    #[test]
    fn 计划不同要在报告里说出来() {
        let rows = vec![行("A", Verdict::Pass, Some(950.0), 900.0)];
        let text = render(&compare(&rows, &rows, false));
        assert!(
            text.contains("不是同一份计划"),
            "计划不同却不提示，读的人会把计划差异当成回归"
        );

        let text2 = render(&compare(&rows, &rows, true));
        assert!(!text2.contains("不是同一份计划"));
    }
}
