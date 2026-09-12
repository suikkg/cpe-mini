//! 测试计划：从 JSON 读进来，然后校验。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/master/plan.rs` 和 `src/master/webui/plan.rs`
//! （真实文件 2197 行，多出来的部分主要是 HTTP DTO 和前端来的字段校验）。
//!
//! ## 为什么校验要单独一步
//!
//! 非法配置必须在**开跑之前**就拦下来。一个负数门限如果混进执行阶段，
//! 结果就是跑完十分钟拿到一份没意义的报告。校验失败要说清楚是第几条、
//! 哪个字段、错在哪 —— 报错定位不到位，用户只能猜。

use serde::{Deserialize, Serialize};

/// 一份测试计划。
///
/// `#[serde(default)]` 让缺字段的旧计划也能读进来——真实项目里这是兼容面的
/// 基本要求：新版加了字段，不能让历史计划文件读不出来。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Plan {
    pub plan_id: String,
    /// 稳定性轮次：同一批 spec 重复跑几轮。不分轮就是 1。
    pub rounds: u32,
    /// 每条腿要切掉的爬坡秒数，见 [`crate::rate_window`]。
    pub warmup_secs: usize,
    pub specs: Vec<Spec>,
}

impl Default for Plan {
    fn default() -> Self {
        Plan {
            plan_id: String::new(),
            rounds: 1,
            warmup_secs: 2,
            specs: Vec::new(),
        }
    }
}

/// 一条测试规格。展开成测试单元之前的样子。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Spec {
    pub title: String,
    /// `tcp` / `udp`
    pub transport: String,
    /// `ab`（正向）/ `ba`（反向）/ `bidir`（双向）
    pub direction: String,
    /// 门限，Mbps。`0` = 不判定，只记录（对应 `Verdict::Measured`）。
    pub target_mbps: f64,
    /// A→B 方向接收端网卡逐秒 RX 速率（Mbps）。
    ///
    /// 真实项目这些数是执行时从网卡计数器采出来的；教学版直接写在计划里，
    /// 这样每次运行结果完全一样，改了代码能立刻看出是不是改对了。
    pub samples_ab: Vec<f64>,
    /// B→A 方向。单向计划留空。
    pub samples_ba: Vec<f64>,
    /// A→B 方向的 UDP 丢包率（百分比，0~100）。
    ///
    /// 字段名的根与真实项目的 `Row.udp_loss`（`src/report/model.rs:229`）一致。
    ///
    /// **`None` 表示这一轮没采到丢包数据，不是「丢包 0%」。** 两者在报告里
    /// 必须分得开：前者是「不知道」，后者是「知道，很好」。这就是它用
    /// `Option<f64>` 而不是 `f64` 的全部理由——用 `f64` 的话，缺字段的旧计划
    /// 会静默变成一份「全部零丢包」的完美报告。
    ///
    /// 它**不参与判定**，只进诊断，见 [`crate::executor`] 里的 ADR-17 注释。
    pub udp_loss_ab: Option<f64>,
    /// B→A 方向的丢包率。
    pub udp_loss_ba: Option<f64>,
}

/// 校验结果：要么放行，要么给出**全部**问题。
///
/// 刻意一次报全部而不是遇到第一个就返回：用户改一次配置就能改完，
/// 而不是改一条跑一次、再被下一条拦住。
pub fn validate(plan: &Plan) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if plan.plan_id.trim().is_empty() {
        errors.push("plan_id 不能为空".to_string());
    }
    if plan.rounds < 1 {
        errors.push(format!("rounds 至少是 1，当前是 {}", plan.rounds));
    }
    if plan.specs.is_empty() {
        errors.push("specs 至少要有一条测试规格".to_string());
    }

    for (i, spec) in plan.specs.iter().enumerate() {
        // 报错要带位置：第几条、哪个字段
        let at = format!("specs[{i}]");

        if spec.title.trim().is_empty() {
            errors.push(format!("{at}.title 不能为空"));
        }
        if !matches!(spec.transport.as_str(), "tcp" | "udp") {
            errors.push(format!(
                "{at}.transport 只能是 tcp 或 udp，当前是 {:?}",
                spec.transport
            ));
        }
        if !matches!(spec.direction.as_str(), "ab" | "ba" | "bidir") {
            errors.push(format!(
                "{at}.direction 只能是 ab / ba / bidir，当前是 {:?}",
                spec.direction
            ));
        }
        if spec.target_mbps < 0.0 {
            errors.push(format!(
                "{at}.target_mbps 不能是负数，当前是 {}",
                spec.target_mbps
            ));
        }
        if !spec.target_mbps.is_finite() {
            errors.push(format!("{at}.target_mbps 必须是有限数值"));
        }
        if spec.direction == "bidir" && spec.samples_ba.is_empty() && spec.samples_ab.is_empty() {
            errors.push(format!("{at} 是双向测试，但两个方向都没有样本"));
        }

        // 丢包率是百分比，超出 0~100 就是数据本身有问题。
        // 拦在这里而不是等报告渲染时才发现：跑完十分钟拿到一份
        // 「丢包 -3%」的报告，没人知道该信哪个数。
        for (field, value) in [
            ("udp_loss_ab", spec.udp_loss_ab),
            ("udp_loss_ba", spec.udp_loss_ba),
        ] {
            let Some(v) = value else { continue };
            if !v.is_finite() || !(0.0..=100.0).contains(&v) {
                errors.push(format!("{at}.{field} 要在 0~100 之间，当前是 {v}"));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// 从文件读一份计划并校验。
pub fn load(path: &std::path::Path) -> Result<Plan, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;

    // serde 的报错自带行列号，直接透出去比自己包装更有用
    let plan: Plan = serde_json::from_str(&text)
        .map_err(|e| format!("{} 不是合法的计划文件：{e}", path.display()))?;

    validate(&plan).map_err(|errs| {
        let mut msg = format!("{} 校验不通过（{} 处）：", path.display(), errs.len());
        for e in errs {
            msg.push_str("\n  - ");
            msg.push_str(&e);
        }
        msg
    })?;

    Ok(plan)
}

#[cfg(test)]
// 测试名用中文是为了让失败信息直接说清楚"哪条规则被破坏了"。
// 中文名里夹着 PASS / AB / JSON 这类大写 ASCII 会触发 non_snake_case，
// 这里按模块限定地关掉——allow 要贴在最小范围上并写明理由，
// 不要图省事加在 crate 根上。
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn 合法计划() -> Plan {
        Plan {
            plan_id: "t".into(),
            rounds: 1,
            warmup_secs: 2,
            specs: vec![Spec {
                title: "TCP 下行".into(),
                transport: "tcp".into(),
                direction: "ab".into(),
                target_mbps: 900.0,
                samples_ab: vec![900.0; 6],
                samples_ba: vec![],
                udp_loss_ab: None,
                udp_loss_ba: None,
            }],
        }
    }

    #[test]
    fn 合法计划能通过() {
        assert!(validate(&合法计划()).is_ok());
    }

    #[test]
    fn 负数门限被拦下() {
        let mut p = 合法计划();
        p.specs[0].target_mbps = -1.0;
        let errs = validate(&p).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("target_mbps")));
    }

    #[test]
    fn 门限为0是合法的_表示只记录不判定() {
        let mut p = 合法计划();
        p.specs[0].target_mbps = 0.0;
        assert!(validate(&p).is_ok());
    }

    #[test]
    fn 未知的transport被拦下且报错带位置() {
        let mut p = 合法计划();
        p.specs[0].transport = "sctp".into();
        let errs = validate(&p).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("specs[0].transport")));
    }

    #[test]
    fn 一次报出全部问题而不是只报第一个() {
        let mut p = 合法计划();
        p.plan_id = String::new();
        p.specs[0].transport = "sctp".into();
        p.specs[0].direction = "up".into();
        let errs = validate(&p).unwrap_err();
        assert!(errs.len() >= 3, "应该一次报全部，实际只报了 {}", errs.len());
    }

    #[test]
    fn 缺字段的计划用默认值补齐() {
        // 旧版计划文件没有 rounds，读进来应该是 1 而不是报错
        let json = r#"{"plan_id":"t","specs":[]}"#;
        let p: Plan = serde_json::from_str(json).unwrap();
        assert_eq!(p.rounds, 1);
        assert_eq!(p.warmup_secs, 2);
    }

    #[test]
    fn 丢包率超出范围会被拦下() {
        let mut plan = 合法计划();
        plan.specs[0].udp_loss_ab = Some(120.0);
        let errs = validate(&plan).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| e.contains("udp_loss_ab") && e.contains("0~100")),
            "报错要说清楚哪个字段、合法范围是什么。当前：{errs:?}"
        );

        plan.specs[0].udp_loss_ab = Some(-1.0);
        assert!(validate(&plan).is_err(), "负的丢包率也是坏数据");
    }

    #[test]
    fn 没给丢包率不算错() {
        let mut plan = 合法计划();
        plan.specs[0].udp_loss_ab = None;
        // None = 这一轮没采丢包，是完全正常的情况，不该拦
        assert!(validate(&plan).is_ok());
    }

    #[test]
    fn 旧计划没有丢包字段也读得出来() {
        // 兼容面：加字段之前写的计划文件，今天还要能读。
        let text = r#"{
            "plan_id": "old",
            "rounds": 1,
            "warmup_secs": 2,
            "specs": [{
                "title": "TCP",
                "transport": "tcp",
                "direction": "ab",
                "target_mbps": 900.0,
                "samples_ab": [900.0],
                "samples_ba": []
            }]
        }"#;
        let plan: Plan = serde_json::from_str(text).expect("缺字段的旧计划必须还能读");
        assert_eq!(plan.specs[0].udp_loss_ab, None, "缺字段读成 None，不是 0.0");
    }
}
