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
use crate::cancel::CancelFlag;
use crate::rate_window::{effective_rx_avg, RateWindow};
use crate::reason::ReasonCode;
use crate::verdict::{aggregate_verdict, rate_verdict, ExecutionStatus, Verdict, VerdictResult};

/// UDP 丢包门槛（百分比）。超过它才写进诊断。
///
/// 名字和真实项目一致：那边是配置项 `iperf.rate_check.max_udp_loss_pct`
/// （`src/config.rs:447`），可以按用例调；这里固定成一个常量。
///
/// 低于它的丢包在真实链路上是常态，每条都写一句「丢包 0.2%」等于把诊断栏
/// 变成噪声，真正要紧的那条反而看不见了。
///
/// **这不是一条判定门限。** 丢包再高也不会改写 [`Verdict`]，它只决定
/// 「这句话要不要说出来」。见下面 `execute_leg` 里的 ADR-17 注释。
pub const MAX_UDP_LOSS_PCT: f64 = 1.0;

/// 这个丢包率值不值得说出来。
///
/// 抽成一个函数是因为**有两个地方要问同一个问题**：这里（写进诊断）和
/// [`crate::report::render`]（显示在摘要里）。各写一遍 `loss > 1.0`，
/// 哪天阈值改成 2.0，两边就漂了——报告里显示了一条诊断，JSONL 里却没有。
///
/// 这和 `verdict.rs` 那条「判定只能有一份」是同一条规矩的小号版本。
pub fn loss_worth_mentioning(udp_loss: Option<f64>) -> bool {
    matches!(udp_loss, Some(v) if v > MAX_UDP_LOSS_PCT)
}

/// 一条腿跑完的结果。
#[derive(Debug, Clone)]
pub struct LegOutcome {
    pub tag: String,
    pub target_mbps: f64,
    pub window: RateWindow,
    pub verdict: VerdictResult,
    pub status: ExecutionStatus,
    /// 丢包率（百分比）。`None` = 没采到。**不参与判定。**
    pub udp_loss: Option<f64>,
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

    // 丢包率也走这条通道。
    //
    // 这里是全项目最容易写错的一处：看到「丢包 30%」，第一反应是
    // 「这肯定该判失败」。不行。吞吐验收的唯一判定输入是**接收端 RX 平均对门限**
    // （ADR-17）。丢包 30% 但 RX 仍然跑到了门限，结论就是 PASS——
    // 因为用户要的是「这条链路能不能跑到这个速度」，它能。
    //
    // 丢包高是**排障线索**：它解释了为什么重传多、为什么时延抖，
    // 但它不是这个用例要回答的问题。给它开第二条判定路径，
    // 就等于让同一份报告里有两套「算不算失败」的口径。
    //
    // （真实项目里确实有 `ReasonCode::UdpLossHigh` 这个码，但它属于
    // **另一类用例**：ctsTraffic 的 UDP 专项测试，那里丢包本身就是被测指标。
    // 同一个数字在不同用例里是不是判定输入，取决于用例在问什么。）
    if loss_worth_mentioning(leg.udp_loss) {
        // 这句话的措辞照抄真实项目（`src/inner/mod.rs:1586`）：
        // 「丢包只作诊断，达标与否只看接收端速率」——把规则写进用户看得到的
        // 那句话里，比写在注释里管用得多。
        //
        // unwrap 安全：loss_worth_mentioning 为真意味着它是 Some
        diagnostics.push(format!(
            "UDP_LOSS_HIGH: 丢包率 {:.1}% 超过门槛 {MAX_UDP_LOSS_PCT:.1}%；丢包只作诊断，达标与否只看接收端速率",
            leg.udp_loss.unwrap_or_default()
        ));
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
        udp_loss: leg.udp_loss,
    }
}

/// 跑一整份单元清单。
///
/// 这是 [`execute_plan_cancellable`] 的便捷版：给一个永远不会置位的标志。
pub fn execute_plan(units: &[Unit]) -> Vec<UnitOutcome> {
    execute_plan_cancellable(units, &CancelFlag::new())
}

/// 跑一整份单元清单，中途可以被叫停。
///
/// ## 检查时机：只在单元边界
///
/// 每跑完一个单元问一次「要不要停」，不在单元中途问。理由写在
/// [`crate::cancel`] 的模块注释里：收尾路径要跑完。
///
/// ## 被取消的单元不会从报告里消失
///
/// 这是这个函数最要紧的一件事，也是最容易写错的一件事。
///
/// 「停下来」的直觉实现是 `break`——剩下的单元干脆不产出结果。但
/// [`crate::report::rows_from`] 是按**腿**摊平成行的：没有结果的单元
/// 一行都不会有，报告里于是安安静静地少了十个测试项。
///
/// 少的那十个和「跑了但跳过」长得一模一样，跟上一轮做 `compare` 时
/// 它们会被判成 `Disappeared`——而真相是「机器被人按停了」。
///
/// 所以取消之后的每个单元照样产出完整的腿，只是判定是 `SKIP`、
/// 执行状态是 `CANCELLED`。**报告的行数只由计划决定，跟跑没跑完无关。**
pub fn execute_plan_cancellable(units: &[Unit], cancel: &CancelFlag) -> Vec<UnitOutcome> {
    let mut outcomes = Vec::with_capacity(units.len());
    let mut stopped = false;

    for unit in units {
        // 一个单元只问一次：is_cancelled 会推进教学扳机的计数
        if !stopped && cancel.is_cancelled() {
            stopped = true;
        }

        outcomes.push(if stopped {
            cancelled_unit(unit)
        } else {
            execute_unit(unit)
        });
    }

    outcomes
}

/// 一个因为整轮被取消而没跑的单元长什么样。
///
/// 注意它**照样有腿**，只是每条腿都是 `SKIP` / `CANCELLED`。
fn cancelled_unit(unit: &Unit) -> UnitOutcome {
    let legs: Vec<LegOutcome> = unit
        .legs
        .iter()
        .map(|leg| LegOutcome {
            tag: leg.tag.clone(),
            target_mbps: leg.target_mbps,
            window: RateWindow {
                rx_avg: None,
                code: ReasonCode::None,
                effective_samples: 0,
                coverage: 0.0,
            },
            // 判定是 SKIP 不是 SETUP_ERROR：环境没出问题，是人喊停的。
            // 写成 SETUP_ERROR 会让退出码变成 1，脚本就以为测试失败了。
            verdict: VerdictResult::new(
                Verdict::Skip,
                ReasonCode::None,
                "整轮测试被取消，这个单元没有执行",
            ),
            // ExecutionStatus 和 Verdict 是两件事：
            // 「结论是跳过」和「过程是被取消的」分开记，排障时才知道
            // 这个 SKIP 是计划里本来就要跳过的，还是被人按停的。
            status: ExecutionStatus::Cancelled,
            udp_loss: None,
        })
        .collect();

    // 单元级判定照样走聚合，不在这里硬写 Skip——判定只有一份。
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

    /// 带丢包率的单向计划。丢包只进诊断，不该动判定。
    fn 带丢包的计划(target: f64, ab: Vec<f64>, loss: Option<f64>) -> Plan {
        let mut plan = 单条计划("ab", target, ab, vec![]);
        plan.specs[0].udp_loss_ab = loss;
        plan
    }

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
                udp_loss_ab: None,
                udp_loss_ba: None,
            }],
        }
    }

    fn 跑一个(plan: &Plan) -> UnitOutcome {
        let units = build_units(plan);
        execute_unit(&units[0])
    }

    /// 三轮 × 一条规格 = 三个单元，用来看取消停在哪里。
    fn 三轮计划() -> Plan {
        let mut plan = 单条计划("ab", 100.0, vec![200.0; 6], vec![]);
        plan.rounds = 3;
        plan
    }

    #[test]
    fn 没取消时跟原来完全一样() {
        let units = build_units(&三轮计划());
        let a = execute_plan(&units);
        let b = execute_plan_cancellable(&units, &CancelFlag::new());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.unit_id, y.unit_id);
            assert_eq!(x.verdict, y.verdict);
        }
    }

    #[test]
    fn 取消之后的单元判SKIP() {
        let units = build_units(&三轮计划());
        // 第 1 次检查（也就是第 2 个单元开跑前）扳机生效
        let outs = execute_plan_cancellable(&units, &CancelFlag::trip_after_polls(1));

        assert_eq!(outs[0].verdict, Verdict::Pass, "第一个单元在取消前跑完了");
        assert_eq!(outs[1].verdict, Verdict::Skip);
        assert_eq!(outs[2].verdict, Verdict::Skip);
    }

    #[test]
    fn 被取消的单元一个都不许少() {
        let units = build_units(&三轮计划());
        let outs = execute_plan_cancellable(&units, &CancelFlag::trip_after_polls(0));

        // 这是最要紧的一条：取消不等于「从报告里消失」。
        // 写成 break 的话这里是 0，跟上一轮 compare 时全变成 Disappeared。
        assert_eq!(outs.len(), units.len());
        for (out, unit) in outs.iter().zip(units.iter()) {
            assert_eq!(out.unit_id, unit.id);
            assert_eq!(out.legs.len(), unit.legs.len(), "腿也不许少");
        }
    }

    #[test]
    fn 取消记在执行状态上而不是判定上() {
        let units = build_units(&三轮计划());
        let outs = execute_plan_cancellable(&units, &CancelFlag::trip_after_polls(0));

        // 判定说「这个单元没有结论」，执行状态说「因为被人按停了」。
        // 两者分开记，排障时才分得出「计划里本来就跳过」和「被取消」。
        assert_eq!(outs[0].verdict, Verdict::Skip);
        assert_eq!(outs[0].legs[0].status, ExecutionStatus::Cancelled);
    }

    #[test]
    fn 取消不算失败() {
        // SETUP_ERROR 会让退出码变成 1，脚本就以为测试挂了。
        // 人按了停止不是测试失败。
        let units = build_units(&三轮计划());
        let outs = execute_plan_cancellable(&units, &CancelFlag::trip_after_polls(0));
        assert!(outs.iter().all(|o| o.verdict != Verdict::SetupError));
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

    // ---- 丢包只进诊断，不进判定（ADR-17）----------------------------------
    //
    // 这四个测试是一条边界的四个方向。有人哪天觉得「丢包 30% 判 PASS 不合理」
    // 而在 execute_leg 里加一句 if loss > x { verdict = RateFail }，
    // 下面第一个测试立刻会红。它红的时候请先读它的名字，再决定要不要改规则。

    #[test]
    fn 丢包再高也不改判定() {
        let p = 带丢包的计划(
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0],
            Some(30.0),
        );
        let out = 跑一个(&p);

        // 用户问的是「这条链路能不能跑到 900」。它跑到了 950，所以 PASS。
        assert_eq!(
            out.verdict,
            Verdict::Pass,
            "丢包是排障线索，不是判定输入（ADR-17）"
        );
        assert_eq!(out.legs[0].verdict.code, ReasonCode::RxTargetMet);
    }

    #[test]
    fn 高丢包会出现在诊断里() {
        let p = 带丢包的计划(
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0],
            Some(30.0),
        );
        let out = 跑一个(&p);
        let diags = &out.legs[0].verdict.diagnostics;
        assert!(
            diags.iter().any(|d| d.contains("丢包率 30.0%")),
            "判定不受影响，但这条线索必须说出来，否则排障的人看不到。当前诊断：{diags:?}"
        );
    }

    #[test]
    fn 低丢包不写进诊断免得刷屏() {
        let p = 带丢包的计划(
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0],
            Some(0.2),
        );
        let out = 跑一个(&p);
        assert!(
            !out.legs[0]
                .verdict
                .diagnostics
                .iter()
                .any(|d| d.contains("丢包")),
            "0.2% 是常态，每条都写一句会把真正要紧的那条淹掉"
        );
    }

    #[test]
    fn 没采到丢包和丢包为零是两回事() {
        let 没采到 = 跑一个(&带丢包的计划(
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0],
            None,
        ));
        let 零丢包 = 跑一个(&带丢包的计划(
            900.0,
            vec![100.0, 500.0, 950.0, 960.0, 950.0, 940.0],
            Some(0.0),
        ));

        assert_eq!(没采到.legs[0].udp_loss, None);
        assert_eq!(零丢包.legs[0].udp_loss, Some(0.0));
        // 两者判定都不受影响，但报告里读得出区别
        assert_eq!(没采到.verdict, Verdict::Pass);
        assert_eq!(零丢包.verdict, Verdict::Pass);
    }
}
