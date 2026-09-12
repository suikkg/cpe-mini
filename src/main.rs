//! 程序入口。
//!
//! ## 与真实项目的对应
//!
//! 这个 `main` → `real_main(args) -> i32` → `match mode` 的结构，
//! 和 `cpe-test` 的 `src/main.rs` 一模一样（真实项目那里有 14 个分支：
//! inner / agent / master / report / compare / scan / monitor / ui …）。
//!
//! `main` 自己不干活：它把退出码交给 `real_main`，这样每个分支都能明确
//! 返回 0 还是 1，而不是到处 `std::process::exit`。

use cpe_mini::{default_output, demo_plan_path, preview_plan, report, run_plan};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(real_main(args));
}

fn real_main(args: Vec<String>) -> i32 {
    // 和真实项目同一个写法：取不到就当空串，交给 match 的 "" 分支
    let mode = args.first().map(|s| s.as_str()).unwrap_or("");

    match mode {
        "demo" => run_and_report(demo_plan_path(), default_output()),

        "plan" => {
            let Some(path) = args.get(1) else {
                return fail("plan 后面要跟计划文件，例如：plan fixtures/plan.json");
            };
            match preview_plan(&PathBuf::from(path)) {
                Ok(text) => {
                    println!("{text}");
                    0
                }
                Err(e) => fail(&e),
            }
        }

        "run" => {
            let Some(path) = args.get(1) else {
                return fail("run 后面要跟计划文件，例如：run fixtures/plan.json");
            };
            let out = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(default_output);
            run_and_report(PathBuf::from(path), out)
        }

        "report" => {
            let Some(path) = args.get(1) else {
                return fail("report 后面要跟报告文件，例如：report output/rows.jsonl");
            };
            match report::read_jsonl(&PathBuf::from(path)) {
                Ok((rows, skipped)) => {
                    for s in &skipped {
                        eprintln!("警告：{s}");
                    }
                    println!("{}", report::render(&rows));
                    0
                }
                Err(e) => fail(&e),
            }
        }

        "-h" | "--help" | "help" => {
            println!("{}", help());
            0
        }

        "" => {
            println!("{}", help());
            0
        }

        other => fail(&format!("不认识的命令：{other}\n\n{}", help())),
    }
}

fn run_and_report(plan_path: PathBuf, out_path: PathBuf) -> i32 {
    let rows = match run_plan(&plan_path) {
        Ok(rows) => rows,
        Err(e) => return fail(&e),
    };

    println!("{}", report::render(&rows));

    match report::write_jsonl(&out_path, &rows) {
        Ok(()) => {
            println!("\n报告已保存：{}", out_path.display());
            // 退出码给脚本用：有问题才返回 1。
            //
            // 注意 Measured（没设门限，只记录）和 Skip 不算问题——
            // 拿 `pass == total` 当判据是错的：一份全是「只记录」的计划
            // 永远返回 1，脚本就再也分不出真失败和没设门限。
            let t = report::tally(&rows);
            let bad = t.rate_fail + t.setup_error + t.not_evaluated;
            if bad == 0 {
                0
            } else {
                1
            }
        }
        Err(e) => fail(&e),
    }
}

fn fail(msg: &str) -> i32 {
    eprintln!("错误：{msg}");
    1
}

fn help() -> String {
    [
        "cpe-mini —— cpe-test 的教学缩微版",
        "",
        "用法：",
        "  cargo run -- demo                      跑内置样例计划",
        "  cargo run -- plan <计划文件>            只展开，看会跑成什么样",
        "  cargo run -- run <计划文件> [输出文件]   跑一份计划并存报告",
        "  cargo run -- report <报告文件>          读回报告重新渲染",
        "",
        "样例：",
        "  cargo run -- plan fixtures/plan.json",
        "  cargo run -- run fixtures/plan.json output/rows.jsonl",
        "  cargo run -- report output/rows.jsonl",
    ]
    .join("\n")
}
