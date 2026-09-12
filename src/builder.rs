//! 把测试规格展开成一条条**测试单元**。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/master/builder.rs`（真实文件 5205 行）。
//! 真实实现要处理网卡组合、IP 对、套件参数笛卡尔积、NIC 漂移检测；
//! 这里只保留最核心的两条展开：**轮次**和**方向**。
//!
//! ## 计划 vs 单元
//!
//! 计划是用户填的东西（「TCP 双向，门限 900，跑 2 轮」），
//! 单元是真正要执行的一条条任务（「第 1 轮 TCP，AB 腿 + BA 腿」）。
//! 一条规格能展开成好几个单元，这一层就是干这个的。

use crate::plan::{Plan, Spec};

/// 一条测试腿：单元里实际要跑的一个方向。
#[derive(Debug, Clone)]
pub struct Leg {
    /// 方向标签。
    ///
    /// **单向单元这里是空串**——这是真实项目的语义，不是漏填：
    /// 空 tag 在执行侧表示「这个单元只有一个方向，不用区分」。
    /// 双向单元才是 `AB` / `BA`。
    pub tag: String,
    /// 这条腿的门限。
    pub target_mbps: f64,
    /// 接收端网卡逐秒 RX 速率。
    pub samples: Vec<f64>,
    /// 这条腿的丢包率（百分比）。`None` = 没采到，**不是** 0%。
    ///
    /// 只用于诊断，判定一个字节都不看它（ADR-17）。
    pub udp_loss: Option<f64>,
}

/// 一个测试单元：执行和判定的基本粒度。
#[derive(Debug, Clone)]
pub struct Unit {
    /// 单元 id。**轮次拌进 id 里**——对应真实项目的 `round_scoped_id`。
    ///
    /// 不这么做的话，20 轮的计划在 HashMap 里会互相覆盖，只剩最后一轮，
    /// 而且不会有任何异常提示。真实项目为这件事专门留过注释。
    pub id: String,
    pub title: String,
    pub transport: String,
    /// `ab` / `ba` / `bidir`。**只用于展示**，判定不读它。
    pub direction: String,
    /// 稳定性轮次（1-based）。不分轮的计划恒为 1。
    ///
    /// 是类型化字段而不是从 title 的「· 第 N 轮」后缀里搜出来的：
    /// 那个后缀是展示串，改一次文案就全体失效，而失效的表现是
    /// 「对比报告少了 19 轮」这种没人会去核对的安静错误。
    pub round: u32,
    pub warmup_secs: usize,
    pub legs: Vec<Leg>,
}

/// 把计划展开成单元清单。
///
/// 展开顺序是 **轮次 → 规格**：第 1 轮的所有规格跑完，再跑第 2 轮。
/// 反过来（规格 → 轮次）会让同一条测试连着跑 N 遍，测不出跨时间的稳定性。
pub fn build_units(plan: &Plan) -> Vec<Unit> {
    let mut units = Vec::new();

    for round in 1..=plan.rounds {
        for (i, spec) in plan.specs.iter().enumerate() {
            units.push(build_unit(plan, spec, i, round));
        }
    }

    units
}

fn build_unit(plan: &Plan, spec: &Spec, index: usize, round: u32) -> Unit {
    let legs = build_legs(spec);

    // 轮次拌进 id
    let id = format!("{}-{}-r{}", plan.plan_id, index, round);

    // 标题上带轮次是给人看的；判定和对齐都不读这个后缀
    let title = if plan.rounds > 1 {
        format!("{} · 第 {} 轮", spec.title, round)
    } else {
        spec.title.clone()
    };

    Unit {
        id,
        title,
        transport: spec.transport.clone(),
        direction: spec.direction.clone(),
        round,
        warmup_secs: plan.warmup_secs,
        legs,
    }
}

fn build_legs(spec: &Spec) -> Vec<Leg> {
    match spec.direction.as_str() {
        // 单向：一条腿，tag 是空串
        "ab" => vec![Leg {
            tag: String::new(),
            target_mbps: spec.target_mbps,
            samples: spec.samples_ab.clone(),
            udp_loss: spec.udp_loss_ab,
        }],
        "ba" => vec![Leg {
            tag: String::new(),
            target_mbps: spec.target_mbps,
            samples: spec.samples_ba.clone(),
            udp_loss: spec.udp_loss_ba,
        }],
        // 双向：两条腿，各自按同一个门限判
        "bidir" => vec![
            Leg {
                tag: "AB".to_string(),
                target_mbps: spec.target_mbps,
                samples: spec.samples_ab.clone(),
                udp_loss: spec.udp_loss_ab,
            },
            Leg {
                tag: "BA".to_string(),
                target_mbps: spec.target_mbps,
                samples: spec.samples_ba.clone(),
                udp_loss: spec.udp_loss_ba,
            },
        ],
        // 校验已经拦过非法方向，走到这里说明校验漏了——给空腿，
        // 聚合规则会把它判成 SETUP_ERROR，不会静默通过。
        _ => Vec::new(),
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
    use crate::plan::Spec;

    fn 计划(rounds: u32, direction: &str) -> Plan {
        Plan {
            plan_id: "p".into(),
            rounds,
            warmup_secs: 2,
            specs: vec![
                Spec {
                    title: "TCP".into(),
                    transport: "tcp".into(),
                    direction: direction.into(),
                    target_mbps: 900.0,
                    samples_ab: vec![900.0; 6],
                    samples_ba: vec![800.0; 6],
                    udp_loss_ab: None,
                    udp_loss_ba: None,
                },
                Spec {
                    title: "UDP".into(),
                    transport: "udp".into(),
                    direction: direction.into(),
                    target_mbps: 500.0,
                    samples_ab: vec![500.0; 6],
                    samples_ba: vec![500.0; 6],
                    udp_loss_ab: None,
                    udp_loss_ba: None,
                },
            ],
        }
    }

    #[test]
    fn 两条规格两轮展开成四个单元() {
        let units = build_units(&计划(2, "ab"));
        assert_eq!(units.len(), 4);
    }

    #[test]
    fn 展开顺序是先轮次后规格() {
        let units = build_units(&计划(2, "ab"));
        // 第 1 轮的两条先跑完，才轮到第 2 轮
        assert_eq!(units[0].round, 1);
        assert_eq!(units[1].round, 1);
        assert_eq!(units[2].round, 2);
        assert_eq!(units[3].round, 2);
    }

    #[test]
    fn 轮次拌进id所以不会互相覆盖() {
        let units = build_units(&计划(2, "ab"));
        let ids: std::collections::HashSet<&str> = units.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(ids.len(), 4, "4 个单元必须有 4 个不同的 id");
    }

    #[test]
    fn 单向单元一条腿且tag是空串() {
        let units = build_units(&计划(1, "ab"));
        assert_eq!(units[0].legs.len(), 1);
        assert_eq!(units[0].legs[0].tag, "", "单向腿的 tag 就该是空串");
    }

    #[test]
    fn 双向单元两条腿分别是AB和BA() {
        let units = build_units(&计划(1, "bidir"));
        assert_eq!(units[0].legs.len(), 2);
        assert_eq!(units[0].legs[0].tag, "AB");
        assert_eq!(units[0].legs[1].tag, "BA");
        // 两条腿拿的是不同方向的样本
        assert_eq!(units[0].legs[0].samples[0], 900.0);
        assert_eq!(units[0].legs[1].samples[0], 800.0);
    }

    #[test]
    fn 只有一轮时标题不带轮次后缀() {
        let units = build_units(&计划(1, "ab"));
        assert_eq!(units[0].title, "TCP");
        let units = build_units(&计划(2, "ab"));
        assert!(units[2].title.contains("第 2 轮"));
    }
}
