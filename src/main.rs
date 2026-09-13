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

use cpe_mini::cancel::CancelFlag;
use cpe_mini::{
    compare, default_output, demo_plan_path, preview_plan, report, run_plan_cancellable,
};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(real_main(args));
}

fn real_main(args: Vec<String>) -> i32 {
    // 和真实项目同一个写法：取不到就当空串，交给 match 的 "" 分支
    let mode = args.first().map(|s| s.as_str()).unwrap_or("");

    match mode {
        "demo" => run_and_report(demo_plan_path(), default_output(), None),

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
            // --cancel-after N：跑完 N 个单元就叫停（第 18 课的演示开关）
            let cancel_after = match parse_cancel_after(&args) {
                Ok(v) => v,
                Err(e) => return fail(&e),
            };
            let out = args
                .get(2)
                .filter(|a| !a.starts_with("--"))
                .map(PathBuf::from)
                .unwrap_or_else(default_output);
            run_and_report(PathBuf::from(path), out, cancel_after)
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

        "csv" => {
            let Some(path) = args.get(1) else {
                return fail("csv 后面要跟报告文件，例如：csv output/rows.jsonl");
            };
            let out = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("output/rows.csv"));

            match report::read_jsonl(&PathBuf::from(path)) {
                Ok((rows, skipped)) => {
                    for s in &skipped {
                        eprintln!("警告：{s}");
                    }
                    match report::write_csv(&out, &rows) {
                        Ok(()) => {
                            println!("已导出 {} 行到 {}", rows.len(), out.display());
                            0
                        }
                        Err(e) => fail(&e),
                    }
                }
                Err(e) => fail(&e),
            }
        }

        "compare" => {
            let (Some(before_path), Some(after_path)) = (args.get(1), args.get(2)) else {
                return fail("compare 后面要跟两份报告，例如：compare output/before.jsonl output/after.jsonl");
            };

            // 真实项目靠 Row 上的 plan_hash 自动判断两轮是不是同一份计划。
            // cpe-mini 的 Row 没有那个字段，所以这里由用户显式说明。
            //
            // 刻意**不**从 task_id 里反解 plan_id：那是从展示串里搜字段，
            // 和 builder.rs 里「轮次要是类型化字段，不能从标题后缀里搜」
            // 是同一个错误。缺字段就把它加上，不要拿字符串凑。
            // 第 15 课的动手任务就是把 plan_hash 这个字段补上。
            let same_plan = !args.iter().any(|a| a == "--plan-changed");

            let before = match report::read_jsonl(&PathBuf::from(before_path)) {
                Ok((rows, _)) => rows,
                Err(e) => return fail(&e),
            };
            let after = match report::read_jsonl(&PathBuf::from(after_path)) {
                Ok((rows, _)) => rows,
                Err(e) => return fail(&e),
            };

            let diff = compare::compare(&before, &after, same_plan);
            println!("{}", compare::render(&diff));

            // 退出码给 CI 用：有回归就别放行。
            if diff.has_regression() {
                1
            } else {
                0
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

/// 解析 `--cancel-after N`。
///
/// 单独抽出来是为了能测：`parse` 和 `run` 分开，解析就不用起进程也能验。
fn parse_cancel_after(args: &[String]) -> Result<Option<usize>, String> {
    let Some(i) = args.iter().position(|a| a == "--cancel-after") else {
        return Ok(None);
    };
    let Some(raw) = args.get(i + 1) else {
        return Err("--cancel-after 后面要跟一个数字，例如：--cancel-after 2".into());
    };
    raw.parse::<usize>()
        .map(Some)
        .map_err(|_| format!("--cancel-after 要跟一个非负整数，当前是 `{raw}`"))
}

fn run_and_report(plan_path: PathBuf, out_path: PathBuf, cancel_after: Option<usize>) -> i32 {
    // 没给 --cancel-after 时这个标志永远不置位，行为和以前一模一样。
    let flag = match cancel_after {
        Some(n) => CancelFlag::trip_after_polls(n),
        None => CancelFlag::new(),
    };

    let rows = match run_plan_cancellable(&plan_path, &flag) {
        Ok(rows) => rows,
        Err(e) => return fail(&e),
    };

    if flag.peek() {
        println!(
            "整轮测试在第 {} 个单元后被取消。\n",
            cancel_after.unwrap_or(0)
        );
    }

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
        "       [--cancel-after N]              跑完 N 个单元就叫停（第 18 课）",
        "  cargo run -- report <报告文件>          读回报告重新渲染",
        "  cargo run -- compare <旧> <新>          对比两份报告，有回归返回 1",
        "  cargo run -- csv <报告文件> [输出]       导出 CSV（给 Excel 用）",
        "",
        "样例：",
        "  cargo run -- plan fixtures/plan.json",
        "  cargo run -- run fixtures/plan.json output/rows.jsonl",
        "  cargo run -- report output/rows.jsonl",
        "  cargo run -- compare output/before.jsonl output/after.jsonl",
        "",
        "两轮跑的不是同一份计划时，加 --plan-changed。",
        "被取消的单元不会从报告里消失，它们是 SKIP / CANCELLED。",
    ]
    .join("\n")
}
