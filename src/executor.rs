//! 执行测试单元并产出判定。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/master/executor/`。真实实现要起 iperf3 / ctsTraffic
//! 进程、连辅测机、采网卡计数器、处理取消和超时；教学版**不起任何进程**，
//! 直接用计划里写死的样本，所以每次运行结果完全一样。
//!
//! 但结构是一样的：
//!
//! ```text
//! 拿到样本 → rate_window 算有效平均 → verdict 判定 → 聚合成单元结论
//! ```
//!
//! ## 这一层刻意不做的事
//!
//! 它**不自己判定**。判定规则全在 [`crate::verdict`]。真实项目吃过亏：
//! executor 和 report 各写过一份判定，两份优先级不一致，出过两个真实缺陷。

use crate::builder::Unit;
use crate::rate_window::{effective_rx_avg, RateWindow};
use crate::reason::ReasonCode;
use crate::verdict::{aggregate_verdict, rate_verdict, ExecutionStatus, Verdict, VerdictResult};

/// 一条腿跑完的结果。
#[derive(Debug, Clone)]
pub struct LegOutcome {
    pub tag: String,
    pub target_mbps: f64,
    pub window: RateWindow,
    pub verdict: VerdictResult,
    pub status: ExecutionStatus,
}

/// 一个单元跑完的结果。
#[derive(Debug, Clone)]
pub struct UnitOutcome {
    pub unit_id: String,
    pub title: String,
    pub transport: String,
    pub direction: String,
    pub round: u32,
    /// 单元级判定 = 各条腿聚合的结果，见 [`aggregate_verdict`]。
    pub verdict: Verdict,
    pub legs: Vec<LegOutcome>,
}

/// 跑一个单元。
pub fn execute_unit(unit: &Unit) -> UnitOutcome {
    let legs: Vec<LegOutcome> = unit
        .legs
        .iter()
        .map(|leg| execute_leg(leg, unit.warmup_secs))
        .collect();

    // 聚合只吃 (判定, 原因码)，不关心测量值——所以它是纯函数，好测。
    let verdict = aggregate_verdict(legs.iter().map(|l| (l.verdict.verdict, l.verdict.code)));

    UnitOutcome {
        unit_id: unit.id.clone(),
        title: unit.title.clone(),
        transport: unit.transport.clone(),
        direction: unit.direction.clone(),
        round: unit.round,
        verdict,
        legs,
    }
}

fn execute_leg(leg: &crate::builder::Leg, warmup_secs: usize) -> LegOutcome {
    // 1. 从样本算有效窗口平均
    let window = effective_rx_avg(&leg.samples, warmup_secs);

    // 2. 交给判定层。executor 自己不定规则。
    let mut verdict = rate_verdict(window.rx_avg, leg.target_mbps, window.code);

    // 3. 诊断线索：**不参与判定**，只是给排障的人看（ADR-17）。
    let mut diagnostics = Vec::new();
    if window.effective_samples > 0 {
        diagnostics.push(format!(
            "有效窗口 {} 秒，采样覆盖率 {:.0}%",
            window.effective_samples,
            window.coverage * 100.0
        ));
    }
    if window.coverage > 0.0 && window.coverage < 1.0 {
        diagnostics.push("窗口内存在采样空洞".to_string());
    }
    verdict = verdict.with_diagnostics(diagnostics);

    // 4. 执行状态和判定是正交的：一次正常跑完的执行也可以判出 RATE_FAIL。
    let status = match verdict.code {
        ReasonCode::NoStreamStarted => ExecutionStatus::Error,
        ReasonCode::EffectiveWindowShort => ExecutionStatus::Partial,
        _ => ExecutionStatus::Completed,
    };

    LegOutcome {
        tag: leg.tag.clone(),
        target_mbps: leg.target_mbps,
        window,
        verdict,
        status,
    }
}

/// 跑一整份单元清单。
pub fn execute_plan(units: &[Unit]) -> Vec<UnitOutcome> {
    units.iter().map(execute_unit).collect()
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
    use crate::plan::{Plan, Spec};

    fn 单条计划(direction: &str, target: f64, ab: Vec<f64>, ba: Vec<f64>) -> Plan {
        Plan {
            plan_id: "t".into(),
            rounds: 1,
            warmup_secs: 2,
            specs: vec![Spec {
                title: "测试".into(),
                transport: "tcp".into(),
                direction: direction.into(),
                target_mbps: target,
                samples_ab: ab,
                samples_ba: ba,
            }],
        }
    }

    fn 跑一个(plan: &Plan) -> UnitOutcome {
        let units = build_units(plan);
        execute_unit(&units[0])
    }

    #[test]
    fn 达标的单元判PASS() {
        let p = 单条计划(
            "ab",
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0],
            vec![],
        );
        let out = 跑一个(&p);
        assert_eq!(out.verdict, Verdict::Pass);
        assert_eq!(out.legs[0].verdict.code, ReasonCode::RxTargetMet);
    }

    #[test]
    fn 不达标的单元判RATE_FAIL() {
        let p = 单条计划(
            "ab",
            900.0,
            vec![100.0, 300.0, 850.0, 840.0, 860.0, 850.0],
            vec![],
        );
        let out = 跑一个(&p);
        assert_eq!(out.verdict, Verdict::RateFail);
        assert_eq!(out.legs[0].verdict.code, ReasonCode::RxBelowTarget);
    }

    #[test]
    fn 没有样本判NOT_EVALUATED而不是失败() {
        let p = 单条计划("ab", 900.0, vec![], vec![]);
        let out = 跑一个(&p);
        assert_eq!(out.verdict, Verdict::NotEvaluated);
        assert_eq!(out.legs[0].verdict.code, ReasonCode::NoStreamStarted);
        // 执行状态是 ERROR，但判定不是 RATE_FAIL——这两件事是分开的
        assert_eq!(out.legs[0].status, ExecutionStatus::Error);
    }

    #[test]
    fn 双向一条腿挂了整个单元不算通过() {
        let p = 单条计划(
            "bidir",
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0], // AB 达标
            vec![],                                         // BA 没样本
        );
        let out = 跑一个(&p);
        assert_eq!(out.legs.len(), 2);
        assert_eq!(out.legs[0].verdict.verdict, Verdict::Pass);
        assert_eq!(out.legs[1].verdict.verdict, Verdict::NotEvaluated);
        // 聚合之后不能是 Pass
        assert_eq!(out.verdict, Verdict::NotEvaluated);
    }

    #[test]
    fn 双向两条腿都达标才算通过() {
        let good = vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0];
        let p = 单条计划("bidir", 900.0, good.clone(), good);
        assert_eq!(跑一个(&p).verdict, Verdict::Pass);
    }

    #[test]
    fn 诊断不改写判定() {
        // 有空洞但覆盖率够，照样达标；空洞只出现在 diagnostics 里
        let p = 单条计划(
            "ab",
            900.0,
            vec![100.0, 500.0, 950.0, 0.0, 950.0, 1900.0],
            vec![],
        );
        let out = 跑一个(&p);
        assert_eq!(out.verdict, Verdict::Pass);
        assert!(!out.legs[0].verdict.diagnostics.is_empty());
    }

    #[test]
    fn 同一份计划跑两次结果完全一样() {
        let p = 单条计划(
            "ab",
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0],
            vec![],
        );
        let a = 跑一个(&p);
        let b = 跑一个(&p);
        assert_eq!(a.verdict, b.verdict);
        assert_eq!(a.legs[0].window.rx_avg, b.legs[0].window.rx_avg);
    }
}
