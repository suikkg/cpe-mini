# 扩展课 B（第 14 课）答案

完整代码：**`solutions/webui_answer.rs`** + **`solutions/webui_answer.html`**
（验证过：`cargo test --lib webui` 6 个全绿，clippy 零告警，服务真的能起来）

```bash
cp solutions/webui_answer.rs   src/webui.rs
cp solutions/webui_answer.html src/webui.html
# src/lib.rs 里加 pub mod webui;
# Cargo.toml 里加 tiny_http = "0.12"
cargo test --lib webui
cargo run -- ui
```

---

## 这一课真正要学的一件事

**业务逻辑一行都没改。**

`run_plan()` 本来就是 `Result<Vec<Row>, String>`，HTTP 层只是换一种方式调用它：

```rust
fn run_rows(query: &str) -> Result<serde_json::Value, String> {
    let path = plan_path(query)?;
    let rows = run_plan(&path)?;
    Ok(serde_json::json!({ "rows": rows }))
}
```

三行。**这是第 07 课分层的回报** —— 判定、执行、报告都不知道 HTTP 存在，
所以加一层 HTTP 不需要碰它们中的任何一个。

验收标准：翻一遍 `webui.rs`，如果出现了 `if rx_avg >= target` 这种东西，
就是分层破了。

---

## 五个要点

### 1. `route` 是纯函数

```rust
fn route(url: &str) -> (String, &'static str)
```

把路由从 `run` 里拆出来，6 个测试就**一个服务都不用起、一个请求都不用发**。
起服务的测试要挑端口、要等启动、会在 CI 上偶发失败 —— 能避开就避开。

`run` 里剩下的只有「收、调 route、发」，那部分不值得测。

### 2. `recv_timeout` 而不是 `recv()`

```rust
let request = match server.recv_timeout(Duration::from_millis(500)) {
    Ok(Some(req)) => req,
    Ok(None) => continue,    // 超时，没请求。将来在这里查取消标志
    Err(e) => { eprintln!("接收请求出错：{e}"); continue }
};
```

真实项目 `src/master/webui/http.rs:95` 的原话：

> 用 `recv_timeout` 而不是 `recv()`：后者没有出口，取消标志永远查不到。

现在还没有取消标志，但**结构先摆对**：将来要加优雅退出，是在那个 `Ok(None)`
分支里加一行，而不是推翻重写。

### 3. `respond` 失败不许让服务挂掉

```rust
if let Err(e) = request.respond(response) {
    eprintln!("响应失败（客户端多半已断开）：{e}");
}
```

客户端可能已经把页面关了。这里 `unwrap()` 的话，**用户关个标签页就能把你的服务
搞崩**。真实项目里每一处 `respond` 都是这么写的。

### 4. 统一的 JSON 出口

```rust
fn json_of(result: Result<serde_json::Value, String>) -> String {
    let value = match result {
        Ok(data) => serde_json::json!({ "ok": true, "data": data }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    value.to_string()
}
```

每个接口都走它。成功和失败的形状统一了，前端那个判断才只用写一次：

```js
if (!r.ok) return showError(r.error);
```

注意**业务失败也是 HTTP 200**。HTTP 状态码说的是「请求本身」有没有问题；
「计划校验不通过」是一个正常的业务结果，不是请求出错。

### 5. 别把 URL 参数直接当路径

```rust
if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
    return Err(format!("非法的计划文件名：{name:?}"));
}
```

`?plan=../../etc/passwd` 是所有 Web 服务的第一课。这里是本机教学工具，
但习惯要从第一天养。测试里钉了三种写法，包括 URL 编码过的 `..%2F`。

---

## 第 4 步：错误路径

这一课最重要的一步，也是最多人跳过的一步。

```bash
cargo run -- ui
# 页面上选 plan_bad.json → 点「运行」
```

页面上应该看到：

```text
fixtures/plan_bad.json 校验不通过（4 处）：
  - plan_id 不能为空
  - specs[0].transport 只能是 tcp 或 udp，当前是 "sctp"
  - specs[0].direction 只能是 ab / ba / bidir，当前是 "up"
  - specs[0].target_mbps 不能是负数，当前是 -900
```

**一个字都不是 webui.rs 写的。** 第 06 课在 `plan.rs` 里做的那些报错，
原样传到了页面上 —— 因为中间每一层都用 `?` 把错误往上抛，没人加工它。

「加工错误文案」是一种很常见的坏习惯：每过一层包一句「运行失败：」，
最后用户看到的是「运行失败：执行出错：校验不通过：plan_id 不能为空」。

---

## `main.rs` 的 `ui` 分支

```rust
use cpe_mini::{compare, default_output, demo_plan_path, preview_plan, report, run_plan, webui};
```

```rust
"ui" => {
    let port: u16 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(8080);
    match webui::run(port) {
        Ok(()) => 0,
        Err(e) => fail(&e),
    }
}
```

---

## 自己验一遍

```bash
cargo run -- ui 8137 &
curl -s "http://127.0.0.1:8137/api/run?plan=plan.json" | head -c 200
curl -s "http://127.0.0.1:8137/api/run?plan=plan_bad.json"
curl -s "http://127.0.0.1:8137/api/run?plan=../Cargo.toml"
```

第三条应该返回 `{"error":"非法的计划文件名：\"../Cargo.toml\"","ok":false}`。

---

## 做完之后读真实代码

`src/master/webui/`：

- `http.rs` —— 监听、路由、鉴权、超时、并发。看它比你多做了多少
- `plan.rs:13` `validated_config_from_request` —— 前端来的配置怎么校验（第 06 课见过）
- `api.rs` / `runs.rs` —— 业务接口
- `model.rs` —— DTO。**看它为什么需要一层 DTO 而你不需要**：
  前后端契约一复杂，直接把内部结构序列化出去就变成了「内部结构成了对外兼容面」，
  以后连改个字段名都会打破前端

前端在 `ui/src/`（Vue + TypeScript）。字段从 Vue 一路走到 Rust 的完整链路：

```text
ui/src/views/*.vue → state/*.ts → domain/*.ts → api/client.ts
    → HTTP → master/webui/http.rs → api.rs → model.rs
    → builder → executor → verdict → report
```

**这条链路就是 45 天路线图里那张图。** 做完这两个扩展课，你已经把它走过一遍了。
