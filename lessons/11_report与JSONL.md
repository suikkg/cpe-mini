# 第 11 课：report —— Row、JSONL 与坏行容错

**目标**：理解报告层的职责边界和存储格式的选择。

## 1. 先跑

```bash
cargo run -- demo
head -1 output/rows.jsonl
cargo run -- report output/rows.jsonl
```

存下去再读回来，渲染结果一样。

## 2. 为什么是 JSONL 不是一个大 JSON 数组

| | JSONL（一行一条） | JSON 数组 |
|---|---|---|
| 跑到一半崩了 | 已写的行仍然可读 | 整个文件是坏的 |
| 追加一条 | 直接 append | 要重写整个文件 |
| 用 grep 筛 | 可以 | 不行 |
| 单行坏了 | 跳过那一行 | 整份读不出来 |

真实项目就是这么存的。

## 3. 坏行容错

```rust
match serde_json::from_str::<Row>(line) {
    Ok(row) => rows.push(row),
    Err(e) => skipped.push(format!("第 {} 行读不出来：{e}", i + 1)),
}
```

**跳过并记下来**，不是整份报错。
理由和第 03 课的枚举回落一样：宁可少一行，也不要因为一个格式问题让历史报告全废。

注意它**不是静默跳过** —— `main.rs` 会把 `skipped` 打到 stderr。
静默跳过和记录后跳过，差别是"用户知不知道自己少看了东西"。

## 4. 报告层不判定

`Row.verdict` 是执行侧算好写进来的，报告层原样显示。

真实项目早期在这里自带过一份回退判定，和 executor 的优先级对不上，
出过缺陷，后来整个删掉了。第 07 课讲过这件事。

`render()` 里唯一的"逻辑"是 `tally()` 数数，以及按原因码给处置建议 ——
都不改变任何判定。

## 5. `Row` 字段的挑选

这里只有 14 个字段，真实项目有 40 多个。但**字段名一致**，其中几个值得注意：

```rust
pub task_id: String,      // 这一行的 id
pub parent_id: String,    // 所属单元 —— 双向两行共享
pub round: u32,           // 轮次，对比报告的对齐键要用
pub diagnostics: Vec<String>,  // 不参与判定的排障线索
pub rx_avg: Option<f64>,  // None 而不是 0.0
pub udp_loss: Option<f64>,// 同上，而且它不参与判定（第 16 课）
```

`parent_id` 的作用：双向单元的 AB / BA 两行要能认回同一个单元，
否则报告里它们就是两条互不相干的测试。

### 一个 `Row` 里其实混着三类字段

读真实项目那 40 多个字段时，**先按这三类分堆**，能省一半力气：

| 类 | 例子 | 读代码时要不要细看 |
|---|---|---|
| **身份** | `task_id`、`parent_id`、`round` | 要。它们决定「这一行是谁」，改动代价最大 |
| **结论** | `verdict`、`reason_code`、`execution_status` | 要。判定链路的终点 |
| **证据/诊断** | `rx_avg`、`udp_loss`、`diagnostics`、`reason_detail` | **可以先跳过**。它们只解释「为什么」，不改变结论 |

第三类在真实 `Row` 里占了一大半（`tcp_retransmits`、`load_latency`、
`ping_*`、`tx_avg` …）。认出这一类，你一眼就知道哪些字段可以先不管。

> 这个分类不是我编的 —— 它就是 ADR-17（第 16 课）在字段层面的样子。

## 6. 中文对齐

```rust
fn display_width(s: &str) -> usize {
    s.chars().map(|c| if (c as u32) > 0x2000 { 2 } else { 1 }).sum()
}
```

中文字符在终端占 2 列，直接用 `len()`（字节数）或 `chars().count()`（字符数）
都会排歪。这是个粗糙实现（真实项目用 `unicode-width` crate），但够用，
而且它说明了一件事：**"长度"在不同语境下是三个不同的数。**

```text
"学习 Rust"
  .len()           = 11    字节数      —— 存文件、切片要用
  .chars().count() = 7     字符数      —— 算「几个字」要用
  display_width()  = 9     终端列宽    —— 排版对齐要用
```

（rust-starter 第 05 课讲了前两个，第三个只有做排版时才会撞上。）

**用错哪一个，症状都是「看起来歪了一点」**，不会报错，
所以只能靠知道有这三个数。

## 7. 渲染出来的样子也是兼容面

`render()` 打出来的那张表，看着只是给人看的，但：

- 有人会把它贴进周报
- 有人写脚本 `grep RATE_FAIL` 筛它
- 有人拿它和上一版的输出做 diff

改一个表头、把「无有效数据」改成「N/A」，这些全会坏，
而**现有的测试一条都不会红** —— 没有一条断言提到过表头。

**第 20 课**用「黄金文件」解决这件事：把整段输出存成文件逐字比对。
改动是故意的就一行命令更新，是手滑就当场红。

## 8. 动手任务

1. 在 `render()` 的表格里加一列「轮次」
2. 让 `report` 命令支持只看失败的：`cargo run -- report <文件> --failed-only`
3. 加一个测试：坏行被跳过后，`tally` 统计的是剩下的行

然后：

4. 手动把 `output/rows.jsonl` 的第 2 行改坏，跑 `cargo run -- report output/rows.jsonl`
5. 确认：警告打到了 stderr，其余两行照常显示

## 9. 验收

```bash
cargo test
cargo run -- report output/rows.jsonl --failed-only
```

- [ ] 新增的列和选项都能用
- [ ] 坏行有警告、不影响其余行
- [ ] 能说出 JSONL 相对 JSON 数组的三个好处
- [ ] 能解释报告层为什么不许自己判定
- [ ] 能把 `Row` 的字段分成「身份 / 结论 / 诊断」三堆

## 10. 对应真实项目

`src/report/model.rs:184` 的 `Row` —— 逐条读那些字段注释，
特别是标着「只作诊断，不参与判定」的那几个（`load_latency`、`tcp_retransmits`、
`udp_loss`）。那是 ADR-17 的落地。

`src/report/store.rs` 是落盘，`src/report/compare.rs` 是对比报告 ——
后者就是 `round` 字段存在的理由。
