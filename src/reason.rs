//! 判定原因码。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/reason.rs`。真实项目里有 87 个变体（其中 37 个是
//! `CTSTRAFFIC_*`），这里只取跟「速率达标判定」直接相关的 6 个，**名字和字符串
//! 形式与真实项目完全一致**，将来读真实代码不用二次翻译。
//!
//! ## 为什么是 enum 而不是 String
//!
//! 真实项目的注释写得很清楚：同一个码此前要在 5 个地方各写一遍字面量，任何
//! 一处拼错都不会有编译期信号，只会在报告里静默变成「无建议」。换成 enum
//! 之后，漏登记是编译错误，不是运行时的一片空白。

use std::fmt;
use std::str::FromStr;

/// `None` 表示「这一行没有原因码」，序列化成空字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReasonCode {
    #[default]
    None,
    /// RX 平均达到了门限。
    RxTargetMet,
    /// RX 平均低于门限——这是唯一一个表示「CPE 性能不达标」的码。
    RxBelowTarget,
    /// 一个样本都没有：工具没起来、对端没连上。属于环境问题，不是性能问题。
    NoStreamStarted,
    /// 去掉爬坡段之后，有效窗口太短，不足以形成可信平均。
    EffectiveWindowShort,
    /// 有效窗口里的空洞太多（采样覆盖率低），平均值不可信。
    SampleCoverageLow,
    /// 计划里没给门限，或者门限非法。
    TargetMissing,
    /// 单流 UDP 耗尽全部重试仍然一个样本都没有。
    ///
    /// 这是**用户指定的硬失败**：那个方向「必须灌通」，所以哪怕它本质上是
    /// 「工具没测出来」，判定也算 `RATE_FAIL`，而且不许被另一条腿普通的
    /// 「无法评价」盖住。见第 19 课。
    SingleUdpStreamFailed,
    /// 计划里配的负载就没到门限——拿它判不达标没有意义。
    ///
    /// 这是**这条腿自己**的配置问题，不说明另一条腿的数不可信。第 19 课。
    ConfiguredLoadTooLow,
    /// 实际打出去的负载低于门限，同上，是这条腿自己的问题。第 19 课。
    OfferedLoadLow,
}

impl ReasonCode {
    /// 字符串形式是**对外兼容面**：报告、CSV、JSON 里都是这个大写下划线串。
    pub const fn as_str(self) -> &'static str {
        match self {
            ReasonCode::None => "",
            ReasonCode::RxTargetMet => "RX_TARGET_MET",
            ReasonCode::RxBelowTarget => "RX_BELOW_TARGET",
            ReasonCode::NoStreamStarted => "NO_STREAM_STARTED",
            ReasonCode::EffectiveWindowShort => "EFFECTIVE_WINDOW_SHORT",
            ReasonCode::SampleCoverageLow => "SAMPLE_COVERAGE_LOW",
            ReasonCode::TargetMissing => "TARGET_MISSING",
            ReasonCode::SingleUdpStreamFailed => "SINGLE_UDP_STREAM_FAILED",
            ReasonCode::ConfiguredLoadTooLow => "CONFIGURED_LOAD_TOO_LOW",
            ReasonCode::OfferedLoadLow => "OFFERED_LOAD_LOW",
        }
    }

    /// 这个码是不是「CPE 性能不达标」。
    ///
    /// 环境问题（工具没起来、窗口太短）**不该**算 CPE 的账——这是整套判定
    /// 规则里最要紧的一条区分，真实项目为此专门做过一次重构。
    pub const fn is_performance_failure(self) -> bool {
        matches!(self, ReasonCode::RxBelowTarget)
    }

    /// 把原因码翻译成给人的处置建议。对应真实项目的 `disposition_advice`。
    pub const fn disposition_advice(self) -> Option<&'static str> {
        match self {
            ReasonCode::RxBelowTarget => Some("CPE 没跑到门限，检查信号、信道或设备本身"),
            ReasonCode::NoStreamStarted => Some("测试工具没跑起来，检查对端是否在线、端口是否被占"),
            ReasonCode::EffectiveWindowShort => Some("有效测试时间太短，把 duration 调长再跑一次"),
            ReasonCode::SampleCoverageLow => {
                Some("采样有大量空洞，检查网卡计数器是否被其他程序干扰")
            }
            ReasonCode::TargetMissing => Some("计划里没有给出有效门限，先改配置"),
            ReasonCode::SingleUdpStreamFailed => {
                Some("必须灌通的单流 UDP 方向一个样本都没测到，先查对端和端口")
            }
            ReasonCode::ConfiguredLoadTooLow => {
                Some("计划里配的负载本来就没到门限，改大 bandwidth 再跑")
            }
            ReasonCode::OfferedLoadLow => Some("实际打出去的负载没到门限，查发送端是否被限速"),
            ReasonCode::None | ReasonCode::RxTargetMet => None,
        }
    }
}

impl fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ReasonCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "" => ReasonCode::None,
            "RX_TARGET_MET" => ReasonCode::RxTargetMet,
            "RX_BELOW_TARGET" => ReasonCode::RxBelowTarget,
            "NO_STREAM_STARTED" => ReasonCode::NoStreamStarted,
            "EFFECTIVE_WINDOW_SHORT" => ReasonCode::EffectiveWindowShort,
            "SAMPLE_COVERAGE_LOW" => ReasonCode::SampleCoverageLow,
            "TARGET_MISSING" => ReasonCode::TargetMissing,
            "SINGLE_UDP_STREAM_FAILED" => ReasonCode::SingleUdpStreamFailed,
            "CONFIGURED_LOAD_TOO_LOW" => ReasonCode::ConfiguredLoadTooLow,
            "OFFERED_LOAD_LOW" => ReasonCode::OfferedLoadLow,
            _ => return Err(()),
        })
    }
}

/// serde 表示 = `as_str()` 的结果，不是 serde 默认的驼峰变体名。
///
/// 理由和真实项目一样：这些串已经是对外兼容面了，rows.jsonl 再引入第二种
/// 拼法，等于让同一个概念在同一个产品里有两个名字。
mod reason_serde {
    use super::ReasonCode;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    impl Serialize for ReasonCode {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.as_str().serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for ReasonCode {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let raw = String::deserialize(deserializer)?;
            // 认不出来的串回落到 Default 而不是报错：重放器要容忍未来版本
            // 写出的新取值——宁可把一行显示成「无原因码」，也不要因为一个
            // 不认识的枚举值让整份历史报告读不出来。
            Ok(ReasonCode::from_str(&raw).unwrap_or_default())
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
    fn 每个码都能往返() {
        let all = [
            ReasonCode::None,
            ReasonCode::RxTargetMet,
            ReasonCode::RxBelowTarget,
            ReasonCode::NoStreamStarted,
            ReasonCode::EffectiveWindowShort,
            ReasonCode::SampleCoverageLow,
            ReasonCode::TargetMissing,
            ReasonCode::SingleUdpStreamFailed,
            ReasonCode::ConfiguredLoadTooLow,
            ReasonCode::OfferedLoadLow,
        ];
        for code in all {
            assert_eq!(
                ReasonCode::from_str(code.as_str()),
                Ok(code),
                "{code} 往返失败"
            );
        }
    }

    #[test]
    fn 只有RX低于门限算性能失败() {
        assert!(ReasonCode::RxBelowTarget.is_performance_failure());
        // 环境问题不算 CPE 的账
        assert!(!ReasonCode::NoStreamStarted.is_performance_failure());
        assert!(!ReasonCode::EffectiveWindowShort.is_performance_failure());
    }

    #[test]
    fn 不认识的串回落到None() {
        let code: ReasonCode = serde_json::from_str("\"FUTURE_CODE_2030\"").unwrap();
        assert_eq!(code, ReasonCode::None);
    }
}
