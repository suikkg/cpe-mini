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

## 6. 这几行里藏着你学过的三件事

打开 `src/rate_window.rs` 看这三行，它们是 rust-starter 那边几个坑的真实用法：

```rust
let non_zero = window.iter().filter(|v| **v > 0.0).count();
let coverage = non_zero as f64 / window.len() as f64;
let sum: f64 = window.iter().sum();
```

**一、`as f64` 出现了两次，而且在除号两边**

`non_zero` 和 `window.len()` 都是 `usize`。不转的话：

```rust
let coverage = non_zero / window.len();     // 整数除法！
```

`3 / 5` 等于 **0**，`5 / 5` 等于 1 —— 覆盖率只有 0 和 1 两个值，
`coverage < MIN_COVERAGE` 这道关就变成了「只要有一个零样本就不通过」。

**而且编译器一个字都不会说**，因为它是合法代码。
（rust-starter 第 02 课那个坑，这就是它在真实代码里的样子。）

**二、`**v` 那两个星号**

`window.iter()` 给的是 `&f64`，`filter` 又借了一层，所以闭包里拿到 `&&f64`。
要和 `0.0` 比大小得先解两层引用。

写 `|v| v > 0.0` 编译不过，而且报错直接把答案写出来了：

```
error[E0308]: mismatched types
  |     .filter(|v| v > 0.0)
  |                     ^^^ expected `&&f64`, found floating-point number
```

`expected &&f64` —— 它明说了你手里是两层引用。
（rust-starter 第 13 课提过这个 `&&T`。）

**三、`let sum: f64 =` 那个类型标注不能省**

`sum()` 不知道你要加成什么类型。不标注就是
`error[E0282]: type annotations needed`。
另一种写法是 `window.iter().sum::<f64>()`。

> **读真实代码时，这类「为什么多写了两个字」的地方值得停一下。**
> 十次有九次，那两个字是被某个 bug 逼出来的。

## 7. 动手任务

1. 加一道关：如果窗口里最大值比最小值大 10 倍以上，判 `RX_UNSTABLE`
   （真实项目有这个码，字符串 `"RX_UNSTABLE"`）
2. 在 `reason.rs` 加上这个码和它的 `disposition_advice`
3. 写测试：`[900, 900, 50, 900, 900]` 应该触发
4. 确认 `[900, 910, 890, 900]` **不**触发

注意：这道关该放在覆盖率检查之前还是之后？想清楚再写 —— 顺序会影响原因码。

## 8. 验收

```bash
cargo test --lib rate_window
cargo run -- run fixtures/plan_edge.json
```

- [ ] 新关卡有测试，正反两个用例都覆盖
- [ ] 能解释为什么必须切爬坡
- [ ] 能说出三（现在四）道关各自对应哪个原因码
- [ ] 检查过自己写的代码有没有切片越界的可能
- [ ] 说得出 `coverage` 那一行少了 `as f64` 会发生什么（而且不会报错）

## 9. 对应真实项目

`src/master/rate_window.rs`（2035 行）。真实实现还要处理：
网卡累计计数器回绕、多网卡合并、滚动窗口、时钟漂移、
`RATE_WINDOW_COVERAGE_LOW` 和 `EFFECTIVE_WINDOW_SHORT` 的区分。

核心思路是一样的：**先确定哪一段数据可信，再在那一段上求平均。**
