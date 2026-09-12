# 第 09 课：builder —— 把计划展开成测试单元

**目标**：理解"用户填的东西"和"真正要跑的任务"之间那一层。

## 1. 先跑

```bash
cargo run -- plan fixtures/plan_bidir.json
```

2 条规格 × 2 轮 = 4 个单元，其中双向单元各有 2 条腿。

## 2. 计划 vs 单元

| | 是什么 | 例子 |
|---|---|---|
| `Spec` | 用户填的 | 「TCP 双向，门限 900，跑 2 轮」 |
| `Unit` | 真正要执行的一条任务 | 「第 1 轮 TCP 双向」 |
| `Leg` | 单元里的一个方向 | 「AB 腿，门限 900」 |

一条规格能展开成好几个单元。真实项目里还要乘上网卡组合、IP 对、套件参数，
所以 `builder.rs` 有 5205 行。

## 3. 两个真实的设计细节

**单向腿的 `tag` 是空串**

```rust
"ab" => vec![Leg { tag: String::new(), ... }],
"bidir" => vec![Leg { tag: "AB".into(), ... }, Leg { tag: "BA".into(), ... }],
```

这不是漏填。空 tag 在执行侧有语义：「这个单元只有一个方向，不用区分」。
`report.rs::rows_from` 会检查它：

```rust
let task_id = if leg.tag.is_empty() { out.unit_id.clone() } else { format!("{}#{}", out.unit_id, leg.tag) };
```

**轮次拌进 id**

```rust
let id = format!("{}-{}-r{}", plan.plan_id, index, round);
```

不这么做的话，20 轮的计划在 `HashMap` 里会互相覆盖，只剩最后一轮 ——
而且**不会有任何异常提示**，对比报告安静地少了 19 轮。
`tests::轮次拌进id所以不会互相覆盖` 钉住了这件事。

## 4. 展开顺序

```rust
for round in 1..=plan.rounds {
    for (i, spec) in plan.specs.iter().enumerate() {
```

**先轮次，后规格**。反过来的话，同一条测试会连着跑 N 遍 ——
那测的是"这一分钟稳不稳"，不是"跨时间稳不稳"。稳定性测试的意义就没了。

这一行循环嵌套的顺序，就是一条业务规则。

## 5. `round` 为什么是字段而不是从标题里搜

标题是 `"TCP 双向 · 第 2 轮"`。看起来可以用字符串搜出轮次，但真实项目的注释警告过：

> 那个后缀是展示串，改一次文案就全体失效，而失效的表现是「对比报告少了 19 轮」
> 这种没人会去核对的安静错误。

**展示串不能当数据用。** 要用就单独开一个类型化字段。

## 6. 动手任务

1. 加一种方向 `"loop"`（自环测试），展开成一条腿，tag 是空串，
   样本取 `samples_ab`
2. 在 `plan.rs::validate` 里放行这个新方向
3. 造一个 fixture 验证
4. 写测试钉住"loop 方向展开成一条腿"

然后：

5. 把展开顺序改成"先规格后轮次"，跑 `cargo test`，看哪个测试挂了
6. 改回来

## 7. 验收

```bash
cargo test --lib builder
cargo run -- plan fixtures/你造的文件.json
```

- [ ] 新方向能展开、能跑、有测试
- [ ] 能解释单向腿的 tag 为什么是空串
- [ ] 能解释轮次为什么必须拌进 id

## 8. 对应真实项目

`src/master/builder.rs:240`（`Leg`）、`:247`（`Unit`）、`:742`（`round_scoped_id`）。
真实 `Unit` 多出来的字段（`link_group`、`target_lines`、`bidir_total_target_mbps`、
`direction`）注释里都标了「只用于展示，判定不读它」或者「不进 resume identity」——
**读代码时先看这类注释，它们告诉你哪些字段是能改的**。
