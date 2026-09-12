# scaffold/ —— 扩展课 13 / 14 的骨架

这两课不像前面的课「读现成代码再改」，而是**自己写一个新模块**。
骨架给了结构和测试，你补函数体。

| 文件 | 对应课 | 装到哪 |
|---|---|---|
| `ping.rs` | 13（真实执行） | `src/ping.rs` |
| `webui.rs` + `webui.html` | 14（HTTP 界面） | `src/webui.rs` + `src/webui.html` |

## 为什么不直接放在 src/

放进 `src/` 就要么是空的（`cargo test` 一片红），要么是写好的（那就没得练了）。
放在这里，**你决定什么时候开始**。

## 第 13 课

```bash
cp scaffold/ping.rs src/ping.rs
```

在 `src/lib.rs` 的模块列表里加一行（按字母序）：

```rust
pub mod ping;
```

然后：

```bash
cargo test --lib ping     # 7 个测试，现在全红
```

一个一个把 `todo!()` 换掉。**测试一个包都不发**，跑起来是毫秒级的。

最后在 `src/main.rs` 的 `match mode` 里加 `"ping"` 分支
（完整代码在 `solutions/13_ping.md`）。

## 第 14 课

```bash
cp scaffold/webui.rs  src/webui.rs
cp scaffold/webui.html src/webui.html
```

`src/lib.rs` 加 `pub mod webui;`，`Cargo.toml` 的 `[dependencies]` 加：

```toml
tiny_http = "0.12"
```

**不要换成 axum / actix**：那两个要 async 运行时，学的东西迁不回 cpe-test。
`tiny_http` 是真实项目同款。

```bash
cargo test --lib webui    # 6 个测试，现在全红
cargo run -- ui           # 做完之后，打开 http://127.0.0.1:8080
```

`webui.html` 是**给好的**，不用你写 —— 这一课练的是 Rust 那一层。

## 骨架里的两个 allow

```rust
#![allow(dead_code, unused_variables, unused_imports)]
```

函数体还是 `todo!()` 的时候，参数用不上、导入用不上，编译器会告警。
**写完实现之后这三条就都不需要了** —— 做完记得删掉，看看是不是真的没有告警。

## 答案

- `solutions/ping_answer.rs`、`solutions/13_ping.md`
- `solutions/webui_answer.rs`、`solutions/webui_answer.html`、`solutions/14_webui.md`

答案都是**验证过能编译、测试全绿**的。但先自己写 15 分钟。
