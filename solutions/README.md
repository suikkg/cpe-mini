# 动手任务答案要点

和 rust-starter 不一样：这里 `src/` 本身就是完整可跑的参考实现，
所以答案不是"完整代码"，而是**思路要点和容易踩的坑**。

先自己做，卡住再看。每做完一步都跑 `cargo test`。

## 这个目录里有什么

| 文件 | 对应 |
|---|---|
| 本文件 | 第 01–11、15–18、20 课的动手任务要点 |
| `13_ping.md` + `ping_answer.rs` | 扩展课 A 的**完整代码** |
| `14_webui.md` + `webui_answer.rs` / `.html` | 扩展课 B 的**完整代码** |
| `aggregate_special_answer.rs` | 第 19 课的**完整代码** |

**第 12 课（毕业考）没有答案**，这是有意的。

`.rs` 答案都验证过能编译、测试全绿 —— `./check.sh` 每次都会重新验一遍。

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

- 扩展课 13 → `src/ping.rs`
- 扩展课 14 → `src/master/webui/http.rs`

---

## 第 15 课：对比两份报告

**1. 对调 `Regressed` 和 `Fixed`**

红的是 `compare::tests::判定变坏排在最前面`。失败信息会说
`left: Fixed, right: Regressed`。

`DeltaKind` 的 `Ord` 是 derive 的，**按变体声明顺序比大小** ——
所以那个 enum 的顺序不是"排版"，是报告的排序规则。改它要当成改规则来改。

**2. 把门限加进对齐键**

红的是 `门限变了仍然对得上`：加了门限之后，同一条测试在两轮里成了两个键，
报告变成「缺失 1 + 新增 1」。

两种做法各自适合：

| 做法 | 适合 | 代价 |
|---|---|---|
| 键里**不含**门限（现在这样） | 门限会调的场景，能看到「门限变了所以判定变了」 | 改了门限的测试会被当成同一条比 |
| 键里**含**门限（真实项目） | 参数固定的回归跑，参数变了就是另一个测试 | 调一次门限，整张表变成全新增 |

选哪个都行，**但要在注释里写清楚为什么** —— 下一个读代码的人
（多半是三个月后的你）需要知道这是选择，不是疏忽。

**3. 加门限列**

`UnitSnapshot.target_mbps` 已经有了，改 `side()` 和表头即可。
注意两轮门限不同时要都显示得出来，否则这一列反而会误导。

**4. `--only-regressed`**

```rust
let only = args.iter().any(|a| a == "--only-regressed");
```

在 `render` 里过滤 `deltas`。**注意底部的统计还要数全部**，
否则「共 1 条」会让人以为只跑了一条。

**5. `plan_hash`（有点难）**

链路：`plan.rs` 算哈希 → `Unit` 带上 → `UnitOutcome` 带上 → `Row` 带上 →
`compare` 比较两边的值。

FNV-1a 二十行不到：

```rust
pub fn plan_hash(plan: &Plan) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |s: &str| {
        for b in s.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    };
    eat(&plan.plan_id);
    for spec in &plan.specs {
        eat(&spec.title);
        eat(&spec.transport);
        eat(&spec.direction);
        eat(&format!("{}", spec.target_mbps));
    }
    format!("{h:016x}")
}
```

**别把样本算进去**：样本是测量结果，不是计划身份。算进去的话同一份计划
跑两次就成了两份计划。

`Row` 上加字段记得靠 `#[serde(default)]` 兼容老报告（第 08 课那条）。

---

## 第 16 课：诊断通道

**1. 让丢包改判定**

红的是 `executor::tests::丢包再高也不改判定`。**它的名字就是答案**：
这条规则被写成了测试，不是写在文档里 —— 文档会过时，测试不会。

**2. 把 `Option<f64>` 改成 `f64`**

红的是 `没采到丢包和丢包为零是两回事`。改完之后，没采丢包的那条会变成
`0.0` —— 一份「完美零丢包」的报告，而且**没有任何报错**。

这类静默的错误最贵。

**3. 加 `tcp_retransmits`**

和丢包完全对称：`Spec` → `Leg` → `LegOutcome` → `Row`，
executor 里加一个阈值判断塞进 `diagnostics`。

必写的测试：

```rust
#[test]
fn 重传再多也不改判定() {
    // 重传 5000 次，但 RX 达标 → 仍然 PASS
}
```

**没有这个测试，这条规则就只存在于你的记忆里。**

**4. `MAX_UDP_LOSS_PCT` 改成 0.0**

每一行都会冒出一条诊断（只要采到了丢包），包括 0.1% 这种完全正常的。
诊断栏变成噪声，真正要紧的那条反而看不见了 ——
这就是真实项目把它做成配置项的理由：不同用例对「正常」的定义不一样。

---

## 第 17 课：CSV 导出

**1. 加 `coverage` 列**

`Row` 上还没有这个字段，先加（`coverage: Option<f64>`，从
`LegOutcome.window.coverage` 来），再加到 `CSV_COLUMNS` **末尾**和 `fields` 末尾。

只改一处的话，`debug_assert_eq!(fields.len(), CSV_COLUMNS.len())` 会当场炸 ——
故意试一次，看那条断言长什么样。

**2. `--only-failed`**

```rust
let rows: Vec<Row> = if only_failed {
    rows.into_iter()
        .filter(|r| matches!(r.verdict, Verdict::RateFail | Verdict::SetupError | Verdict::NotEvaluated))
        .collect()
} else {
    rows
};
```

注意筛选条件要和退出码那条一致（`main.rs` 里 `bad = rate_fail + setup_error + not_evaluated`）。
**两处口径不一样的话，「导出失败项」和「退出码说有失败」会对不上。**
更好的做法：把这个判断抽成一个函数，两处都调它 —— 第 16 课那条规矩。

**3. 逗号测试**

```rust
#[test]
fn 明细里有逗号也不错位() {
    let mut rows = 样例行();
    rows[0].reason_detail = "RX 平均 950 Mbps, 目标 900".to_string();
    let text = to_csv(&rows);
    let line = text.lines().nth(1).unwrap();
    assert!(line.contains("\"RX 平均 950 Mbps, 目标 900\""));
}
```

**4. 去掉 BOM**

在 macOS 的 Numbers 上看不出区别（它按 UTF-8 猜对了）。
中文 Windows 的 Excel 上整列中文会变成乱码 ——
这就是为什么这个坑只有在真实用户那里才会暴露。

---

## 第 18 课：取消

### 任务 A：把 `break` 写回去

`cargo test` 会挂三个（名字直接说了破坏的是哪条规则）：

```text
executor::tests::被取消的单元一个都不许少
executor::tests::取消之后的单元判SKIP
executor::tests::取消记在执行状态上而不是判定上
```

**但实际上挂的是四个。** 单元测试那个二进制一失败，cargo 就不往下跑了，
集成测试根本没开始。单独跑一次：

```bash
cargo test --test golden
```

```text
failures:
    取消之后剩下的单元还在报告里
```

那份黄金文件从六行掉到两行 —— 它钉的不是排版，是**行数**。

顺带记住这件事：**`cargo test` 全红的时候，看到的失败清单往往是不全的。**
修完第一批再跑一次，后面可能还有。

### 任务 B：摘要里说一声

`report::Tally` 加一个字段，`tally()` 里数一下：

```rust
pub struct Tally {
    // ... 原有的
    pub skip: usize,
}
```

```rust
Verdict::Skip => t.skip += 1,
```

`render` 的摘要行改成：

```rust
let mut line = format!("总计 {} 项：通过 {}，未达标 {}，无法评价 {}",
                       t.total, t.pass, t.rate_fail, t.not_evaluated);
if t.skip > 0 {
    line.push_str(&format!("，跳过 {}", t.skip));
}
```

**`if t.skip > 0` 那个判断是关键**：没有跳过的时候不要多打一句。
摘要每多一个永远为 0 的数字，真正要紧的那个就少一分注意力。

改完 `cargo test --test golden` 会红三个（三份计划里都有 SKIP 吗？
自己跑一下看是几个）。读完 diff 再 `UPDATE_GOLDEN=1`。

### 任务 C：加「跳过当前单元」

**一个标志不够。** 理由就是第 6 节引的那段真实注释：

跳过要复用同一套收尾路径，所以它也得设 `RUN_CANCELLED`；
但单元边界上要把这个标志清掉才能继续下一个。
于是「刚点了跳过、紧接着点停止」会被那次清零抹掉。

最小可行的两个标志：

```rust
pub struct CancelFlag {
    /// 「现在这个单元该收尾了」—— 跳过和停止都设它，单元边界清零
    run_cancelled: Arc<AtomicBool>,
    /// 「整轮真的要停」—— 只有停止设它，永不清零
    stop_requested: Arc<AtomicBool>,
}
```

执行循环：

```rust
if self.stop_requested() {
    // 后面全部 SKIP
} else if self.run_cancelled() {
    self.clear_run_cancelled();   // 只清这一个
    // 当前单元 SKIP，继续下一个
}
```

**清零的范围要和意图的生命周期对齐。** 一次性的意图（跳过这一个）
用可清零的标志，持久的意图（整轮停）用不清零的。

## 第 20 课：黄金文件

### 任务 A：钉住报错

```rust
#[test]
fn 非法配置的报错() {
    let err = cpe_mini::plan::load(std::path::Path::new("fixtures/plan_bad.json"))
        .expect_err("这份计划就是用来报错的");
    assert_golden("plan_bad.error.txt", &err);
}
```

`expect_err` 是 `expect` 的反面：期待 `Err`，拿到 `Ok` 就 panic。
用 `unwrap()` 写不出来这个意思。

第一次跑要 `UPDATE_GOLDEN=1` 生成文件，然后**读一遍生成的内容** ——
黄金文件的第一版是你唯一一次逐字读它的机会，
后面每次都只会读 diff。

### 任务 C：防住橡皮图章

几种常见做法，各有各的漏：

| 做法 | 挡得住 | 挡不住 |
|---|---|---|
| CI 上不认 `UPDATE_GOLDEN` | 「跑绿了就提交」 | 本地更新完再提交 |
| 更新时要求在提交信息里写理由 | 无意识的更新 | 写一句「更新黄金文件」 |
| 黄金文件列进 code review 必看清单 | 大部分 | 审的人也不看 |
| 黄金文件控制在 50 行以内 | **根源** | 输出本来就长的情况 |

最后一条最有效，也最容易被忽略：**diff 短到能一眼看完，人才会真的看。**
一个三百行的黄金文件，更新它的人一定是闭着眼按的。
