# 第 08 课：serde、兼容面与错误传播

**目标**：读懂 `Plan` 是怎么从文件变成结构体的，以及出错时错误怎么往上走。

## 1. 先跑

```bash
cat fixtures/plan.json
cargo run -- plan fixtures/plan.json
```

## 2. 读代码：`src/plan.rs::load`

```rust
pub fn load(path: &std::path::Path) -> Result<Plan, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;

    let plan: Plan = serde_json::from_str(&text)
        .map_err(|e| format!("{} 不是合法的计划文件：{e}", path.display()))?;

    validate(&plan).map_err(|errs| { ... })?;

    Ok(plan)
}
```

三个 `?`，三种错误，每一种都被翻译成给人看的话。

## 3. `map_err` + `?` 这个组合

```rust
something().map_err(|e| format!("...{e}"))?
```

- `map_err` 把底层错误换成我们自己的话（**保留原始错误 `{e}`**）
- `?` 失败就立刻返回

**不要丢掉原始错误。** serde 的报错自带 `at line 7 column 7`，
自己包装时把它吞掉，用户就定位不到了。对比：

```text
✅ fixtures/plan_broken.json 不是合法的计划文件：expected `,` or `}` at line 7 column 7
❌ 配置文件读取失败
```

## 4. 三层错误各自的用处

| 层 | 错误来源 | 给用户的信息 |
|---|---|---|
| 文件 | 不存在、没权限 | 哪个路径 |
| 语法 | JSON 格式坏 | 第几行第几列 |
| 语义 | 字段值不合法 | 第几条 spec 的哪个字段 |

三层缺一层，排查就会卡住。

## 5. 为什么返回 `Result<Plan, String>` 而不是 `Box<dyn Error>`

这个函数的错误**最终都要显示给人看**，不会有调用方去区分错误种类做不同处理。
这种情况下 `String` 最直接。

如果将来需要区分（比如"文件不存在"要走创建默认配置的流程），
就该定义自己的错误 enum。**别提前做**。

## 6. 兼容要兼两个方向

`#[serde(default)]` 解决的只是一半：

```text
老文件 → 新程序     老文件缺新字段 → #[serde(default)] 补上默认值
新文件 → 老程序     新文件多了老程序不认识的字段 → ？
```

第二个方向 **serde 默认就替你处理了**：反序列化时遇到结构体里没有的字段，
**直接忽略**，不报错。

所以：

| 方向 | 靠什么 | 要不要自己写 |
|---|---|---|
| 老文件 → 新程序 | `#[serde(default)]` | **要**，不写就读不出来 |
| 新文件 → 老程序 | serde 默认忽略未知字段 | 不用，白送 |

两个方向都通，同一批文件才能在新旧版本之间来回读。
真实项目的现场经常是「测试机上跑的是 v6.3，分析用的是 v6.4」——
少一个方向就得现场升级。

> 想关掉第二个方向（多一个字段就报错）可以加
> `#[serde(deny_unknown_fields)]`。**这个项目刻意不加** ——
> 严格一点换来的是历史文件全废，不划算。

## 7. 动手任务

1. 在 `Spec` 里加一个字段 `duration_secs: u32`
2. **不要改 fixtures** —— 直接 `cargo test`，确认因为 `#[serde(default)]` 旧文件仍能读
3. 在 `plan fixtures/plan.json` 的预览输出里显示这个字段
4. 只在 `fixtures/plan_bidir.json` 里填上这个字段，确认两份文件都能读

然后验证另一个方向（30 秒）：

5. 复制 `fixtures/plan.json`，随手往里加两个不存在的字段 ——
   一个加在顶层，一个加进某条 spec 里：

   ```json
   { "plan_id": "demo", "未来版本才有的字段": 123, "specs": [ ... ] }
   ```

6. `cargo run -- plan 你的文件.json` —— **照样跑通**。
   这就是 serde 默认忽略未知字段：新版写的文件，老版程序读得下去。

最后：

7. 把 `Plan` 上的 `#[serde(default)]` 删掉，跑 `cargo test`，看多少测试挂了
8. 加回来

## 8. 验收

- [ ] 新字段加完，**没改任何 fixture** 的情况下全部测试通过
- [ ] 能解释 `#[serde(default)]` 保护的是什么
- [ ] 能说出三层错误各自该给什么信息
- [ ] 能说出兼容的两个方向各靠什么保证

## 9. 对应真实项目

`src/report/model.rs:184` 的 `Row` 上就是 `#[serde(default)]`，
注释里写着"版本号写在 meta.json 里，重放器容忍未知字段"。
真实项目的 `Row` 有 40 多个字段，这个保护一旦没有，
每加一个字段所有历史报告就全废了。
