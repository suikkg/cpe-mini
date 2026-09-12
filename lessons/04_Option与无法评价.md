# 第 04 课：`Option` 与「无法评价」

**目标**：搞清楚这个工具里最容易出错的一条业务区分。

## 1. 先跑

```bash
cargo run -- run fixtures/plan_edge.json
```

六行里有三行是 `NOT_EVALUATED`，原因各不相同。

## 2. 三种「测不出来」

| 情况 | 原因码 | 执行状态 |
|---|---|---|
| 一个样本都没有 | `NO_STREAM_STARTED` | `ERROR` |
| 切掉爬坡后窗口太短 | `EFFECTIVE_WINDOW_SHORT` | `PARTIAL` |
| 窗口里空洞太多 | `SAMPLE_COVERAGE_LOW` | `COMPLETED` |

判定都是 `NOT_EVALUATED`，但**原因码不同**，所以给用户的建议也不同 ——
看 `ReasonCode::disposition_advice`。

## 3. `Option<f64>` 表达的是什么

```rust
pub struct RateWindow {
    pub rx_avg: Option<f64>,   // None = 没形成可信平均
    pub code: ReasonCode,       // 为什么没形成
    ...
}
```

如果 `rx_avg` 是 `f64` 而不是 `Option<f64>`，"没测到"就只能用 `0.0` 表示。
而 `0.0 < 900.0`，于是**每一条没测成的测试都会被判成 CPE 性能不达标**。

`Option` 逼着每个用到这个值的地方都先回答"没有的时候怎么办"。

## 4. 判定和执行状态是**正交**的

```rust
pub verdict: Verdict,              // 达标了吗
pub execution_status: ExecutionStatus,   // 跑完了吗
```

一次 `COMPLETED` 的执行完全可以判出 `RATE_FAIL`（跑得很顺，就是速度不够）；
一次 `ERROR` 的执行判 `NOT_EVALUATED`（压根没跑成，无从判起）。

把这两件事塞进一个字段，就没法区分"设备不行"和"环境不行"了。

## 5. 动手任务

1. 打开 `src/rate_window.rs`，把 `MIN_COVERAGE` 从 `0.6` 改成 `0.9`
2. 跑 `cargo run -- run fixtures/plan_edge.json`，看哪一行的结果变了，
   以及**为什么只有那一行变** —— 其余几行的覆盖率各是多少？
3. 跑 `cargo test`，看哪个测试挂了，读懂它在保护什么
4. 改回 `0.6`

然后：

5. 在 `src/reason.rs` 里加一个码 `RxUnstable`（真实项目里有这个码，
   字符串是 `"RX_UNSTABLE"`），给它写一条 `disposition_advice`
6. `cargo test` 确认往返测试仍然通过

## 6. 验收

- [ ] 能说出三种 `NOT_EVALUATED` 的区别，以及各自该怎么处理
- [ ] 能解释如果 `rx_avg` 用 `0.0` 代替 `None` 会出什么事
- [ ] 能举例说明「判定」和「执行状态」正交是什么意思

## 7. 对应真实项目

`src/verdict.rs` 的 `VerdictResult` 注释（第 167 行往下）详细写了这个设计。
真实项目还有 ADR-17：UDP 丢包、TCP 重传、负载下时延都**不参与判定**，
只进 `diagnostics` —— 下一课看这个。
