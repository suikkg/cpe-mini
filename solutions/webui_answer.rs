//! 扩展课 B（第 14 课）的完整答案：最小 Web 界面。
//!
//! 拷成 `src/webui.rs`，把 `solutions/webui_answer.html` 拷成 `src/webui.html`，
//! 在 `src/lib.rs` 里加 `pub mod webui;`，`Cargo.toml` 里加 `tiny_http = "0.12"`，
//! 再在 `main.rs` 的 `match mode` 里加一个 `"ui"` 分支。
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
    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr).map_err(|e| format!("监听 {addr} 失败：{e}"))?;
    println!("打开 http://{addr}   （Ctrl-C 停止）");

    loop {
        // 用 recv_timeout 而不是 recv()。
        //
        // 真实项目 `src/master/webui/http.rs:95` 的注释：
        // 「用 recv_timeout 而不是 recv()：后者没有出口，取消标志永远查不到。」
        //
        // 这里还没有取消标志，但结构先摆对——将来要加优雅退出，
        // 就是在这个 None 分支里查一下标志位，而不是推翻重写。
        let request = match server.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(req)) => req,
            Ok(None) => continue, // 超时，没请求。将来在这里查取消标志
            Err(e) => {
                eprintln!("接收请求出错：{e}");
                continue;
            }
        };

        let url = request.url().to_string();
        let (body, content_type) = route(&url);

        let response = Response::from_string(body).with_header(header(content_type));

        // 客户端可能已经把页面关了。**响应失败不该让整个服务挂掉。**
        // 真实项目里每一处 respond 都是这么写的。
        if let Err(e) = request.respond(response) {
            eprintln!("响应失败（客户端多半已断开）：{e}");
        }
    }
}

/// 路由。返回 (响应体, Content-Type)。
///
/// 抽成纯函数是有意的：**它能单测**，不用起服务、不用发请求。
fn route(url: &str) -> (String, &'static str) {
    let (path, query) = split_query(url);

    match path {
        "/" => (PAGE.to_string(), "text/html; charset=utf-8"),

        "/api/plan" => (json_of(plan_text(query)), "application/json; charset=utf-8"),

        "/api/run" => (json_of(run_rows(query)), "application/json; charset=utf-8"),

        _ => (
            json_of(Err(format!("没有这个接口：{path}"))),
            "application/json; charset=utf-8",
        ),
    }
}

/// 统一的 JSON 出口。
///
/// 每个接口都走它，而不是各写各的 —— 真实项目有同样一个 `json_response()`。
/// 好处：成功和失败的形状是统一的，前端只要写一次判断。
fn json_of(result: Result<serde_json::Value, String>) -> String {
    let value = match result {
        Ok(data) => serde_json::json!({ "ok": true, "data": data }),
        // 失败也是 200 + ok:false。真实项目对业务错误也是这么做的：
        // HTTP 状态码说的是「请求本身」有没有问题，
        // 「计划校验不通过」是业务结果，不是请求出错。
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    };
    value.to_string()
}

fn plan_text(query: &str) -> Result<serde_json::Value, String> {
    let path = plan_path(query)?;
    let text = preview_plan(&path)?;
    Ok(serde_json::json!({ "text": text }))
}

fn run_rows(query: &str) -> Result<serde_json::Value, String> {
    let path = plan_path(query)?;
    let rows = run_plan(&path)?;
    // Row 已经 derive 了 Serialize，直接转就行 —— 不用再定义一份 DTO。
    // 真实项目那边前后端契约复杂，所以单独有 model.rs 放 DTO。
    Ok(serde_json::json!({ "rows": rows }))
}

/// 从 `plan=xxx.json` 取文件名，并挡住路径穿越。
fn plan_path(query: &str) -> Result<PathBuf, String> {
    let name = param(query, "plan").ok_or("缺少 plan 参数")?;

    // 只收纯文件名。含 / 或 .. 一律拒绝。
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("非法的计划文件名：{name:?}"));
    }

    let path = Path::new(PLAN_DIR).join(&name);
    if !path.exists() {
        return Err(format!("{PLAN_DIR}/{name} 不存在"));
    }
    Ok(path)
}

fn split_query(url: &str) -> (&str, &str) {
    match url.split_once('?') {
        Some((p, q)) => (p, q),
        None => (url, ""),
    }
}

fn param(query: &str, key: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| percent_decode(v))
}

/// 最小的 percent 解码：够用就行，别为这个引依赖。
fn percent_decode(s: &str) -> String {
    let bytes = s.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn header(content_type: &str) -> Header {
    // 浏览器不看 Content-Type 会把 JSON 当纯文本显示。
    Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
        .expect("Content-Type 头是写死的常量，不可能构造失败")
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
