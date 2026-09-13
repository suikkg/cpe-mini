//! 进阶课（第 19 课）的骨架：聚合的两条特例。
//!
//! ## 怎么用
//!
//! ```bash
//! cp scaffold/aggregate_special.rs src/aggregate_special.rs
//! # 在 src/lib.rs 的模块列表里加一行（按字母序，放在 builder 前面）：
//! #     pub mod aggregate_special;
//! cargo test --lib aggregate_special     # 现在全红
//! ```
//!
//! 答案：`solutions/aggregate_special_answer.rs`
//!
//! ## 这一课在补什么
//!
//! `src/verdict.rs` 的 `aggregate_verdict` 上面有一句：
//!
//! > 真实项目在第 2 步和第 3 步之间还有两条特例
//! > （`is_hard_single_udp_failure` 和 `blocks_other_legs`），
//! > 它们要读完整的 87 个原因码表才能实现，这里省掉。
//!
//! 现在 `src/reason.rs` 里有那三个码了，可以把这两条补上。
//!
//! ## 为什么这两条特例存在
//!
//! 都是**真实缺陷**逼出来的，两个方向刚好相反：
//!
//! | 特例 | 防的是 | 不加会怎样 |
//! |---|---|---|
//! | `is_hard_single_udp_failure` | 真失败被藏起来 | 用户点名「必须灌通」的方向没灌通，概览却显示「无法评价」 |
//! | `blocks_other_legs` | 环境异常被写成性能失败 | 采样塌了那段时间的数不可信，却拿它判 CPE 不达标 |
//!
//! 第二条还有一层：**不是所有「无法评价」都该盖住别人。** 双向单元里
//! A→B 的门限没配（这条腿自己的配置问题），不该让 B→A 那个确凿的
//! `RATE_FAIL` 从概览里消失。
//!
//! 真实项目的原话：「名单只放『确定安全』的三个，其余一律按老行为盖住：
//! 这里放宽一个码，对应的就是一批历史上判『无法评价』的单元变成 `RATE_FAIL`。」
//!
//! **放宽一条规则的代价要能说清楚，说不清楚就别放宽。**

#![allow(dead_code, unused_variables)]

use crate::reason::ReasonCode;
use crate::verdict::Verdict;

/// 用户点名「必须灌通」的方向没灌通。
///
/// 真实项目这个数组有 2 个元素（另一个是 `CtsSingleUdpStreamFailed`，
/// ctsTraffic 工具那条路径的同一件事）；教学版只有一个。
pub const HARD_SINGLE_UDP_FAILURE_CODES: [ReasonCode; 1] = [ReasonCode::SingleUdpStreamFailed];

/// 只说明**这条腿自己**判定前提不成立的「无法评价」。
///
/// 名单**只放确定安全的三个**，名字和内容都照抄真实项目。
/// 剩下的（采样覆盖率低、有效窗口太短、工具没起来）一律按老行为盖住别人。
pub const LEG_LOCAL_NOT_EVALUATED_CODES: [ReasonCode; 3] = [
    ReasonCode::ConfiguredLoadTooLow,
    ReasonCode::OfferedLoadLow,
    ReasonCode::TargetMissing,
];

// ===========================================================================
// 第 1 题
// ===========================================================================

/// 这一条结果是不是「用户点名必须灌通的方向没灌通」。
///
/// 两个条件都要满足：判定是 `RATE_FAIL`，**并且**原因码在
/// [`HARD_SINGLE_UDP_FAILURE_CODES`] 里。
///
/// 想一想：为什么要同时看判定？只看原因码不行吗？
pub fn is_hard_single_udp_failure(verdict: Verdict, reason_code: ReasonCode) -> bool {
    todo!("第 1 题")
}

// ===========================================================================
// 第 2 题
// ===========================================================================

/// 这一条「无法评价」会不会盖住同一个单元里别的腿的结论。
///
/// 条件：判定是 `NOT_EVALUATED`，**并且**原因码**不在**
/// [`LEG_LOCAL_NOT_EVALUATED_CODES`] 里。
///
/// 注意那个「不在」——名单是**白名单**（这几个安全，不盖），
/// 不是黑名单。写反了的后果，在模块注释的表里。
pub fn blocks_other_legs(verdict: Verdict, reason_code: ReasonCode) -> bool {
    todo!("第 2 题")
}

// ===========================================================================
// 第 3 题
// ===========================================================================

/// 带上两条特例的完整聚合，照抄真实项目的九步优先级：
///
/// ```text
/// 1. 空集合                      → SetupError
/// 2. 任一 SetupError             → SetupError
/// 3. 任一 硬失败                  → RateFail        ← 新增
/// 4. 任一 会盖住别人的 NotEvaluated → NotEvaluated    ← 新增
/// 5. 任一 RateFail               → RateFail
/// 6. 任一 NotEvaluated           → NotEvaluated
/// 7. 任一 Measured               → Measured
/// 8. 任一 Skip                   → Skip
/// 9. 否则                        → Pass
/// ```
///
/// 第 3 步**必须**排在第 4 步前面。反过来的话，双向单元里
/// 「A→B 硬失败 + B→A 采样塌了」会判成 `NOT_EVALUATED`，
/// 那个硬失败就从概览里消失了——这正是特例要防的事。
///
/// `src/verdict.rs` 里现有的 `aggregate_verdict` 是这个的第 3、4 步删掉版，
/// 可以直接拿来改。
pub fn aggregate_verdict_full<I>(items: I) -> Verdict
where
    I: IntoIterator<Item = (Verdict, ReasonCode)>,
{
    let items: Vec<(Verdict, ReasonCode)> = items.into_iter().collect();
    todo!("第 3 题")
}

#[cfg(test)]
// 测试名用中文；夹着大写 ASCII 会触发 non_snake_case。
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::verdict::aggregate_verdict;

    // ---- 第 1 题 ----

    #[test]
    fn 硬失败要判定和原因码都对上() {
        assert!(is_hard_single_udp_failure(
            Verdict::RateFail,
            ReasonCode::SingleUdpStreamFailed
        ));
        // 原因码对了但判定不是 RATE_FAIL：不算。
        // 只看原因码的话，一条 NOT_EVALUATED 会被当成硬失败，
        // 把整个单元拉成 RATE_FAIL——把没测成写成了性能不达标。
        assert!(!is_hard_single_udp_failure(
            Verdict::NotEvaluated,
            ReasonCode::SingleUdpStreamFailed
        ));
        // 判定对了但原因码是普通的低于门限：那是普通 RATE_FAIL，不是硬失败
        assert!(!is_hard_single_udp_failure(
            Verdict::RateFail,
            ReasonCode::RxBelowTarget
        ));
    }

    // ---- 第 2 题 ----

    #[test]
    fn 名单里的三个不盖住别人() {
        for code in LEG_LOCAL_NOT_EVALUATED_CODES {
            assert!(
                !blocks_other_legs(Verdict::NotEvaluated, code),
                "{code} 是这条腿自己的配置问题，不该盖住别的腿"
            );
        }
    }

    #[test]
    fn 采样类的判不了必须盖住别人() {
        // 双向的两条腿跑在同一段时间窗里。那段时间的采样塌了，
        // 另一条腿的数同样不可信，拿它判 FAIL 就是把环境异常写成 CPE 性能失败。
        for code in [
            ReasonCode::SampleCoverageLow,
            ReasonCode::EffectiveWindowShort,
            ReasonCode::NoStreamStarted,
        ] {
            assert!(blocks_other_legs(Verdict::NotEvaluated, code), "{code}");
        }
    }

    #[test]
    fn 不是无法评价就谈不上盖住谁() {
        assert!(!blocks_other_legs(
            Verdict::Pass,
            ReasonCode::SampleCoverageLow
        ));
        assert!(!blocks_other_legs(
            Verdict::RateFail,
            ReasonCode::SampleCoverageLow
        ));
    }

    // ---- 第 3 题 ----

    #[test]
    fn 硬失败不被另一条腿的无法评价盖住() {
        // 加了第 4 步之后，普通的不达标会被「采样塌了」盖住：
        assert_eq!(
            aggregate_verdict_full([
                (Verdict::RateFail, ReasonCode::RxBelowTarget),
                (Verdict::NotEvaluated, ReasonCode::SampleCoverageLow),
            ]),
            Verdict::NotEvaluated
        );

        // 但用户点名「必须灌通」的那条不会——第 3 步排在第 4 步前面
        // 就是为了这个。两条腿的形状一模一样，只有原因码不同。
        assert_eq!(
            aggregate_verdict_full([
                (Verdict::RateFail, ReasonCode::SingleUdpStreamFailed),
                (Verdict::NotEvaluated, ReasonCode::SampleCoverageLow),
            ]),
            Verdict::RateFail
        );
    }

    #[test]
    fn 这条腿自己缺门限不盖住另一条腿的不达标() {
        // A→B 没配门限（这条腿自己的事），B→A 确凿地没跑到门限。
        // 单元结论应该是 RATE_FAIL，不是「无法评价」。
        //
        // 白名单要是写成了「所有 NOT_EVALUATED 都盖住别人」，这里就变成
        // NOT_EVALUATED——一个确凿的不达标从概览里安静地消失。
        let legs = [
            (Verdict::NotEvaluated, ReasonCode::TargetMissing),
            (Verdict::RateFail, ReasonCode::RxBelowTarget),
        ];
        assert_eq!(aggregate_verdict_full(legs), Verdict::RateFail);
    }

    #[test]
    fn 采样塌了就不许下不达标的结论() {
        let legs = [
            (Verdict::NotEvaluated, ReasonCode::SampleCoverageLow),
            (Verdict::RateFail, ReasonCode::RxBelowTarget),
        ];
        assert_eq!(aggregate_verdict_full(legs), Verdict::NotEvaluated);

        // 这一条就是两条特例带来的**唯一一处**行为改变。
        // 现有实现（没有第 4 步）判 RATE_FAIL——那段时间的采样都塌了，
        // 另一条腿的数同样不可信，拿它写成「CPE 性能不达标」是误判。
        assert_eq!(aggregate_verdict(legs), Verdict::RateFail);
    }

    #[test]
    fn SETUP_ERROR排在硬失败前面() {
        // 环境都没搭起来，性能结论一律没意义
        let legs = [
            (Verdict::SetupError, ReasonCode::None),
            (Verdict::RateFail, ReasonCode::SingleUdpStreamFailed),
        ];
        assert_eq!(aggregate_verdict_full(legs), Verdict::SetupError);
    }

    #[test]
    fn 普通情形跟现有实现结果一样() {
        // 特例只在特定原因码上生效。别的情况必须原样，
        // 否则这就不是「补两条特例」，是「换了一套判定」。
        let cases: [&[(Verdict, ReasonCode)]; 5] = [
            &[],
            &[(Verdict::Pass, ReasonCode::RxTargetMet)],
            &[
                (Verdict::Pass, ReasonCode::RxTargetMet),
                (Verdict::Skip, ReasonCode::None),
            ],
            &[(Verdict::Measured, ReasonCode::None)],
            &[
                (Verdict::RateFail, ReasonCode::RxBelowTarget),
                (Verdict::Pass, ReasonCode::RxTargetMet),
            ],
        ];
        for case in cases {
            assert_eq!(
                aggregate_verdict_full(case.iter().copied()),
                aggregate_verdict(case.iter().copied()),
                "{case:?} 上两个实现不一致"
            );
        }
    }

    #[test]
    fn 全PASS才是PASS() {
        assert_eq!(
            aggregate_verdict_full([
                (Verdict::Pass, ReasonCode::RxTargetMet),
                (Verdict::Pass, ReasonCode::RxTargetMet),
            ]),
            Verdict::Pass
        );
    }
}
