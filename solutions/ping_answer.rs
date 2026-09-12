//! 扩展课 A（第 13 课）的完整答案：真实执行一个 ping 进程。
//!
//! 把这个文件拷成 `src/ping.rs`，在 `src/lib.rs` 里加 `pub mod ping;`，
//! 再在 `src/main.rs` 的 `match mode` 里加一个 `"ping"` 分支就能跑。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/ping.rs`（真实文件 440 行）。
//! `PingOut` 的字段名与真实项目的 `src/protocol.rs:182` 一致，
//! `build` / `run` / `parse` / `execution_error` 四个函数名也一致。
//!
//! 真实版多出来的东西：regex 解析（中英文 Windows 输出）、可取消的 `run_cancellable`、
//! 超时、逐包行采集、`resolution_caveat`。骨架是同一个。

use std::process::Command;

/// ping 跑完的结果。
///
/// 注意 `rtt_*` 是 `Option<f64>`：100% 丢包时一个 RTT 都没有，
/// 那是「没有这个数」，不是 0 毫秒。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PingOut {
    /// 进程起起来了、而且拿到了可用的统计行。
    pub ok: bool,
    pub sent: u32,
    pub received: u32,
    pub lost: u32,
    pub loss_pct: f64,
    pub rtt_min: Option<f64>,
    pub rtt_avg: Option<f64>,
    pub rtt_max: Option<f64>,
    /// 实际执行的命令行，出问题时贴给用户看。
    pub cmd: String,
    /// 原始输出。**一定要留着** —— 解析不出来的时候，它是唯一的线索。
    pub raw: String,
}

/// 执行层错误的分类。
///
/// 「工具没起来」和「工具跑了但结果不好」是两件事，必须分开 ——
/// 前者要查环境，后者是测量结果。真实项目的 `PingExecErrorKind`
/// （`src/ping.rs:15`）分了三类，这里取两类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingExecErrorKind {
    /// 进程根本没起来（命令不存在、没权限）。
    Spawn,
    /// 起来了，但输出里没有能用的统计行。
    Execution,
}

/// 拼命令行。
///
/// 返回 `(程序名, 参数列表)` 而不是一个拼好的字符串 ——
/// 真实项目也是这么做的（`src/ping.rs:47`）。理由：拼成字符串就得考虑
/// 带空格的主机名怎么转义，交给 `Command` 一个个传参数根本不会有这个问题。
pub fn build(host: &str, count: u32) -> (String, Vec<String>) {
    let flag = if cfg!(windows) { "-n" } else { "-c" };
    (
        "ping".to_string(),
        vec![flag.to_string(), count.to_string(), host.to_string()],
    )
}

/// 真的起一个进程去 ping。
pub fn run(host: &str, count: u32) -> PingOut {
    let (program, args) = build(host, count);
    let cmd = format!("{program} {}", args.join(" "));

    // output() 等它跑完再一次性拿输出，适合有限时长的命令。
    // 真实项目跑 iperf 用 spawn()：那个要边跑边显示进度。
    let output = match Command::new(&program).args(&args).output() {
        Ok(o) => o,
        Err(e) => {
            // 进程没起来。这不是「丢包 100%」，是环境问题，要分开报。
            return PingOut {
                ok: false,
                sent: count,
                loss_pct: 100.0,
                cmd,
                raw: format!("ping 起不来：{e}"),
                ..Default::default()
            };
        }
    };

    // from_utf8_lossy 而不是 from_utf8：外部程序的输出不保证是合法 UTF-8
    // （中文 Windows 上尤其）。坏字节换成 � 而不是报错——
    // **不能因为一个乱码字节丢掉整份测量结果。**
    let mut raw = String::from_utf8_lossy(&output.stdout).into_owned();
    let err = String::from_utf8_lossy(&output.stderr);
    if !err.trim().is_empty() {
        raw.push('\n');
        raw.push_str(&err);
    }

    let mut out = parse(&raw, count);
    out.cmd = cmd;
    out.raw = raw;
    out
}

/// 解析 ping 的输出。
///
/// **和进程启动分开**是这一课最重要的结构决定：解析能用固定文本单测，
/// 一个包都不用发，跑起来是毫秒级的。
pub fn parse(text: &str, count: u32) -> PingOut {
    let echo_replies = count_echo_replies(text);
    let summary = packet_summary(text);

    let (sent, summary_received) = summary.unwrap_or((count, echo_replies));
    // 接收数以**带 RTT 的应答行**为准，统计行只用来约束上限。
    // 理由和真实项目一样：有些系统会把「不可达」这类 ICMP 错误应答也算进 received。
    let received = echo_replies.min(summary_received).min(sent);
    let lost = sent.saturating_sub(received);
    let loss_pct = if sent > 0 {
        lost as f64 / sent as f64 * 100.0
    } else {
        100.0
    };

    let (rtt_min, rtt_avg, rtt_max) = parse_rtt(text);

    PingOut {
        // 有统计行才算跑成了。没有统计行说明输出被截断或者压根没跑起来。
        ok: summary.is_some(),
        sent,
        received,
        lost,
        loss_pct,
        rtt_min,
        rtt_avg,
        rtt_max,
        cmd: String::new(),
        raw: String::new(),
    }
}

/// 执行层错误：起不来 / 起来了但没结果。返回 `None` 表示执行本身没问题。
///
/// 注意「100% 丢包」**不是**执行错误 —— 那是一个有效的测量结果。
pub fn execution_error(out: &PingOut) -> Option<(PingExecErrorKind, String)> {
    if out.raw.starts_with("ping 起不来") {
        return Some((PingExecErrorKind::Spawn, out.raw.clone()));
    }
    if !out.ok {
        return Some((
            PingExecErrorKind::Execution,
            "ping 跑了，但输出里没有统计行".to_string(),
        ));
    }
    None
}

/// 数带 RTT 的应答行。
fn count_echo_replies(text: &str) -> u32 {
    text.lines()
        .filter(|l| l.contains("time=") || l.contains("time<") || l.contains("时间"))
        .count() as u32
}

/// 从统计行取 (发出, 收到)。
///
/// ```text
/// 3 packets transmitted, 3 packets received, 0.0% packet loss
/// ```
fn packet_summary(text: &str) -> Option<(u32, u32)> {
    let line = text
        .lines()
        .find(|l| l.contains("packets transmitted") || l.contains("已发送"))?;

    let mut sent = None;
    let mut received = None;

    for field in line.split(',') {
        let field = field.trim();
        // 每段的第一个词就是数字：「3 packets transmitted」
        let Some(n) = field.split_whitespace().next().and_then(|w| w.parse().ok()) else {
            continue;
        };
        if field.contains("transmitted") || field.contains("已发送") {
            sent = Some(n);
        } else if field.contains("received") || field.contains("已接收") {
            received = Some(n);
        }
    }

    Some((sent?, received?))
}

/// 从这一行取三个数：
///
/// ```text
/// round-trip min/avg/max/stddev = 0.067/0.123/0.156/0.040 ms
/// ```
fn parse_rtt(text: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    let Some(line) = text
        .lines()
        .find(|l| l.contains("min/avg/max") || l.contains("rtt"))
    else {
        return (None, None, None);
    };

    let Some(values) = line.split('=').nth(1) else {
        return (None, None, None);
    };

    // 每一步都可能失败，失败就返回 None —— 解析函数不该自己决定
    // 「解析不出来算不算失败」，那是调用方的事。
    let nums: Vec<Option<f64>> = values
        .trim()
        .trim_end_matches(" ms")
        .split('/')
        .map(|s| s.trim().parse::<f64>().ok())
        .collect();

    if nums.len() < 3 {
        return (None, None, None);
    }
    (nums[0], nums[1], nums[2])
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    // 全部用固定文本，一个包都不发。跑起来是毫秒级的。
    const 正常: &str = "\
PING 127.0.0.1 (127.0.0.1): 56 data bytes
64 bytes from 127.0.0.1: icmp_seq=0 ttl=64 time=0.067 ms
64 bytes from 127.0.0.1: icmp_seq=1 ttl=64 time=0.146 ms
64 bytes from 127.0.0.1: icmp_seq=2 ttl=64 time=0.156 ms

--- 127.0.0.1 ping statistics ---
3 packets transmitted, 3 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 0.067/0.123/0.156/0.040 ms
";

    const 全丢: &str = "\
PING 192.0.2.1 (192.0.2.1): 56 data bytes

--- 192.0.2.1 ping statistics ---
1 packets transmitted, 0 packets received, 100.0% packet loss
";

    #[test]
    fn 正常输出解析出四个数() {
        let out = parse(正常, 3);
        assert!(out.ok);
        assert_eq!(out.sent, 3);
        assert_eq!(out.received, 3);
        assert_eq!(out.lost, 0);
        assert_eq!(out.loss_pct, 0.0);
        assert_eq!(out.rtt_min, Some(0.067));
        assert_eq!(out.rtt_avg, Some(0.123));
        assert_eq!(out.rtt_max, Some(0.156));
    }

    #[test]
    fn 全丢包是有效结果不是执行错误() {
        let out = parse(全丢, 1);
        assert!(out.ok, "有统计行就算跑成了");
        assert_eq!(out.received, 0);
        assert_eq!(out.loss_pct, 100.0);
        // 一个 RTT 都没有 —— 是 None，不是 0.0
        assert_eq!(out.rtt_avg, None, "没有 RTT 是「没有这个数」，不是 0 毫秒");
        assert_eq!(
            execution_error(&out),
            None,
            "100% 丢包是测量结果，不是执行错误"
        );
    }

    #[test]
    fn 输出被截断时不崩溃() {
        let out = parse(
            "PING 127.0.0.1 (127.0.0.1): 56 data bytes\n64 bytes from 127.0",
            3,
        );
        assert!(!out.ok, "没有统计行 = 没跑成");
        assert_eq!(out.sent, 3, "退回用请求的次数");
        assert!(matches!(
            execution_error(&out),
            Some((PingExecErrorKind::Execution, _))
        ));
    }

    #[test]
    fn 空输出不崩溃() {
        let out = parse("", 3);
        assert!(!out.ok);
        assert_eq!(out.received, 0);
        assert_eq!(out.loss_pct, 100.0);
    }

    #[test]
    fn 含乱码也能解析出统计行() {
        // from_utf8_lossy 会把坏字节换成 �。一个乱码字节不该丢掉整份结果。
        let text = 正常.replace("icmp_seq=1", "icmp\u{fffd}seq=1");
        let out = parse(&text, 3);
        assert!(out.ok);
        assert_eq!(out.rtt_avg, Some(0.123));
    }

    #[test]
    fn 统计行说收到3但只有2条应答时以应答为准() {
        let text = "\
64 bytes from 127.0.0.1: icmp_seq=0 ttl=64 time=0.067 ms
64 bytes from 127.0.0.1: icmp_seq=1 ttl=64 time=0.146 ms

--- 127.0.0.1 ping statistics ---
3 packets transmitted, 3 packets received, 0.0% packet loss
";
        let out = parse(text, 3);
        assert_eq!(out.received, 2, "接收数以带 RTT 的应答行为准");
        assert_eq!(out.lost, 1);
    }

    #[test]
    fn 命令行参数按平台拼() {
        let (program, args) = build("127.0.0.1", 3);
        assert_eq!(program, "ping");
        assert_eq!(args[1], "3");
        assert_eq!(args[2], "127.0.0.1");
        assert_eq!(args[0], if cfg!(windows) { "-n" } else { "-c" });
    }
}
