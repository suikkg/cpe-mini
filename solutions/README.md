# 动手任务答案要点

和 rust-starter 不一样：这里 `src/` 本身就是完整可跑的参考实现，
所以答案不是"完整代码"，而是**思路要点和容易踩的坑**。

先自己做，卡住再看。每做完一步都跑 `cargo test`。

---

## 第 01 课：加 `version` 分支

在 `real_main` 的 `match mode` 里加：

```rust
"version" | "-V" | "--version" => {
    println!("cpe-mini {}", env!("CARGO_PKG_VERSION"));
    0
}
```

`env!("CARGO_PKG_VERSION")` 在编译期读 `Cargo.toml` 的版本号，
比手写字符串好 —— 改版本号只改一处。

**「demoo」会走到哪个分支**：`other =>`，打印「不认识的命令」+ 用法，退出码 1。

---

## 第 02 课：把 `>=` 改成 `>`

失败的测试是 `verdict::tests::等于门限算通过`。失败信息会告诉你
`left: RateFail, right: Pass`。

新测试：

```rust
#[test]
fn 略高于门限也通过() {
    let r = rate_verdict(Some(900.1), 900.0, ReasonCode::None);
    assert_eq!(r.verdict, Verdict::Pass);
}
```

---

## 第 03 课：加 `Degraded` 变体

`cargo check` 只报 **2 处**：`verdict.rs:42`（`label()`）和 `report.rs:158`（`tally()`）。

**这一课真正的收获是编译器没报的那些地方**：

- `from_label()` 不会被提醒 —— 它 `match` 的是 `&str` 不是 `Verdict`，
  编译器无从知道你少了一个分支。往返测试会漏掉新变体，除非你手动加进那个数组。
- `aggregate_verdict` 也不会被提醒 —— 它用的是 `any()` 比较，不是 `match`。
  新变体在优先级里排第几，只能你自己想清楚。

**穷尽性检查只保护 `match` 在枚举上的那一半。** 另一半（从字符串反解、
用 `if`/`any` 比较的地方）得靠测试。这就是 `每个判定label都能往返`
那个循环测试存在的理由。

---

## 第 04 课：改 `MIN_COVERAGE` 为 0.9

`fixtures/plan_edge.json` 里「偶发抖动一个空洞」那一行会从 `PASS` 变成
`NOT_EVALUATED`：它的有效窗口是 `[1150, 0, 1120, 1130, 1140]`，
覆盖率 0.8 —— 过得了 0.6，过不了 0.9。其余几行不受影响
（它们要么没有空洞，要么空洞多到 0.6 也过不了）。

挂掉的测试是 `rate_window::tests::偶尔一个空洞不影响` —— 它保护的是
「偶发抖动不该判成采样失败」。阈值定得太严，正常测试会被大量误杀：
这一行明明测到了 908 Mbps，却因为一秒的调度抖动被报成「无法评价」。

加 `RxUnstable`：在 `reason.rs` 的 enum、`as_str`、`from_str`、
`disposition_advice` 四处各加一行。往返测试的数组也要加。

---

## 第 05 课：`&[f64]` 改成 `Vec<f64>`

报错会出现在 `executor.rs::execute_leg` 的调用点：
`expected Vec<f64>, found &[f64]`。要么在调用点 `.to_vec()`（多一次复制），
要么改回 `&[f64]`。**这就是 `&[T]` 更通用的直接体现。**

`time.clone()` 为什么必要：`rows_from` 里 `time` 是循环外算的一个 `String`，
循环里每一行 `Row` 都要拥有一份。不 clone 的话第一行就把它移动走了。

---

## 第 06 课：加 `warmup_secs` 校验

```rust
if plan.warmup_secs >= 100 {
    errors.push(format!("warmup_secs 看起来填错了，当前是 {}", plan.warmup_secs));
}
```

样本负数校验要带双重位置：

```rust
for (j, v) in spec.samples_ab.iter().enumerate() {
    if *v < 0.0 {
        errors.push(format!("{at}.samples_ab[{j}] 速率不能是负数，当前是 {v}"));
    }
}
```

---

## 第 07 课：制造一次「两份实现」

故意写错的版本：

```rust
fn fallback_verdict(rows: &[Row]) -> Verdict {
    if rows.iter().any(|r| r.verdict == Verdict::Pass) { return Verdict::Pass; }  // ← 错在这
    ...
}
```

一条 Pass + 一条 RateFail 时，它返回 `Pass`，而 `aggregate_verdict` 返回 `RateFail`。
在双向测试里，这意味着「下行好、上行断」被报成「通过」。

**任务 B 没有唯一答案。** 一种看法：`ExecutionStatus` 描述的是执行过程，
该由 executor 决定；但它现在是从 `verdict.code` 反推的，说明这两件事在
这一层还没完全分开。真实项目里 executor 有独立的状态跟踪，不靠反推。

---

## 第 08 课：加 `duration_secs`

只在 `Spec` 里加 `pub duration_secs: u32`。因为有 `#[serde(default)]`，
不填就是 0，旧 fixture 照常能读。

删掉 `#[serde(default)]` 后，所有 fixture 都会报 `missing field`——
这就是它保护的东西。

---

## 第 09 课：加 `"loop"` 方向

`builder.rs::build_legs` 加一个分支，`plan.rs::validate` 的 `matches!` 里放行。

改成"先规格后轮次"后挂掉的是 `builder::tests::展开顺序是先轮次后规格`。
它保护的是稳定性测试的语义。

---

## 第 10 课：加 `RX_UNSTABLE` 关卡

```rust
let max = window.iter().cloned().fold(f64::MIN, f64::max);
let min = window.iter().cloned().fold(f64::MAX, f64::min);
if min > 0.0 && max / min > 10.0 { → RxUnstable }
```

**放在覆盖率检查之后**：全是 0 的窗口应该报 `SAMPLE_COVERAGE_LOW`
（更具体的原因），而不是被抖动检查先截住。顺序决定用户看到哪个原因码，
而原因码决定他去查什么 —— 这不是排版问题。

注意 `min > 0.0` 这个前提：窗口里有 0 时 `max/min` 是无穷大。

---

## 第 11 课：加「轮次」列和 `--failed-only`

渲染加列时记得同步改表头和那条分隔线的长度。

`--failed-only`：

```rust
let rows: Vec<Row> = if failed_only {
    rows.into_iter().filter(|r| !r.verdict.is_pass()).collect()
} else { rows };
```

注意 `tally` 该统计**过滤前**还是过滤后？想清楚 ——
「总计 3 项：通过 1」里的 3 如果变成过滤后的数，这句话就没意义了。

---

## 第 12 课

**没有答案。** 那是毕业考 —— 前面 11 课教的东西够你自己做出来了。

---

## 扩展课 13、14

**也没有答案，而且是故意的。**

到了这一步，你要练的已经不是「照着答案改对」，而是**自己判断改得对不对**：
课程里给了骨架代码、要点和验收清单，剩下的靠 `cargo test` 和你自己的判断。

真卡住了，去读对应的真实项目代码 —— 那才是最权威的参考：

- 扩展课 13 → `/Users/kk/uv/cpe_test/main/src/ping.rs`
- 扩展课 14 → `/Users/kk/uv/cpe_test/main/src/master/webui/http.rs`
