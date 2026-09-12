# 扩展课 A（第 13 课）答案

完整代码：**`solutions/ping_answer.rs`**（验证过：`cargo test --lib ping` 7 个全绿，clippy 零告警）

```bash
cp solutions/ping_answer.rs src/ping.rs
# src/lib.rs 里加 pub mod ping;
cargo test --lib ping
```

和自己写的比：

```bash
diff src/ping.rs solutions/ping_answer.rs
```

---

## 四个容易写错的地方

### 1. `parse` 必须和 `run` 分开

这是唯一的结构硬要求。分开之后，7 个测试全部用固定文本，**一个包都不发**，
跑起来是毫秒级的，CI 机器没网也能跑。

真实项目也是这么分的：`src/ping.rs` 的 `run`（第 104 行）和
`parse`（第 336 行）之间没有任何耦合。

### 2. 「100% 丢包」不是执行错误

```rust
pub fn execution_error(out: &PingOut) -> Option<(PingExecErrorKind, String)> {
    if out.raw.starts_with("ping 起不来") {
        return Some((PingExecErrorKind::Spawn, out.raw.clone()));
    }
    if !out.ok {
        return Some((PingExecErrorKind::Execution, "ping 跑了，但输出里没有统计行".to_string()));
    }
    None
}
```

`ping 192.0.2.1` 全丢包，**是一个有效的测量结果**。把它当成执行错误，
就等于告诉用户「测试没跑成」，而实际上跑成了，结论是「全丢」。

这和第 04 课那条「测出来不达标」vs「压根没测成」是同一条区分。

### 3. `from_utf8_lossy`，不是 `from_utf8`

外部程序的输出不保证是合法 UTF-8（中文 Windows 上尤其）。
`lossy` 把坏字节换成 `�` 而不是报错 ——
**不能因为一个乱码字节丢掉整份测量结果。**

真实项目为此专门引了 `encoding_rs`。

### 4. 没有 RTT 是 `None`，不是 `0.0`

```rust
pub rtt_avg: Option<f64>,
```

100% 丢包时一个 RTT 都没有。写成 `0.0` 的话，报告里会显示
「平均时延 0 毫秒」—— 一个完美的成绩，实际上一个包都没通。

第 16 课那条规则（空 ≠ 0），在这里第三次出现。

---

## `main.rs` 的 `ping` 分支

`use` 那一行加上 `ping`：

```rust
use cpe_mini::{compare, default_output, demo_plan_path, ping, preview_plan, report, run_plan};
```

`match mode` 里加：

```rust
"ping" => {
    let Some(host) = args.get(1) else {
        return fail("ping 后面要跟主机，例如：ping 127.0.0.1");
    };
    if host.trim().is_empty() {
        return fail("主机不能是空的");
    }
    let count: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);

    let out = ping::run(host, count);

    if let Some((kind, detail)) = ping::execution_error(&out) {
        // 「工具没起来」和「结果不好」分开报——前者要查环境
        return fail(&format!("ping 执行失败（{kind:?}）：{detail}"));
    }

    println!("模式：真实执行（ping）\n");
    println!("目标      {host}");
    println!("发 / 收   {} / {}", out.sent, out.received);
    println!("丢包率    {:.1}%", out.loss_pct);
    match (out.rtt_min, out.rtt_avg, out.rtt_max) {
        (Some(lo), Some(avg), Some(hi)) => {
            println!("时延      min {lo:.3} / avg {avg:.3} / max {hi:.3} ms")
        }
        // 一个 RTT 都没有：显示成「—」，不是 0
        _ => println!("时延      —"),
    }

    let pass = out.loss_pct == 0.0;
    println!("结果      {}", if pass { "PASS" } else { "FAIL" });

    if pass { 0 } else { 1 }
}
```

跑起来：

```text
$ cargo run -- ping 127.0.0.1 2
模式：真实执行（ping）

目标      127.0.0.1
发 / 收   2 / 2
丢包率    0.0%
时延      min 0.086 / avg 0.114 / max 0.143 ms
结果      PASS

$ cargo run -- ping 192.0.2.1 1
目标      192.0.2.1
发 / 收   1 / 0
丢包率    100.0%
时延      —
结果      FAIL
$ echo $?
1
```

---

## 一个值得想清楚的问题

这里丢包率**是**判定输入（丢一个包就 FAIL），而第 16 课里丢包
**不是**判定输入（丢 31% 照样 PASS）。矛盾吗？

不矛盾。**两个用例在问不同的问题**：

| 用例 | 问的是 | 判定输入 |
|---|---|---|
| 吞吐验收 | 这条链路能不能跑到 900 Mbps | 接收端 RX 平均 |
| 连通性检查 | 这条链路通不通 | 丢包率 |

同一个数字算不算判定输入，**取决于用例在问什么** ——
这是读真实代码时最需要的一种判断力。真实项目里 `PING_PACKET_LOSS_HIGH`
和 `UDP_LOSS_HIGH` 是两个不同的原因码，正是因为这个。

---

## 做完之后读真实代码

`src/ping.rs`（440 行）：

- `build`（第 47 行）—— 和你写的对比，它多处理了 IPv6、超时、包大小
- `run` / `run_cancellable`（第 104 / 121 行）—— 后者能被取消，看它怎么做到的
- `parse`（第 336 行）—— 它用 regex，而且要同时认中英文 Windows 的输出
- `execution_error_kind`（第 275 行）—— 分了三类：Spawn / Timeout / Execution
- `EXEC_ERROR_PREFIX`（第 10 行）—— 它把执行错误编码进 `raw` 字段，
  这样远端 agent 返回的结果也能还原分类。**看它为什么不加协议字段。**

**重点看它处理了多少你没想到的情况。** 那些就是真实工程和练习的差距。
