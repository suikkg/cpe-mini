//! 有效窗口与 RX 平均。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/master/rate_window.rs`（真实文件 2035 行）。
//! 真实实现要处理网卡累计计数器回绕、多网卡合并、滚动窗口、时钟漂移；
//! 这里只保留最核心的那条逻辑：**去掉爬坡段，检查窗口够不够长和采样够不够密，
//! 然后求平均**。
//!
//! ## 为什么不直接对全部样本求平均
//!
//! 测试刚起来的几秒是 TCP 慢启动 / 工具预热，速率必然偏低。把这几秒算进去，
//! 一条本来达标的链路会被判成不达标。所以要先切掉爬坡段（`warmup_secs`），
//! 剩下的才叫**有效窗口**。

use crate::reason::ReasonCode;

/// 有效窗口至少要有这么多个样本，否则平均值没有意义。
pub const MIN_EFFECTIVE_SAMPLES: usize = 3;

/// 有效窗口里非零样本的占比至少要到这个数。
///
/// 0 表示那一秒网卡计数器没有增长——偶尔一两个是正常的（调度抖动），
/// 大面积出现说明链路断过或者采样出了问题，这时的平均值不可信。
pub const MIN_COVERAGE: f64 = 0.6;

/// 一次窗口计算的结果。
#[derive(Debug, Clone, PartialEq)]
pub struct RateWindow {
    /// 有效窗口的 RX 平均，单位 Mbps。`None` = 没形成可信平均。
    pub rx_avg: Option<f64>,
    /// 没形成平均时的原因码；形成了就是 `None` 码。
    pub code: ReasonCode,
    /// 有效窗口里有几个样本。
    pub effective_samples: usize,
    /// 非零样本占比。
    pub coverage: f64,
}

/// 从逐秒样本算出有效窗口 RX 平均。
///
/// `samples` 是接收端网卡逐秒的 RX 速率（Mbps），`warmup_secs` 是要切掉的爬坡秒数。
///
/// 三种拿不到平均值的情况，各自对应一个原因码 —— 这一点很重要：
/// 「测出来不达标」和「压根没测成」在报告里必须分得开。
pub fn effective_rx_avg(samples: &[f64], warmup_secs: usize) -> RateWindow {
    // 1. 一个样本都没有：工具没起来，或者对端没连上。
    if samples.is_empty() {
        return RateWindow {
            rx_avg: None,
            code: ReasonCode::NoStreamStarted,
            effective_samples: 0,
            coverage: 0.0,
        };
    }

    // 2. 切掉爬坡段。样本比爬坡还短，等于什么都没剩下。
    let window: &[f64] = if warmup_secs >= samples.len() {
        &[]
    } else {
        &samples[warmup_secs..]
    };

    if window.len() < MIN_EFFECTIVE_SAMPLES {
        return RateWindow {
            rx_avg: None,
            code: ReasonCode::EffectiveWindowShort,
            effective_samples: window.len(),
            coverage: 0.0,
        };
    }

    // 3. 覆盖率：非零样本占比太低，说明链路断过，平均值不可信。
    let non_zero = window.iter().filter(|v| **v > 0.0).count();
    let coverage = non_zero as f64 / window.len() as f64;

    if coverage < MIN_COVERAGE {
        return RateWindow {
            rx_avg: None,
            code: ReasonCode::SampleCoverageLow,
            effective_samples: window.len(),
            coverage,
        };
    }

    let sum: f64 = window.iter().sum();
    let avg = sum / window.len() as f64;

    RateWindow {
        rx_avg: Some(avg),
        code: ReasonCode::None,
        effective_samples: window.len(),
        coverage,
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
    fn 正常样本算出平均() {
        // 前 2 个是爬坡，会被切掉；剩下 4 个的平均是 950
        let samples = vec![100.0, 500.0, 940.0, 950.0, 960.0, 950.0];
        let w = effective_rx_avg(&samples, 2);
        assert_eq!(w.effective_samples, 4);
        assert_eq!(w.rx_avg, Some(950.0));
        assert_eq!(w.code, ReasonCode::None);
    }

    #[test]
    fn 爬坡段会拉低平均_所以必须切掉() {
        let samples = vec![100.0, 500.0, 940.0, 950.0, 960.0, 950.0];
        let 切了 = effective_rx_avg(&samples, 2).rx_avg.unwrap();
        let 没切 = effective_rx_avg(&samples, 0).rx_avg.unwrap();
        assert!(切了 > 没切, "切掉爬坡后平均应该更高：{切了} vs {没切}");
        // 没切的话 733 会被判成不达标，切了的 950 才是真实水平
        assert!(没切 < 900.0 && 切了 > 900.0);
    }

    #[test]
    fn 没有样本是NO_STREAM_STARTED() {
        let w = effective_rx_avg(&[], 2);
        assert_eq!(w.rx_avg, None);
        assert_eq!(w.code, ReasonCode::NoStreamStarted);
    }

    #[test]
    fn 窗口太短是EFFECTIVE_WINDOW_SHORT() {
        // 5 个样本切掉 3 个爬坡，只剩 2 个，不够 MIN_EFFECTIVE_SAMPLES
        let w = effective_rx_avg(&[10.0, 20.0, 30.0, 40.0, 50.0], 3);
        assert_eq!(w.rx_avg, None);
        assert_eq!(w.code, ReasonCode::EffectiveWindowShort);
    }

    #[test]
    fn 爬坡比样本还长也是窗口太短() {
        let w = effective_rx_avg(&[10.0, 20.0], 5);
        assert_eq!(w.code, ReasonCode::EffectiveWindowShort);
        assert_eq!(w.effective_samples, 0);
    }

    #[test]
    fn 空洞太多是SAMPLE_COVERAGE_LOW() {
        // 6 个样本里 4 个是 0，覆盖率 0.33 < 0.6
        let w = effective_rx_avg(&[900.0, 0.0, 0.0, 0.0, 0.0, 900.0], 0);
        assert_eq!(w.rx_avg, None);
        assert_eq!(w.code, ReasonCode::SampleCoverageLow);
        assert!(w.coverage < MIN_COVERAGE);
    }

    #[test]
    fn 偶尔一个空洞不影响() {
        // 5 个里 1 个 0，覆盖率 0.8 >= 0.6，照常出平均
        let w = effective_rx_avg(&[900.0, 900.0, 0.0, 900.0, 900.0], 0);
        assert!(w.rx_avg.is_some());
        assert_eq!(w.code, ReasonCode::None);
    }
}
