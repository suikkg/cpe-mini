//! 判定词汇表：结果分级、执行状态、聚合规则。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/verdict.rs`。`Verdict`、`ExecutionStatus`、
//! `VerdictResult`、`aggregate_verdict` 四个名字和它们的字符串形式与真实项目
//! 一致；`aggregate_verdict` 的优先级顺序也照抄，只省掉了两条需要完整原因码
//! 表才能实现的特例（见函数上的注释）。
//!
//! ## 为什么判定要单独成一个模块
//!
//! 真实项目的注释记着一段教训：判定规则曾经在 `executor`（执行侧聚合）和
//! `report`（报告侧回退聚合）各实现一遍，两份实现的优先级不一致，先后产生过
//! 两个真实缺陷——概览把硬失败显示成 `NOT_EVALUATED`，以及把环境异常写成了
//! CPE 性能失败。所以词汇表和聚合规则收敛到这一个模块，别处只许调用。

use crate::reason::ReasonCode;

/// 单条测试腿的结果分级。
///
/// 顺序**不代表**优先级，优先级由 [`aggregate_verdict`] 单独定义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verdict {
    /// 达标。
    Pass,
    /// 没到门限——唯一表示「CPE 性能不行」的取值。
    RateFail,
    /// 测到了数，但这条测试没有门限，不做达标判断。
    Measured,
    /// 无法评价：没有可信的测量值。**不是失败**。
    #[default]
    NotEvaluated,
    /// 准备阶段就出错了，压根没跑起来。
    SetupError,
    /// 跳过。
    Skip,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::RateFail => "RATE_FAIL",
            Verdict::Measured => "MEASURED",
            Verdict::NotEvaluated => "NOT_EVALUATED",
            Verdict::SetupError => "SETUP_ERROR",
            Verdict::Skip => "SKIP",
        }
    }

    /// `label()` 的逆。
    ///
    /// 存在的理由：判定在 rows.jsonl 里是按 **label 字符串**序列化的，读回来时
    /// 需要一条唯一的反向映射。写第二份 `match` 就是又一个会漂的口径。
    pub fn from_label(label: &str) -> Option<Verdict> {
        Some(match label {
            "PASS" => Verdict::Pass,
            "RATE_FAIL" => Verdict::RateFail,
            "MEASURED" => Verdict::Measured,
            "NOT_EVALUATED" => Verdict::NotEvaluated,
            "SETUP_ERROR" => Verdict::SetupError,
            "SKIP" => Verdict::Skip,
            _ => return None,
        })
    }

    pub fn is_pass(self) -> bool {
        self == Verdict::Pass
    }
}

/// 执行过程本身的完成情况，与 [`Verdict`] **正交**：
/// 一个 `COMPLETED` 的执行完全可以判出 `RATE_FAIL`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionStatus {
    #[default]
    Completed,
    Partial,
    Error,
    TimedOut,
    Cancelled,
    Skipped,
}

impl ExecutionStatus {
    pub fn label(self) -> &'static str {
        match self {
            ExecutionStatus::Completed => "COMPLETED",
            ExecutionStatus::Partial => "PARTIAL",
            ExecutionStatus::Error => "ERROR",
            ExecutionStatus::TimedOut => "TIMEOUT",
            ExecutionStatus::Cancelled => "CANCELLED",
            ExecutionStatus::Skipped => "SKIPPED",
        }
    }

    pub fn from_label(label: &str) -> Option<ExecutionStatus> {
        Some(match label {
            "COMPLETED" => ExecutionStatus::Completed,
            "PARTIAL" => ExecutionStatus::Partial,
            "ERROR" => ExecutionStatus::Error,
            "TIMEOUT" => ExecutionStatus::TimedOut,
            "CANCELLED" => ExecutionStatus::Cancelled,
            "SKIPPED" => ExecutionStatus::Skipped,
            _ => return None,
        })
    }
}

/// 一次判定的完整结论：**判什么 + 为什么 + 说给人听的那句话**。
///
/// 真实项目的注释解释了它为什么不是一个 `(Verdict, ReasonCode, String)` 裸元组：
/// 三个字段里有两个是判定语义，位置写反了编译器不会拦，报告上却会变成
/// 「原因码和明细对不上」。
///
/// 它只描述**结论**，不含测量值和执行状态——所以产出它的函数可以是纯函数，
/// 拿一份「已经确定的事实」就能单测，不需要起进程、连对端。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VerdictResult {
    pub verdict: Verdict,
    pub code: ReasonCode,
    pub detail: String,
    /// **不参与判定**的排障线索。
    ///
    /// 真实项目的 ADR-17：吞吐验收只认接收端 RX 平均，UDP 丢包、TCP 重传、
    /// 负载下时延一律不许改写那个结论。但它们仍是排障必需的事实，所以单开
    /// 一条通道，报告里显示成「诊断」。
    pub diagnostics: Vec<String>,
}

impl VerdictResult {
    pub fn new(verdict: Verdict, code: ReasonCode, detail: impl Into<String>) -> Self {
        Self {
            verdict,
            code,
            detail: detail.into(),
            diagnostics: Vec::new(),
        }
    }

    /// 挂上诊断线索。判定本身一个字节都不变——这正是它单独存在的意义。
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: Vec<String>) -> Self {
        self.diagnostics
            .extend(diagnostics.into_iter().filter(|l| !l.trim().is_empty()));
        self
    }
}

/// 速率判定：这是整个工具的核心规则，只有三行。
///
/// `rx_avg` 是**接收端网卡 RX 平均**（`None` = 没形成可信平均）。
/// `no_avg_code` 是没有平均值时该记哪个原因码——由上游 [`crate::rate_window`] 给出。
pub fn rate_verdict(
    rx_avg: Option<f64>,
    target_mbps: f64,
    no_avg_code: ReasonCode,
) -> VerdictResult {
    let Some(avg) = rx_avg else {
        // 没有可信测量值 = 无法评价，**不是**失败。
        // 把这一类写成 RATE_FAIL 是真实项目出过的缺陷。
        return VerdictResult::new(
            Verdict::NotEvaluated,
            no_avg_code,
            no_avg_code
                .disposition_advice()
                .unwrap_or("没有可信的测量值"),
        );
    };

    if target_mbps <= 0.0 {
        // 没有门限就只记录数值，不做达标判断。
        return VerdictResult::new(
            Verdict::Measured,
            ReasonCode::None,
            format!("RX 平均 {avg:.0} Mbps（未设门限）"),
        );
    }

    if avg >= target_mbps {
        VerdictResult::new(
            Verdict::Pass,
            ReasonCode::RxTargetMet,
            format!("RX 平均 {avg:.0} Mbps >= 目标 {target_mbps:.0} Mbps"),
        )
    } else {
        VerdictResult::new(
            Verdict::RateFail,
            ReasonCode::RxBelowTarget,
            format!("RX 平均 {avg:.0} Mbps < 目标 {target_mbps:.0} Mbps"),
        )
    }
}

/// 把一个单元下多条腿的判定聚合成单元级判定。
///
/// 优先级顺序**照抄真实项目**的 `aggregate_verdict`：
///
/// ```text
/// 空          → SetupError
/// 任一 SetupError → SetupError
/// 按 [RateFail, NotEvaluated, Measured] 顺序取第一个命中
/// 任一 Skip   → Skip
/// 否则        → Pass
/// ```
///
/// 真实项目在第 2 步和第 3 步之间还有两条特例（`is_hard_single_udp_failure`
/// 和 `blocks_other_legs`），它们要读完整的 87 个原因码表才能实现，这里省掉。
/// 除此之外顺序一致。
///
/// 注意 `Pass` 是**最后**的兜底：只有在没有任何别的情况时才算通过。
/// 反过来写（先看有没有 Pass）会让一条腿通过就盖住另一条腿的失败。
pub fn aggregate_verdict<I>(items: I) -> Verdict
where
    I: IntoIterator<Item = (Verdict, ReasonCode)>,
{
    let items: Vec<(Verdict, ReasonCode)> = items.into_iter().collect();

    if items.is_empty() {
        return Verdict::SetupError;
    }
    if items.iter().any(|(v, _)| *v == Verdict::SetupError) {
        return Verdict::SetupError;
    }
    for candidate in [Verdict::RateFail, Verdict::NotEvaluated, Verdict::Measured] {
        if items.iter().any(|(v, _)| *v == candidate) {
            return candidate;
        }
    }
    if items.iter().any(|(v, _)| *v == Verdict::Skip) {
        return Verdict::Skip;
    }
    Verdict::Pass
}

mod verdict_serde {
    use super::{ExecutionStatus, Verdict};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for Verdict {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.label().serialize(s)
        }
    }

    impl<'de> Deserialize<'de> for Verdict {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let label = String::deserialize(d)?;
            Ok(Verdict::from_label(&label).unwrap_or_default())
        }
    }

    impl Serialize for ExecutionStatus {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            self.label().serialize(s)
        }
    }

    impl<'de> Deserialize<'de> for ExecutionStatus {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let label = String::deserialize(d)?;
            Ok(ExecutionStatus::from_label(&label).unwrap_or_default())
        }
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

    #[test]
    fn 每个判定label都能往返() {
        for v in [
            Verdict::Pass,
            Verdict::RateFail,
            Verdict::Measured,
            Verdict::NotEvaluated,
            Verdict::SetupError,
            Verdict::Skip,
        ] {
            assert_eq!(Verdict::from_label(v.label()), Some(v));
        }
    }

    #[test]
    fn 等于门限算通过() {
        // 边界：>= 而不是 >。这一条最容易写反。
        let r = rate_verdict(Some(900.0), 900.0, ReasonCode::None);
        assert_eq!(r.verdict, Verdict::Pass);
        assert_eq!(r.code, ReasonCode::RxTargetMet);
    }

    #[test]
    fn 低于门限是性能失败() {
        let r = rate_verdict(Some(899.9), 900.0, ReasonCode::None);
        assert_eq!(r.verdict, Verdict::RateFail);
        assert_eq!(r.code, ReasonCode::RxBelowTarget);
    }

    #[test]
    fn 没有测量值是无法评价而不是失败() {
        let r = rate_verdict(None, 900.0, ReasonCode::NoStreamStarted);
        assert_eq!(r.verdict, Verdict::NotEvaluated);
        assert_ne!(r.verdict, Verdict::RateFail, "环境问题不能算 CPE 的账");
    }

    #[test]
    fn 没有门限只记录不判定() {
        let r = rate_verdict(Some(500.0), 0.0, ReasonCode::None);
        assert_eq!(r.verdict, Verdict::Measured);
    }

    #[test]
    fn 聚合_空腿是准备错误() {
        assert_eq!(aggregate_verdict(vec![]), Verdict::SetupError);
    }

    #[test]
    fn 聚合_一条腿失败就盖住通过() {
        let v = aggregate_verdict(vec![
            (Verdict::Pass, ReasonCode::RxTargetMet),
            (Verdict::RateFail, ReasonCode::RxBelowTarget),
        ]);
        assert_eq!(v, Verdict::RateFail);
    }

    #[test]
    fn 聚合_失败优先于无法评价() {
        let v = aggregate_verdict(vec![
            (Verdict::NotEvaluated, ReasonCode::NoStreamStarted),
            (Verdict::RateFail, ReasonCode::RxBelowTarget),
        ]);
        assert_eq!(v, Verdict::RateFail);
    }

    #[test]
    fn 聚合_全通过才是通过() {
        let v = aggregate_verdict(vec![
            (Verdict::Pass, ReasonCode::RxTargetMet),
            (Verdict::Pass, ReasonCode::RxTargetMet),
        ]);
        assert_eq!(v, Verdict::Pass);
    }

    #[test]
    fn 判定和执行状态是正交的() {
        // 一个正常跑完的执行，完全可以判出不达标
        let r = rate_verdict(Some(800.0), 900.0, ReasonCode::None);
        assert_eq!(r.verdict, Verdict::RateFail);
        assert_eq!(ExecutionStatus::Completed.label(), "COMPLETED");
    }
}
