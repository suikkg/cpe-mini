//! 报告：把执行结果落成 rows.jsonl，再渲染成给人看的摘要。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/report/`（`model.rs` 定义 `Row`，`store.rs` 负责落盘，
//! `format.rs` 负责渲染）。真实 `Row` 有 40 多个字段（截图路径、原始日志、
//! ping 四元组、Wi-Fi 上下文……），这里取了其中 13 个，**字段名与真实项目一致**。
//!
//! ## 为什么是 JSONL 而不是一个大 JSON 数组
//!
//! 一行一条记录：跑到一半崩了，已经写下去的行仍然是完整可读的；
//! 追加新行不用重写整个文件；用 `grep` 就能筛。真实项目就是这么存的。
//!
//! ## 这一层刻意不做的事
//!
//! 它**只渲染，不判定**。`verdict` 字段是执行侧算好写进来的，报告层原样显示。
//! 真实项目早期在这里自带过一份回退判定逻辑，和 executor 的优先级对不上，
//! 出过缺陷，后来整个删掉了。

use crate::executor::UnitOutcome;
use crate::reason::ReasonCode;
use crate::verdict::{ExecutionStatus, Verdict};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 报告里的一行 = 一条腿的结果。
///
/// `#[serde(default)]` 是兼容面的基本要求：将来加了字段，历史报告还要读得出来。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Row {
    pub time: String,
    /// 这一行的 id（单元 id + 腿标签）。
    pub task_id: String,
    /// 所属单元的 id。双向单元的两行有同一个 parent_id。
    pub parent_id: String,
    pub task: String,
    pub transport: String,
    /// 展示用的参数摘要，例如 `tcp / bidir / 900 Mbps`。
    pub param: String,
    pub verdict: Verdict,
    pub execution_status: ExecutionStatus,
    pub reason_code: ReasonCode,
    pub reason_detail: String,
    /// **不参与判定**的排障线索。
    pub diagnostics: Vec<String>,
    /// 稳定性轮次。`report::compare` 的对齐键要靠它把第 3 轮和第 4 轮分开。
    pub round: u32,
    /// 有效窗口的接收端 RX 平均，Mbps。`None` = 没形成可信平均。
    pub rx_avg: Option<f64>,
}

/// 把执行结果摊平成行。
pub fn rows_from(outcomes: &[UnitOutcome]) -> Vec<Row> {
    let time = now_stamp();
    let mut rows = Vec::new();

    for out in outcomes {
        for leg in &out.legs {
            // 单向腿的 tag 是空串，这时 task_id 就是单元 id
            let task_id = if leg.tag.is_empty() {
                out.unit_id.clone()
            } else {
                format!("{}#{}", out.unit_id, leg.tag)
            };

            let task = if leg.tag.is_empty() {
                out.title.clone()
            } else {
                format!("{} [{}]", out.title, leg.tag)
            };

            rows.push(Row {
                time: time.clone(),
                task_id,
                parent_id: out.unit_id.clone(),
                task,
                transport: out.transport.clone(),
                param: format!(
                    "{} / {} / {:.0} Mbps",
                    out.transport, out.direction, leg.target_mbps
                ),
                verdict: leg.verdict.verdict,
                execution_status: leg.status,
                reason_code: leg.verdict.code,
                reason_detail: leg.verdict.detail.clone(),
                diagnostics: leg.verdict.diagnostics.clone(),
                round: out.round,
                rx_avg: leg.window.rx_avg,
            });
        }
    }

    rows
}

/// 一行一条，追加写。
pub fn write_jsonl(path: &Path, rows: &[Row]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("创建目录 {} 失败：{e}", dir.display()))?;
        }
    }

    let mut text = String::new();
    for row in rows {
        let line = serde_json::to_string(row).map_err(|e| format!("序列化失败：{e}"))?;
        text.push_str(&line);
        text.push('\n');
    }

    std::fs::write(path, text).map_err(|e| format!("写入 {} 失败：{e}", path.display()))
}

/// 读回来。
///
/// **单行读坏了不让整份报告读不出来** —— 跳过坏行并记下来，这是真实项目
/// 对重放器的要求：宁可少一行，也不要因为一个格式问题让历史报告全军覆没。
pub fn read_jsonl(path: &Path) -> Result<(Vec<Row>, Vec<String>), String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;

    let mut rows = Vec::new();
    let mut skipped = Vec::new();

    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Row>(line) {
            Ok(row) => rows.push(row),
            Err(e) => skipped.push(format!("第 {} 行读不出来：{e}", i + 1)),
        }
    }

    Ok((rows, skipped))
}

/// 一份报告的统计。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    pub total: usize,
    pub pass: usize,
    pub rate_fail: usize,
    pub not_evaluated: usize,
    pub setup_error: usize,
    pub measured: usize,
    pub skip: usize,
}

pub fn tally(rows: &[Row]) -> Tally {
    let mut t = Tally {
        total: rows.len(),
        ..Default::default()
    };
    for row in rows {
        match row.verdict {
            Verdict::Pass => t.pass += 1,
            Verdict::RateFail => t.rate_fail += 1,
            Verdict::NotEvaluated => t.not_evaluated += 1,
            Verdict::SetupError => t.setup_error += 1,
            Verdict::Measured => t.measured += 1,
            Verdict::Skip => t.skip += 1,
        }
    }
    t
}

/// 渲染成终端表格。
pub fn render(rows: &[Row]) -> String {
    let mut out = String::new();
    out.push_str("模式：模拟测试（样本来自计划文件，不起任何进程）\n\n");

    out.push_str(&format!(
        "{}{}{}{}\n",
        pad("测试项", 30),
        pad("实测速率", 14),
        pad("门限", 14),
        "结果"
    ));
    out.push_str(&"─".repeat(66));
    out.push('\n');

    for row in rows {
        let measured = match row.rx_avg {
            Some(v) => format!("{v:.0} Mbps"),
            None => "无有效数据".to_string(),
        };
        // 门限从 param 里取最后一段
        let target = row
            .param
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        out.push_str(&format!(
            "{}{}{}{}\n",
            pad(&row.task, 30),
            pad(&measured, 14),
            pad(&target, 14),
            row.verdict.label()
        ));
    }

    let t = tally(rows);
    out.push('\n');
    out.push_str(&format!(
        "总计 {} 项：通过 {}，未达标 {}，无法评价 {}",
        t.total, t.pass, t.rate_fail, t.not_evaluated
    ));
    if t.setup_error > 0 {
        out.push_str(&format!("，准备失败 {}", t.setup_error));
    }
    if t.measured > 0 {
        out.push_str(&format!("，仅记录 {}", t.measured));
    }
    out.push('\n');

    // 处置建议：只对真正需要动手的行给。
    let mut advices: Vec<String> = Vec::new();
    for row in rows {
        if let Some(advice) = row.reason_code.disposition_advice() {
            let line = format!("  {} → {}", row.task, advice);
            if !advices.contains(&line) {
                advices.push(line);
            }
        }
    }
    if !advices.is_empty() {
        out.push_str("\n接下来该做什么：\n");
        for a in advices {
            out.push_str(&a);
            out.push('\n');
        }
    }

    out
}

/// 按显示宽度补空格。中文字符占 2 列，直接用 `len()` 会排歪。
fn pad(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w >= width {
        format!("{s} ")
    } else {
        format!("{}{}", s, " ".repeat(width - w))
    }
}

fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| if (c as u32) > 0x2000 { 2 } else { 1 })
        .sum()
}

fn now_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
// 测试名用中文是为了让失败信息直接说清楚"哪条规则被破坏了"。
// 中文名里夹着 PASS / AB / JSON 这类大写 ASCII 会触发 non_snake_case，
// 这里按模块限定地关掉——allow 要贴在最小范围上并写明理由，
// 不要图省事加在 crate 根上。
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::builder::build_units;
    use crate::executor::execute_plan;
    use crate::plan::{Plan, Spec};

    fn 样例计划() -> Plan {
        Plan {
            plan_id: "t".into(),
            rounds: 1,
            warmup_secs: 2,
            specs: vec![
                Spec {
                    title: "TCP-A".into(),
                    transport: "tcp".into(),
                    direction: "ab".into(),
                    target_mbps: 900.0,
                    samples_ab: vec![100.0, 500.0, 950.0, 960.0, 940.0, 950.0],
                    samples_ba: vec![],
                },
                Spec {
                    title: "TCP-B".into(),
                    transport: "tcp".into(),
                    direction: "ab".into(),
                    target_mbps: 900.0,
                    samples_ab: vec![100.0, 300.0, 850.0, 840.0, 860.0, 850.0],
                    samples_ba: vec![],
                },
                Spec {
                    title: "TCP-C".into(),
                    transport: "tcp".into(),
                    direction: "ab".into(),
                    target_mbps: 900.0,
                    samples_ab: vec![],
                    samples_ba: vec![],
                },
            ],
        }
    }

    fn 样例行() -> Vec<Row> {
        let plan = 样例计划();
        rows_from(&execute_plan(&build_units(&plan)))
    }

    #[test]
    fn 三条规格产出三行() {
        assert_eq!(样例行().len(), 3);
    }

    #[test]
    fn 统计对得上() {
        let t = tally(&样例行());
        assert_eq!(t.total, 3);
        assert_eq!(t.pass, 1);
        assert_eq!(t.rate_fail, 1);
        assert_eq!(t.not_evaluated, 1);
    }

    #[test]
    fn 存盘再读回来统计一致() {
        let rows = 样例行();
        let path = std::env::temp_dir().join("cpe-mini-test-rows.jsonl");

        write_jsonl(&path, &rows).unwrap();
        let (loaded, skipped) = read_jsonl(&path).unwrap();

        assert!(skipped.is_empty());
        assert_eq!(loaded.len(), rows.len());
        assert_eq!(tally(&loaded), tally(&rows));
        // 判定是按 label 字符串存的，读回来必须还是同一个枚举值
        assert_eq!(loaded[0].verdict, rows[0].verdict);
        assert_eq!(loaded[1].reason_code, ReasonCode::RxBelowTarget);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn 判定在文件里是大写下划线串() {
        let rows = 样例行();
        let line = serde_json::to_string(&rows[0]).unwrap();
        assert!(line.contains("\"PASS\""), "实际是：{line}");
        assert!(line.contains("\"RX_TARGET_MET\""));
        // 不能是 serde 默认的驼峰
        assert!(!line.contains("\"RateFail\""));
    }

    #[test]
    fn 坏行被跳过但其余照常读出() {
        let path = std::env::temp_dir().join("cpe-mini-test-broken.jsonl");
        let rows = 样例行();
        let good = serde_json::to_string(&rows[0]).unwrap();
        std::fs::write(&path, format!("{good}\n这不是 json\n{good}\n")).unwrap();

        let (loaded, skipped) = read_jsonl(&path).unwrap();
        assert_eq!(loaded.len(), 2, "坏行不该让整份报告读不出来");
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].contains("第 2 行"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn 渲染里三种结果都看得见() {
        let text = render(&样例行());
        assert!(text.contains("PASS"));
        assert!(text.contains("RATE_FAIL"));
        assert!(text.contains("NOT_EVALUATED"));
        assert!(text.contains("无有效数据"));
        assert!(text.contains("总计 3 项"));
    }
}
