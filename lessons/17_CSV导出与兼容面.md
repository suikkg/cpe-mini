# 第 17 课：CSV 导出 —— 什么叫「对外兼容面」

**先决条件**：12 课做完。这一课最短，一次就能做完。

**目标**：搞清楚「对外兼容面」这个词具体指什么，以及破坏它的后果为什么
总是**静默的**。

## 1. 先跑

```bash
cargo run -- run fixtures/plan_loss.json output/loss.jsonl
cargo run -- csv output/loss.jsonl output/loss.csv
head -2 output/loss.csv
```

```text
time,task_id,parent_id,task,transport,param,verdict,execution_status,reason_code,reason_detail,diagnostics,round,rx_avg,udp_loss
1789254002,loss-demo-0-r1,loss-demo-0-r1,UDP 高丢包但速率达标,udp,udp / ab / 900 Mbps,PASS,COMPLETED,RX_TARGET_MET,RX 平均 950 Mbps >= 目标 900 Mbps,有效窗口 6 秒，采样覆盖率 100%; UDP_LOSS_HIGH: ...,1,950.3,31.4
```

## 2. 什么是对外兼容面

**别人已经依赖上的东西。** 一旦有人依赖，它就不再只是你的实现细节。

这个项目里有四处：

| 兼容面 | 谁在依赖 | 改了会怎样 |
|---|---|---|
| `Verdict` 的 label 串（`"RATE_FAIL"`） | 历史 rows.jsonl、下游脚本 | 老报告读不出来 |
| `ReasonCode` 的字符串 | 同上 | 同上 |
| rows.jsonl 的字段名 | 重放器、对比报告 | 字段静默变成默认值 |
| **CSV 的列顺序** | `awk -F, '{print $7}'` 这类脚本 | **静默取错值** |

前三个第 03 / 08 / 11 课讲过。这一课讲第四个，因为它的失效方式最阴。

## 3. 列顺序

```rust
pub const CSV_COLUMNS: &[&str] = &[
    "time", "task_id", "parent_id", "task", "transport", "param",
    "verdict", ...
];
```

有人写了个脚本统计通过率：

```bash
awk -F, 'NR>1 && $7=="PASS"' rows.csv | wc -l
```

你在 `param` 前面插了一列 `plan_id`。现在 `$7` 是 `param`，
永远不等于 `"PASS"`，脚本报告通过率 0% —— **不报错，不崩溃，就是安静地错。**

规矩：**要加字段就加在末尾**。和 rows.jsonl 加字段是同一条。

代码里用一个 `debug_assert_eq!` 钉住列数：

```rust
debug_assert_eq!(fields.len(), CSV_COLUMNS.len(), "列数和表头对不上");
```

加了表头忘了加值（或者反过来）会在测试里当场炸掉，而不是在用户的 Excel 里。

## 4. 转义：漏了它，后果也是静默的

RFC 4180 的规则很短：字段里含**逗号、双引号、换行**三者之一，就整个用双引号
包起来，里面的双引号写成两个。

```rust
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
```

**动手**：把 `s.contains(',')` 那一项删掉，跑
`cargo test --lib report`，看 `标题里有逗号也不会把整行挤歪` 怎么红的。

一条标题里带逗号的测试项，会把整行往右挤一格，后面所有列的值都错位 ——
而文件本身照样能打开，Excel 照样能显示。

## 5. 开头那三个看不见的字节

```rust
let mut out = String::from("\u{feff}");
```

BOM。Excel 在中文 Windows 上打开**无 BOM** 的 UTF-8 CSV 会按 GBK 解码，
整列中文变成乱码。用户的反馈是「你们的报告导出坏了」，
而实际上文件完全正确。

**三个字节省掉一轮扯皮。** 这类知识不写在任何教程里，只写在踩过的人身上。

```bash
cargo run -- csv output/loss.jsonl output/loss.csv
xxd output/loss.csv | head -1      # 开头是 efbb bf
```

## 6. 空 ≠ 0

```rust
fn opt_num(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.1}"),
        None => String::new(),   // 不是 "0"
    }
}
```

第 16 课那条规则在这里再出现一次：**空的意思是「没有这个数」，
0 的意思是「测出来是 0」。** 写成 0 的报告没法用来排障。

打开 `output/loss.csv`，最后一列第三行是空的（那条 TCP 没采丢包），
第二行是 `0.0`（那条 UDP 真的一个包没丢）。

## 7. 数组怎么塞进一维表格

```rust
row.diagnostics.join("; ")
```

`diagnostics` 是 `Vec<String>`，CSV 只有一维。用 `;` 而不是 `,` ——
用 `,` 的话它会被转义成带引号的一整格，表格里读起来反而更乱。

**有损是有意的。** CSV 是给人打开看的出口，不是数据交换格式；
要完整数据就读 rows.jsonl。搞混这两者的出口，就会想给 CSV 加嵌套结构。

## 8. 动手任务

1. 给 CSV 加一列 `coverage`（采样覆盖率）。注意加在**末尾**，
   并且两处都要改（`CSV_COLUMNS` 和 `fields`）—— 只改一处会被
   `debug_assert_eq!` 当场抓住，试一次看看。
   （`Row` 上还没有这个字段，先想清楚它该从哪来。）
2. 让 `csv` 命令支持 `--only-failed`，只导出有问题的行。
3. 加一个测试：`reason_detail` 里含逗号时（`RX 平均 950 Mbps, 目标 900`）
   整行不错位。
4. 把 BOM 去掉，用 Excel 或 Numbers 打开看看（如果手边有中文 Windows，
   效果最明显）。

## 9. 验收

```bash
cargo test
cargo run -- csv output/loss.jsonl output/loss.csv
```

- [ ] 新列在末尾，测试全绿
- [ ] 能说出 CSV 列顺序被改会怎样，以及为什么不会有报错
- [ ] 能说出 BOM 是干嘛的
- [ ] 能解释 `diagnostics` 为什么用 `;` 连接而不是 `,`

## 10. 对应真实项目

真实项目导的是 **xlsx**（`src/report/xlsx.rs`），不是 CSV ——
xlsx 没有转义和 BOM 的问题，但多了一个依赖和一堆格式代码。

**这是个典型的取舍**：CSV 零依赖、谁都能打开，代价是上面这一堆坑；
xlsx 代价在依赖和代码量，换来的是不用管编码。

`src/report/store.rs` 是 rows.jsonl 的落盘，对照着看：
同一份数据，两个出口，各自的约束完全不同。
