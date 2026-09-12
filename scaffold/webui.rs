//! 扩展课 B（第 14 课）的骨架：最小 Web 界面。
//!
//! ## 怎么用
//!
//! ```bash
//! cp scaffold/webui.rs  src/webui.rs
//! cp scaffold/webui.html src/webui.html
//! # src/lib.rs 里加：pub mod webui;
//! # Cargo.toml 的 [dependencies] 里加：tiny_http = "0.12"
//! cargo test --lib webui       # 现在全红
//! ```
//!
//! 测试已经写好了，**六个测试一个服务都不用起** —— 因为 `route` 是纯函数。
//! 这正是把它从 `run` 里拆出来的理由。
//!
//! 答案：`solutions/webui_answer.rs` 和 `solutions/webui_answer.html`
//!
//! ## 建议的顺序（每步都要能跑）
//!
//! 1. 只做 `/` 返回页面，`cargo run -- ui` 能打开
//! 2. 做 `/api/plan`，页面上「展开看看」能出东西
//! 3. 做 `/api/run`，「运行」能出表格
//! 4. **故意选 `plan_bad.json`，确认错误显示在页面上，而不是一片空白**
//!
//! 第 4 步最重要。大多数人写到第 3 步就停了，但错误路径才是用户真正会遇到的。
//!
//! ## 与真实项目的对应
//!
//! 对应 `cpe-test` 的 `src/master/webui/`（`http.rs` 监听路由、`api.rs` 业务接口、
//! `model.rs` 前后端数据契约）。真实版还有鉴权、并发、取消、Vue 前端。
//!
//! ## 这一层的唯一职责
//!
//! **收请求 → 调业务 → 转 JSON。** 这个文件里不该出现任何判定逻辑。
//! 翻一遍，如果看到 `if rx_avg >= target` 这种东西，就是分层破了。
//!
//! 业务逻辑一行都没改：`run_plan()` 本来就是 `Result<Vec<Row>, String>`，
//! HTTP 层只是换一种方式调用它。**这是第 07 课分层的回报。**

#![allow(dead_code, unused_variables, unused_imports)]

use crate::{preview_plan, run_plan};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tiny_http::{Header, Response, Server};

/// 页面直接编进二进制。
///
/// 好处：发布就一个 exe，不用管资源文件路径。真实项目也是这么做的
/// （`src/master/webui.html`）。
const PAGE: &str = include_str!("webui.html");

/// 只允许读这个目录下的计划。
///
/// **不要把 URL 参数直接当路径用。** `?plan=../../etc/passwd` 这种东西
/// 是所有 Web 服务的第一课。这里是本机教学工具，但习惯要从第一天养。
const PLAN_DIR: &str = "fixtures";

pub fn run(port: u16) -> Result<(), String> {
    // 【1】起服务。
    //
    // - `Server::http(&addr)`，失败要给清楚的错（端口被占是最常见的）
    // - 循环里用 **`server.recv_timeout(Duration::from_millis(500))`**
    //   而不是 `recv()`：后者没有出口，取消标志永远查不到
    //   （真实项目 `src/master/webui/http.rs:95` 的原话）
    // - 每个请求：取 `request.url()` → `route(&url)` → `Response::from_string`
    //   加上 `header(content_type)` → `request.respond(response)`
    // - **`respond` 失败不许让服务挂掉**：客户端可能已经把页面关了。
    //   用 `if let Err(e) = ... { eprintln!(...) }`，不要 `unwrap()`
    todo!()
}

/// 路由。返回 (响应体, Content-Type)。
///
/// 抽成纯函数是有意的：**它能单测**，不用起服务、不用发请求。
fn route(url: &str) -> (String, &'static str) {
    // 【2】路由。返回 (响应体, Content-Type)。
    //
    // - `/`          → `PAGE`，`text/html; charset=utf-8`
    // - `/api/plan`  → `json_of(plan_text(query))`
    // - `/api/run`   → `json_of(run_rows(query))`
    // - 其它         → `json_of(Err(...))`，**报错要带上路径**
    //
    // 先用 `split_query(url)` 把 `?` 后面切掉。
    todo!()
}

/// 统一的 JSON 出口。
///
/// 每个接口都走它，而不是各写各的 —— 真实项目有同样一个 `json_response()`。
/// 好处：成功和失败的形状是统一的，前端只要写一次判断。
fn json_of(result: Result<serde_json::Value, String>) -> String {
    // 【3】统一的 JSON 出口。
    //
    // 成功：`{"ok": true, "data": ...}`
    // 失败：`{"ok": false, "error": "..."}`
    //
    // 用 `serde_json::json!` 宏。**每个接口都走它**，不要各写各的：
    // 形状统一了，前端那个 `if (!r.ok)` 才只用写一次。
    todo!()
}

fn plan_text(query: &str) -> Result<serde_json::Value, String> {
    // 【4】调 `preview_plan`，包成 `{"text": "..."}`。
    // 注意这里用 `?` 把错误原样往上抛——**不要在这一层加工错误文案**，
    // plan.rs 已经把「哪条 spec、哪个字段」说清楚了。
    todo!()
}

fn run_rows(query: &str) -> Result<serde_json::Value, String> {
    // 【5】调 `run_plan`，包成 `{"rows": [...]}`。
    // `Row` 已经 derive 了 Serialize，直接 `json!({ "rows": rows })` 就行。
    todo!()
}

/// 从 `plan=xxx.json` 取文件名，并挡住路径穿越。
fn plan_path(query: &str) -> Result<PathBuf, String> {
    // 【6】从 `plan=xxx.json` 取文件名，拼成 `fixtures/xxx.json`。
    //
    // **挡住路径穿越**：文件名含 `/`、`\\` 或 `..` 一律拒绝。
    // `?plan=../../etc/passwd` 是所有 Web 服务的第一课。
    // 文件不存在也要给清楚的错。
    todo!()
}

fn split_query(url: &str) -> (&str, &str) {
    // 【7】`/api/run?plan=a.json` → `("/api/run", "plan=a.json")`
    // 提示：`url.split_once('?')`
    todo!()
}

fn param(query: &str, key: &str) -> Option<String> {
    // 【8】从 `a=1&b=2` 里取出 key 对应的值，顺便 percent 解码。
    todo!()
}

/// 最小的 percent 解码：够用就行，别为这个引依赖。
fn percent_decode(s: &str) -> String {
    // 【9】最小的 percent 解码：`+` 当空格，`%XX` 还原成字节。
    // 够用就行，别为这个引依赖。
    // 提示：`u8::from_str_radix(hex, 16)`，最后 `String::from_utf8_lossy`。
    todo!()
}

fn header(content_type: &str) -> Header {
    // 【10】造 Content-Type 头。
    // 浏览器不看 Content-Type 会把 JSON 当纯文本显示。
    // 提示：`Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())`
    todo!()
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    // 全部不起服务、不发请求 —— route 是纯函数，这是把它抽出来的理由。

    #[test]
    fn 根路径返回页面() {
        let (body, ct) = route("/");
        assert!(body.contains("<html") || body.contains("<!doctype"));
        assert!(ct.starts_with("text/html"));
    }

    #[test]
    fn 不存在的接口返回JSON错误而不是空白() {
        let (body, ct) = route("/api/nope");
        assert!(ct.starts_with("application/json"));
        assert!(body.contains("\"ok\":false"));
        assert!(body.contains("/api/nope"), "报错要说清楚是哪个接口");
    }

    #[test]
    fn 跑一份计划返回行() {
        let (body, _) = route("/api/run?plan=plan.json");
        assert!(body.contains("\"ok\":true"), "实际：{body}");
        assert!(body.contains("RATE_FAIL"), "样例计划里有一条不达标");
    }

    #[test]
    fn 校验失败的计划把具体错误送到前端() {
        let (body, _) = route("/api/run?plan=plan_bad.json");
        assert!(body.contains("\"ok\":false"));
        // 这是这一课第 4 步的重点：错误路径不能是一片空白
        assert!(
            body.contains("specs["),
            "要说清楚哪条 spec、哪个字段：{body}"
        );
    }

    #[test]
    fn 挡住路径穿越() {
        for bad in [
            "/api/run?plan=../Cargo.toml",
            "/api/run?plan=/etc/passwd",
            "/api/run?plan=..%2FCargo.toml",
        ] {
            let (body, _) = route(bad);
            assert!(body.contains("\"ok\":false"), "{bad} 应该被拒绝：{body}");
        }
    }

    #[test]
    fn 缺参数有明确报错() {
        let (body, _) = route("/api/run");
        assert!(body.contains("缺少 plan 参数"));
    }
}
