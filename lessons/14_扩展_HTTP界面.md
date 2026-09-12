# 扩展课 B：加一个最小的 Web 界面

**先决条件**：12 课 + 扩展课 A。安排三到五天。

**目标**：把已有的 Rust 业务接上 HTTP，理解 cpe-test 的 `webui` 那一层在干什么。

## 1. 先看清楚要做什么

现在 cpe-mini 只有命令行。加完之后：

```bash
cargo run -- ui
# 打开 http://127.0.0.1:8080
```

页面上能：选一份计划 → 点「运行」→ 看到结果表格。

**业务逻辑一行都不用改。** `run_plan()` 已经是 `Result<Vec<Row>, String>`，
HTTP 层只是换一种方式调用它 —— 这正是第 07 课分层的回报。

## 2. 依赖

用 `tiny_http`，**真实项目同款**（`Cargo.toml` 里是 `tiny_http = "0.12"`）。
不要用 axum / actix：那两个要 async 运行时，学的东西迁不回 cpe-test。

```toml
tiny_http = "0.12"
```

## 3. 骨架

```rust
use tiny_http::{Response, Server};

pub fn run(port: u16) -> Result<(), String> {
    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr).map_err(|e| format!("监听 {addr} 失败：{e}"))?;
    println!("打开 http://{addr}");

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let response = match url.as_str() {
            "/" => Response::from_string(PAGE).with_header(html_header()),
            "/api/run" => { ... }
            _ => Response::from_string("not found").with_status_code(404),
        };
        let _ = request.respond(response);
    }
    Ok(())
}
```

注意 `let _ = request.respond(...)` —— 客户端可能已经断开，
**响应失败不该让整个服务挂掉**。真实项目里每一处 `respond` 都是这么写的。

## 4. 三个真实项目踩过的坑

**用 `recv_timeout` 而不是 `incoming_requests()`**

真实项目 `src/master/webui/http.rs:95` 的注释：

> 用 `recv_timeout` 而不是 `recv()`：后者没有出口，取消标志永远查不到。

`incoming_requests()` 会一直阻塞，Ctrl-C 之外没有别的办法停下来。
要支持优雅退出，就得用带超时的循环。

**页面放哪**

真实项目把 HTML 直接 `include_str!` 进二进制（`src/master/webui.html`）。
好处：发布就一个 exe，不用管资源文件路径。

```rust
const PAGE: &str = include_str!("webui.html");
```

**JSON 响应要带 Content-Type**

浏览器不看 `Content-Type` 会把 JSON 当文本显示。真实项目有专门的
`json_response()` 帮助函数，每个 API 出口都走它 —— 不要每处手写 header。

## 5. 任务

**骨架已经给好了**，6 个测试全写好了（而且一个服务都不用起），你只补函数体：

```bash
cp scaffold/webui.rs  src/webui.rs
cp scaffold/webui.html src/webui.html
# src/lib.rs 里加 pub mod webui;
# Cargo.toml 里加 tiny_http = "0.12"
cargo test --lib webui       # 现在全红
```

`webui.html` 是给好的，不用你写 —— 这一课练的是 Rust 那一层。

答案：`solutions/webui_answer.rs` + `solutions/14_webui.md`。

分四步，每步都要能跑：

1. **只有一个页面**：`cargo run -- ui` 起服务，`/` 返回一句 hello。确认能打开。
2. **只读接口**：`/api/plan` 返回 `fixtures/plan.json` 展开后的单元清单（JSON）。
   页面用 `fetch` 拉下来显示成列表。
3. **执行接口**：`/api/run` 跑 `run_plan()`，返回 `Vec<Row>` 的 JSON。
   页面加一个按钮，点了显示结果表格。
4. **错误路径**：故意指向 `fixtures/plan_bad.json`，确认校验错误能显示在页面上，
   而不是页面一片空白。

**第 4 步最重要。** 大多数人写到第 3 步就停了，但错误路径才是用户真正会遇到的。

## 6. 验收

```bash
cargo run -- ui
```

- [ ] 页面能打开，四步功能都在
- [ ] 计划校验失败时页面显示得出具体错误（哪条 spec、哪个字段）
- [ ] 服务跑着的时候 `cargo test` 仍然全绿（业务逻辑没被 HTTP 层污染）
- [ ] `src/webui.rs` 里**没有任何判定逻辑** —— 它只负责收请求、调业务、转 JSON

最后一条是这一课真正要学的。翻一遍你写的 `webui.rs`，
如果里面出现了 `if rx_avg >= target` 这种东西，就是分层破了。

## 7. 做完之后读真实代码

`src/master/webui/`：

- `http.rs` —— 监听、路由、鉴权、超时。看它怎么处理并发和取消
- `plan.rs:13` `validated_config_from_request` —— 前端来的配置怎么校验（第 06 课见过）
- `api.rs` / `runs.rs` —— 业务接口
- `model.rs` —— DTO，前后端之间的数据契约

前端在 `ui/src/`（Vue + TypeScript）。**字段从 Vue 一路走到 Rust 的完整链路**：

```text
ui/src/views/*.vue → state/*.ts → domain/*.ts → api/client.ts
    → HTTP → master/webui/http.rs → api.rs → model.rs
    → builder → executor → verdict → report
```

这条链路就是 45 天路线图里那张图。做完这两个扩展课，你已经把它走过一遍了。
