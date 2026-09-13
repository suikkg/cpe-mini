# cpe-mini —— cpe-test 的教学缩微版

一条能跑通的完整业务链路：**读配置 → 展开单元 → 执行 → 判定 → 出报告**，
外加三个消费报告的出口（**对比两轮 / 诊断通道 / 导出 CSV**）
和三块真实工程才有的东西（**取消 / 聚合特例 / 黄金文件**）。

用模拟数据（样本写在计划文件里），不起任何进程，**每次运行结果完全一样** ——
所以改完代码能立刻看出是不是改对了。

```bash
cargo run -- demo
```

```text
模式：模拟测试（样本来自计划文件，不起任何进程）

测试项                        实测速率      门限          结果
──────────────────────────────────────────────────────────────────
TCP-A                         950 Mbps      900 Mbps      PASS
TCP-B                         850 Mbps      900 Mbps      RATE_FAIL
TCP-C                         无有效数据    900 Mbps      NOT_EVALUATED

总计 3 项：通过 1，未达标 1，无法评价 1

接下来该做什么：
  TCP-B → CPE 没跑到门限，检查信号、信道或设备本身
  TCP-C → 测试工具没跑起来，检查对端是否在线、端口是否被占
```

---

## 目录

- [这是什么](#这是什么)
- [先修](#先修)
- [怎么学](#怎么学)
- [12 课 + 6 进阶课 + 2 扩展课](#12-课--6-进阶课--2-扩展课)
- [进度表](#进度表)
- [架构](#架构)
- [代码导读顺序](#代码导读顺序)
- [命令](#命令)
- [样例计划](#样例计划)
- [代码里的注释怎么读](#代码里的注释怎么读)
- [刻意没做的事](#刻意没做的事)
- [环境](#环境)

---

## 这是什么

它是真实项目 **cpe-test** 的缩微版。

真实项目是 **[suikkg/cpe-test](https://github.com/suikkg/cpe-test)**（开源）：
v6.4.0，分支 `feat/webui-vue`，92 个 `.rs` 文件、约 **7.6 万行**。
这里是 11 个文件、**3325 行**。

> **课程里所有的文件名和行号，都对应提交 `4e7e970`。**
> 拼永久链接：`https://github.com/suikkg/cpe-test/blob/4e7e970/<路径>#L<行号>`
> 例如 [`src/verdict.rs:21`](https://github.com/suikkg/cpe-test/blob/4e7e9701d711f64d86177eb213309c21d72a15d9/src/verdict.rs#L21)。
>
> 本地 clone 一份对照着读最方便。**下文写 `src/verdict.rs` 时，
> 指的都是 cpe-test 仓库里的路径，不是本项目的。**

缩了 40 倍，但**骨架一模一样**：

> **类型名、字段名、枚举字符串、JSONL 格式都与真实项目一致。**

`Verdict`、`ExecutionStatus`、`VerdictResult`、`aggregate_verdict`、`ReasonCode`、
`Unit`、`Leg`、`Row` —— 全是真实项目里的名字，一个字母都没改。
报告里的判定串是 `"RATE_FAIL"` 而不是 `"RateFail"`，也和真实项目一致。

**所以这里读懂的东西，到真实代码里不用二次翻译。**

每一课末尾都有「对应真实项目」一节，带文件名和行号。
完整映射表见 [`lessons/00_对应关系速查.md`](lessons/00_对应关系速查.md)。

## 先修

先做完隔壁的 `rust-starter`（零基础入门，12 课）。那边学语法，这边学结构。

已经能看懂下面这些就可以直接开始：

```rust
struct / enum / impl        Option / Result / match
&T / &mut T                 fn f(x: &[T]) -> Vec<T>
```

**不必等 rust-starter 全部做完**，学到第 07 课（struct）之后就可以并行。

## 怎么学

和 rust-starter 是两种不同的训练：

| | rust-starter | cpe-mini |
|---|---|---|
| 起点 | 空文件 | 一份现成的、能跑的代码 |
| 你做什么 | 从零写出来 | 读懂它，然后改它 |
| 练的能力 | 写代码 | **读代码、定位、小改、跑测试** |

后者才是维护真实项目的日常。

每一课四步：

1. **跑** —— 先看到这段代码的效果
2. **读** —— 课程会指明读哪个文件的哪一段
3. **改** —— 按「动手任务」改，**每次改完跑 `cargo test`**
4. **对照** —— 到真实项目里看同一处代码

**每天 30–60 分钟，一课一到两天。**

课程里有不少「把它改坏，看哪个测试挂了，读懂它在保护什么」这类任务。
那是这个项目的核心训练：**测试不是负担，是一张写下来的规则清单。**

## 12 课 + 6 进阶课 + 2 扩展课

| 课 | 读什么 | 对应真实项目 |
|---|---|---|
| 01 | 跑通全链路，`match mode` 分发 | `src/main.rs:56` |
| 02 | 核心判定规则 `rate_verdict` | `src/verdict.rs` |
| 03 | 枚举、label 往返、对外兼容面 | `src/verdict.rs:21` |
| 04 | `Option` 与「无法评价」 | `src/verdict.rs:167` |
| 05 | 所有权在这个项目里的惯用写法 | 全项目 |
| 06 | 配置校验与错误定位 | `src/master/webui/plan.rs:13` |
| 07 | 模块边界：判定为什么只能有一份 | `src/verdict.rs:282` |
| 08 | serde、`#[serde(default)]` 与错误传播 | `src/report/model.rs:184` |
| 09 | builder：展开轮次与方向 | `src/master/builder.rs:247` |
| 10 | rate_window：有效窗口与爬坡 | `src/master/rate_window.rs` |
| 11 | report：Row、JSONL、坏行容错 | `src/report/` |
| 12 | **毕业考**：把 note 从配置传到报告 | 跨模块 |

12 课做完之后有八课，**互相之间没有先后关系**，挑感兴趣的做。

进阶课（各一到两天，和前 12 课一样是读现成代码再改）。
前三课讲**报告的三个出口**：

| 课 | 读什么 | 对应真实项目 |
|---|---|---|
| 15 | `compare`：两轮对比，以及**对齐键**这个真实项目栽过的坑 | `src/report/compare.rs` |
| 16 | 诊断通道：丢包 31% 为什么还是 PASS（ADR-17） | `src/verdict.rs:171` |
| 17 | CSV 导出：什么叫「对外兼容面」 | `src/report/xlsx.rs` |

后三课讲**执行、判定、验收各自最难的一块**：

| 课 | 读什么 | 对应真实项目 |
|---|---|---|
| 18 | 取消：按下停止之后，没跑的单元**不许从报告里消失** | `src/cancel.rs` |
| 19 | 聚合的两条特例 —— 全项目最绕的一段规则 | `src/verdict.rs:232` |
| 20 | 黄金文件：把「给人看的输出」逐字钉死 | `src/report/` |

第 18 课是这个项目里唯一一处真开线程的地方（`Arc<AtomicBool>`）。
第 19 课是**要自己写**的：骨架在 `scaffold/aggregate_special.rs`，三道题。

扩展课（各三到五天，是两个小工程，要自己从头写）：

| 课 | 做什么 | 对应真实项目 |
|---|---|---|
| 13 | 把模拟执行换成真跑 `ping`，学进程启动与输出解析 | `src/ping.rs` |
| 14 | 用 `tiny_http`（真实项目同款）加最小 Web 界面 | `src/master/webui/` |

要自己补 `todo!()` 的一共三课 —— 13、14 和 19。骨架都在 **`scaffold/`**
（测试已经写好），完整答案在 `solutions/`：

| 课 | 骨架 | 答案 |
|---|---|---|
| 13 | `scaffold/ping.rs`（7 个测试） | `solutions/13_ping.md` + `ping_answer.rs` |
| 14 | `scaffold/webui.rs` + `.html`（6 个测试） | `solutions/14_webui.md` + `webui_answer.rs` |
| 19 | `scaffold/aggregate_special.rs`（3 道题 / 10 个测试） | `solutions/aggregate_special_answer.rs` |

`./check.sh` 每次都会把骨架和答案拷进临时 crate 重新编译一遍 ——
它们不在主 crate 里，不这么做就会随主代码改动悄悄烂掉。

### 关于第 12 课

**没有标准答案。** 任务是给计划加一个 `note` 字段，让它一路传到报告里。

这正是 45 天路线图里那条目标：「能增加一个简单字段并完成前后端联动」。
在真实 cpe-test 里同一个字段要多走三层（Vue → TypeScript → HTTP），
但后面的链路是一样的。**在这里做一遍，到了真实项目就知道该沿哪条路找。**

## 进度表

- [ ] 00 对应关系速查（先扫一眼，不用记）
- [ ] 01 跑通全链路
- [ ] 02 核心规则：速率判定
- [ ] 03 枚举与对外兼容面
- [ ] 04 Option 与无法评价
- [ ] 05 所有权在本项目里
- [ ] 06 配置校验与错误定位
- [ ] 07 模块边界
- [ ] 08 从 JSON 读计划
- [ ] 09 builder 展开测试单元
- [ ] 10 rate_window 有效窗口
- [ ] 11 report 与 JSONL
- [ ] 12 毕业考：加一个字段 ← 没有答案

下面八课没有先后关系，挑感兴趣的做：

- [ ] 15 对比两份报告
- [ ] 16 诊断通道：丢包不改判定
- [ ] 17 CSV 导出与兼容面
- [ ] 18 取消与收尾
- [ ] 19 聚合的两条特例 ← 要自己写
- [ ] 20 黄金文件把输出钉死
- [ ] 13 扩展：真实执行（三到五天）
- [ ] 14 扩展：HTTP 界面（三到五天）

## 架构

```text
              fixtures/plan.json
                     │
        ┌────────────▼────────────┐
        │  plan.rs                │   读 JSON + 校验
        │  Plan / Spec / validate │   非法配置在开跑前就拦下
        └────────────┬────────────┘
                     │
        ┌────────────▼────────────┐
        │  builder.rs             │   展开：轮次 × 方向
        │  Unit / Leg             │   一条规格 → 多个测试单元
        └────────────┬────────────┘
                     │
        ┌────────────▼────────────┐
        │  executor.rs            │   执行（教学版用固定样本）
        │  UnitOutcome/LegOutcome │   自己不判定
        │        ▲ cancel.rs      │   单元边界上问一句「要停吗」
        └──────┬─┴─────────┬──────┘
               │           │
   ┌───────────▼──┐   ┌────▼──────────────┐
   │ rate_window  │   │ verdict.rs        │   判定规则唯一一份
   │ 有效窗口平均  │   │ Verdict / 聚合     │   executor 和 report 都调它
   └───────────┬──┘   └────┬──────────────┘
               │           │
            ┌──▼───────────▼──┐
            │  reason.rs      │   原因码（最底层，不依赖任何业务模块）
            └──────┬──────────┘
                   │
        ┌──────────▼──────────────┐
        │  report.rs              │   Row → rows.jsonl + CSV + 渲染
        │  只渲染，不判定           │
        └──────────┬──────────────┘
                   │  rows.jsonl
        ┌──────────▼──────────────┐
        │  compare.rs             │   两轮对比：回归测试要的那张表
        │  只吃 Row，不碰前面的链路  │
        └─────────────────────────┘
```

### 依赖方向

```text
reason.rs        ← 谁都不依赖（最核心、最好测、最稳定）
verdict.rs       → reason
rate_window.rs   → reason
plan.rs          ← 谁都不依赖
builder.rs       → plan
executor.rs      → plan, builder, rate_window, verdict, reason
report.rs        → plan, builder, executor, verdict, reason
compare.rs       → report, verdict          ← 只吃 Row，不知道前面的链路存在
cancel.rs        ← 谁都不依赖（一个 Arc<AtomicBool>，executor 问它）
lib.rs / main.rs ← 只负责串联和分发
```

**越核心的模块依赖越少。** `verdict.rs` 不知道 iperf、不知道网卡、
不知道报告长什么样 —— 所以它最好测（10 个测试不起任何进程），
也最不容易被别处的改动带坏。

### 一条铁律

**判定规则只有一份，在 `verdict.rs`。**

`executor` 不判定，`report` 也不判定。这不是洁癖 ——
真实项目的 `src/verdict.rs` 开头记着这段教训：判定曾经在 executor 和 report
各实现一遍，两份实现的优先级不一致，先后产生过两个真实缺陷。
**两份实现最终一定会漂，不是「可能」。**

## 代码导读顺序

第一次读，按这个顺序，从最简单的开始：

| 顺序 | 文件 | 行数 | 为什么先读它 |
|---|---|---|---|
| 1 | `src/lib.rs` | 100 | 全链路四步都在这，一眼看完 |
| 2 | `src/reason.rs` | 193 | 最底层，不依赖任何东西 |
| 3 | `src/rate_window.rs` | 164 | 唯一一处「算法」，纯函数 |
| 4 | `src/executor.rs` | 521 | 看四步怎么串起来 |
| 5 | `src/builder.rs` | 222 | 计划怎么变成任务 |
| 6 | `src/plan.rs` | 282 | 配置和校验 |
| 7 | `src/main.rs` | 234 | 命令分发 |
| 8 | `src/verdict.rs` | 357 | 核心规则，注释最多 |
| 9 | `src/report.rs` | 629 | 最长，但逻辑最简单 |
| 10 | `src/compare.rs` | 475 | 独立的一块，只吃 `Row`（第 15 课） |
| 11 | `src/cancel.rs` | 148 | 全项目唯一一处开线程的地方（第 18 课） |

**读不懂就先跳过，往下读。** 整体形状比某一行细节重要。

## 命令

```bash
cargo run -- demo                       # 跑内置样例计划
cargo run -- plan fixtures/plan.json    # 只展开，看会跑成什么样（不执行）
cargo run -- run <计划> [输出]           # 跑一份计划并存报告
cargo run -- run <计划> [输出] --cancel-after N   # 跑完 N 个单元就叫停（第 18 课）
cargo run -- report <报告文件>           # 读回报告重新渲染
cargo run -- compare <旧> <新>           # 对比两份报告，有回归返回 1
cargo run -- csv <报告文件> [输出]        # 导出 CSV（给 Excel 用）
cargo run -- --help                     # 用法
```

```bash
./check.sh                # 一键自检：fmt + clippy + 全部测试
cargo test                # 111 个测试
cargo test --lib verdict  # 只跑判定模块的 10 个
cargo test --lib compare  # 只跑对比模块的 8 个
cargo test --test golden  # 只跑黄金文件那 6 个（第 20 课）
UPDATE_GOLDEN=1 cargo test --test golden   # 认可这次输出改动，更新黄金文件
cargo clippy              # 代码建议（当前零告警）
cargo check               # 只检查能不能编译，最快
```

### 退出码

给脚本用的：有 `RATE_FAIL` / `SETUP_ERROR` / `NOT_EVALUATED` 返回 **1**，否则 **0**。

`MEASURED`（没设门限，只记录）和 `SKIP` **不算问题** ——
所以 `plan_bidir.json` 返回 0。

拿 `pass == total` 当判据是错的：一份全是「只记录」的计划会永远返回 1，
脚本就再也分不出真失败和没设门限。`tests/pipeline.rs` 里有测试钉住这一点。

## 样例计划

| 文件 | 用来演示 | 退出码 |
|---|---|---|
| `fixtures/plan.json` | 三种典型结果：PASS / RATE_FAIL / NOT_EVALUATED | 1 |
| `fixtures/plan_bidir.json` | 双向 + 多轮 + 无门限（只记录） | 0 |
| `fixtures/plan_edge.json` | 五种边界：等于门限、窗口太短、空洞太多、偶发抖动、单边无数据 | 1 |
| `fixtures/plan_bad.json` | 语义非法配置 —— **一次报出全部 4 个问题** | 1 |
| `fixtures/plan_broken.json` | JSON 语法坏 —— 报错带行列号 | 1 |
| `fixtures/plan_v1.json`、`plan_v2.json` | 「固件 B11 → B12」，第 15 课对比用 | 1 |
| `fixtures/plan_loss.json` | 丢包 31% 却判 PASS，第 16 课用 | 1 |

后两个值得单独跑一下，看报错长什么样：

```bash
cargo run -- run fixtures/plan_bad.json
cargo run -- run fixtures/plan_broken.json
```

**报错质量是这个项目刻意教的东西之一。** 「配置有误」和
「`specs[0].transport` 只能是 tcp 或 udp，当前是 `"sctp"`」，
差别是用户要不要靠猜。

## 代码里的注释怎么读

`src/` 里的注释分两类：

- **「与真实项目的对应」** —— 这段代码在 cpe-test 的哪里，真实版多做了什么
- **「为什么」** —— 这个设计在防什么。多数是真实项目踩过的坑

读代码时优先读这些。**知道一段代码在防什么，比知道它在做什么更有用。**

举个例子，`builder.rs` 里关于轮次的注释：

> 不这么做的话，20 轮的计划在 `HashMap` 里会互相覆盖，只剩最后一轮 ——
> 而且**不会有任何异常提示**，对比报告安静地少了 19 轮。

这类「安静的错误」才是真实工程里最贵的。

## 刻意没做的事

真实项目里有、这里没有的：

- 起 iperf3 / ctsTraffic 进程，连辅测机（`src/cmd/`、`src/agent/`）
- 网卡扫描与计数器采样（`src/nic/`）
- HTTP 服务和 Vue 前端（`src/master/webui/`、`ui/`）
- Excel / PNG 报告、对比报告出 HTML（`src/report/chart.rs`、`xlsx.rs`）
- 断点续跑与超时（`src/run_status.rs`、`recv_timeout`）—— **取消**在第 18 课补上了
- 起 Ctrl+C 处理器；真实项目那边有四个取消标志，这里只留一个

这些都是**真实工程的复杂度**，不是入门要学的东西。先把主链路吃透。

扩展课 13 和 14 会补上其中最有代表性的两块：真实进程执行、HTTP 界面。
第 18 课补上取消，第 19 课补上那两条聚合特例。

## 环境

```bash
rustc --version    # 1.96.0
cargo --version    # 1.96.0
```

**edition 2021**，和真实项目一致 —— 这里读到的写法能原样套用过去。

依赖只有两个，也是真实项目在用的：

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

扩展课 14 会再加一个 `tiny_http = "0.12"`，同样是真实项目同款。
**不要换成 axum / actix**：那两个要 async 运行时，学的东西迁不回 cpe-test。

## 目录

```
cpe-mini/
├── lessons/            12 课 + 6 进阶课 + 2 扩展课 + 速查 ← 主线
├── src/
│   ├── main.rs         real_main + match mode（对齐真实项目的入口结构）
│   ├── lib.rs          串联四步
│   ├── plan.rs         Plan / Spec / validate
│   ├── builder.rs      Unit / Leg，展开轮次与方向
│   ├── executor.rs     执行，产出测量值（自己不判定）
│   ├── rate_window.rs  有效窗口 RX 平均
│   ├── verdict.rs      Verdict / VerdictResult / 聚合（判定唯一一份）
│   ├── reason.rs       ReasonCode
│   ├── report.rs       Row / JSONL / CSV / 渲染（只渲染，不判定）
│   ├── compare.rs      两轮对比（只吃 Row）
│   └── cancel.rs       取消信号 Arc<AtomicBool>（第 18 课）
├── fixtures/
│   ├── *.json          8 份样例计划
│   └── golden/         6 份黄金文件（第 20 课）
├── tests/
│   ├── pipeline.rs     18 个全链路集成测试
│   └── golden.rs       6 个黄金文件测试
├── scaffold/           第 13 / 14 / 19 课的骨架（留了 TODO）
├── solutions/
│   ├── README.md       动手任务的答案要点（第 12 课没有）
│   ├── 13_ping.md      扩展课 A 的完整答案
│   ├── 14_webui.md     扩展课 B 的完整答案
│   └── aggregate_special_answer.rs   第 19 课的完整答案
├── check.sh            一键自检
└── Cargo.toml
```

## 做完之后

回到 `cpe-test`，挑一个真实的简单字段
（比如 `Row` 里的 `kind_label`），**只读不改**，从 `Row` 一路往回找到它的来源。

能找到，就说明这套方法你已经会用了。
