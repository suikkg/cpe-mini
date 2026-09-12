# 第 10 课：rate_window —— 有效窗口为什么要切爬坡

**目标**：理解这个工具里唯一一处"真正的算法"。

## 1. 先跑

```bash
cargo test --lib rate_window
```

特别看 `爬坡段会拉低平均_所以必须切掉` 这个测试。

## 2. 问题

测试刚起来的几秒是 TCP 慢启动 / 工具预热，速率必然偏低：

```text
样本：[100, 500, 940, 950, 960, 950]
全部平均 = 733  → 判 RATE_FAIL ❌
切掉前 2 个 = 950 → 判 PASS ✅
```

**同一条链路，切不切爬坡，结论完全相反。**

## 3. 三道关

```rust
pub fn effective_rx_avg(samples: &[f64], warmup_secs: usize) -> RateWindow {
    if samples.is_empty() { → NoStreamStarted }

    let window = &samples[warmup_secs..];
    if window.len() < MIN_EFFECTIVE_SAMPLES { → EffectiveWindowShort }

    let coverage = 非零样本数 / 窗口长度;
    if coverage < MIN_COVERAGE { → SampleCoverageLow }

    平均值
}
```

每道关对应一个原因码。**"测出来不达标"和"压根没测成"在报告里必须分得开**——
这是第 04 课那条区分的实现处。

## 4. 两个常数

```rust
pub const MIN_EFFECTIVE_SAMPLES: usize = 3;
pub const MIN_COVERAGE: f64 = 0.6;
```

写成 `const` 而不是散落的字面量：改阈值只改一处，测试里也能直接引用它
（看 `空洞太多是SAMPLE_COVERAGE_LOW` 那个断言）。

**这两个数是教学值。** 真实项目的阈值是从实测数据里定出来的，
而且滚动窗口的算法复杂得多。

## 5. 一个容易漏的边界

```rust
let window: &[f64] = if warmup_secs >= samples.len() { &[] } else { &samples[warmup_secs..] };
```

如果直接写 `&samples[warmup_secs..]`，当 `warmup_secs > samples.len()` 时
**会直接 panic**（切片越界）。用户填了个大数字，程序就崩了。

`tests::爬坡比样本还长也是窗口太短` 钉住了这条。
**凡是用下标或切片的地方，先问一句"越界会怎样"。**

## 6. 动手任务

1. 加一道关：如果窗口里最大值比最小值大 10 倍以上，判 `RX_UNSTABLE`
   （真实项目有这个码，字符串 `"RX_UNSTABLE"`）
2. 在 `reason.rs` 加上这个码和它的 `disposition_advice`
3. 写测试：`[900, 900, 50, 900, 900]` 应该触发
4. 确认 `[900, 910, 890, 900]` **不**触发

注意：这道关该放在覆盖率检查之前还是之后？想清楚再写 —— 顺序会影响原因码。

## 7. 验收

```bash
cargo test --lib rate_window
cargo run -- run fixtures/plan_edge.json
```

- [ ] 新关卡有测试，正反两个用例都覆盖
- [ ] 能解释为什么必须切爬坡
- [ ] 能说出三（现在四）道关各自对应哪个原因码
- [ ] 检查过自己写的代码有没有切片越界的可能

## 8. 对应真实项目

`src/master/rate_window.rs`（2035 行）。真实实现还要处理：
网卡累计计数器回绕、多网卡合并、滚动窗口、时钟漂移、
`RATE_WINDOW_COVERAGE_LOW` 和 `EFFECTIVE_WINDOW_SHORT` 的区分。

核心思路是一样的：**先确定哪一段数据可信，再在那一段上求平均。**
