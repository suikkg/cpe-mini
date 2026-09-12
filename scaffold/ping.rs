//! 扩展课 A（第 13 课）的骨架：真实执行一个 ping 进程。
//!
//! ## 怎么用
//!
//! ```bash
//! cp scaffold/ping.rs src/ping.rs
//! # 在 src/lib.rs 里加一行：pub mod ping;
//! cargo test --lib ping        # 现在全红，因为函数体还是 todo!()
//! ```
//!
//! 然后一个一个把 `todo!()` 换掉，跑到全绿。**测试已经写好了，一个包都不发。**
//!
//! 答案：`solutions/ping_answer.rs`（先自己写，卡住超过 15 分钟再看）
//!
//! ## 结构上唯一的硬要求
//!
//! **`parse` 必须和进程启动分开。** `run` 负责起进程拿文本，`parse` 只吃文本。
//! 这样解析能用固定样本单测，跑起来是毫秒级的，不用真的发包，
//! 也不会因为 CI 机器没网就红一片。
//!
//! 真实项目就是这么分的：`src/ping.rs` 的 `run`（第 104 行）和
//! `parse`（第 336 行）之间没有任何耦合。
#![allow(dead_code, unused_variables)]
// 函数体还是 todo!() 的时候 Command 还没用上，写完 run 就用上了。
#![allow(unused_imports)]

use std::process::Command;

/// ping 跑完的结果。
///
/// 字段名与真实项目的 `PingOut`（`src/protocol.rs:182`）一致。
///
/// 注意 `rtt_*` 是 `Option<f64>`：100% 丢包时一个 RTT 都没有，
/// 那是「没有这个数」，不是 0 毫秒。（第 16 课那条规则，又一次。）
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
/// 前者要查环境，后者是测量结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingExecErrorKind {
    /// 进程根本没起来（命令不存在、没权限）。
    Spawn,
    /// 起来了，但输出里没有能用的统计行。
    Execution,
}

/// 【1】拼命令行，返回 `(程序名, 参数列表)`。
///
/// macOS / Linux 是 `ping -c 3 主机`，Windows 是 `ping -n 3 主机`。
/// 用 `cfg!(windows)` 分支。
///
/// **为什么返回元组而不是拼好的字符串**：拼成字符串就得考虑带空格的主机名
/// 怎么转义；一个个传给 `Command` 根本不会有这个问题。
pub fn build(host: &str, count: u32) -> (String, Vec<String>) {
    todo!()
}

/// 【2】真的起一个进程去 ping。
///
/// 三个要点：
///
/// - 用 `Command::new(&program).args(&args).output()`。`output()` 等它跑完再
///   一次性拿输出，适合有限时长的命令；真实项目跑 iperf 用 `spawn()`，
///   因为那个要边跑边显示进度
/// - 起不来（`Err`）时**不要 panic**，返回一个 `ok: false` 的 `PingOut`，
///   并把原因写进 `raw`
/// - 用 `String::from_utf8_lossy` 而不是 `from_utf8`：外部程序的输出不保证
///   是合法 UTF-8（中文 Windows 上尤其）。坏字节换成 `�` 而不是报错 ——
///   **不能因为一个乱码字节丢掉整份测量结果**
///
/// 拿到文本之后交给 `parse`，再把 `cmd` 和 `raw` 填回去。
pub fn run(host: &str, count: u32) -> PingOut {
    todo!()
}

/// 【3】解析 ping 的输出。**这一个函数是这一课的重点。**
///
/// 要从两行里抓四个数：
///
/// ```text
/// 3 packets transmitted, 3 packets received, 0.0% packet loss
/// round-trip min/avg/max/stddev = 0.067/0.123/0.156/0.040 ms
/// ```
///
/// 建议拆成三个小函数（下面已经留好位置）：数应答行、取统计行、取 RTT。
///
/// 拿不到统计行时 `ok` 要是 `false`，并退回用 `count` 当发出数。
pub fn parse(text: &str, count: u32) -> PingOut {
    todo!()
}

/// 【4】执行层错误：起不来 / 起来了但没结果。`None` 表示执行本身没问题。
///
/// **注意「100% 丢包」不是执行错误** —— 那是一个有效的测量结果。
/// 把它当成错误，就等于告诉用户「测试没跑成」，而实际上跑成了，结论是「全丢」。
pub fn execution_error(out: &PingOut) -> Option<(PingExecErrorKind, String)> {
    todo!()
}

/// 【5】数带 RTT 的应答行（含 `time=` 的行）。
fn count_echo_replies(text: &str) -> u32 {
    todo!()
}

/// 【6】从统计行取 `(发出, 收到)`。拿不到就返回 `None`。
///
/// 提示：先 `lines().find(|l| l.contains("packets transmitted"))`，
/// 再按 `,` 切段，每段的第一个词就是数字。
/// **每一步都用 `?` 提前退出** —— 解析函数不该自己决定
/// 「解析不出来算不算失败」，那是调用方的事。
fn packet_summary(text: &str) -> Option<(u32, u32)> {
    todo!()
}

/// 【7】从 `round-trip min/avg/max/stddev = 0.067/0.123/0.156/0.040 ms` 取三个数。
///
/// 拿不到就三个都是 `None`。
fn parse_rtt(text: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    todo!()
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
