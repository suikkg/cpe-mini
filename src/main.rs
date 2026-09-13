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
            let parsed = match parse_run_args(&args[1..]) {
                Ok(p) => p,
                Err(e) => return fail(&e),
            };
            let Some(path) = parsed.positional.first() else {
                return fail("run 后面要跟计划文件，例如：run fixtures/plan.json");
            };
            let out = parsed
                .positional
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_output);
            run_and_report(PathBuf::from(path), out, parsed.cancel_after)
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

/// `run` 的参数拆开之后长这样。
#[derive(Debug, PartialEq)]
struct RunArgs {
    /// 位置参数，按出现顺序：计划文件、输出文件
    positional: Vec<String>,
    /// `--cancel-after N`，没给就是 `None`
    cancel_after: Option<usize>,
}

/// 解析 `run` 后面的全部参数。
///
/// ## 为什么不用「第 2 个参数就是输出文件」那种写法
///
/// 因为选项可以插在位置参数中间：
///
/// ```text
/// run 计划.json --cancel-after 1 输出.jsonl
/// ```
///
/// 按位置取的话，`args[2]` 是 `--cancel-after`，输出路径就被**默默忽略**了，
/// 报告写到默认位置，而且没有任何提示。用户下次来问「我指定的文件怎么是空的」。
///
/// 同理，不认识的选项要**当场报错**，不能跳过。`--cancel-aftr` 拼错一个字母
/// 就静默地不生效，是这个项目一直在防的那类「安静的错误」。
///
/// ## 为什么单独抽成一个函数
///
/// 为了能测。`parse` 和 `run` 分开，解析就不用起进程、不用写文件也能验 ——
/// 和扩展课 13 那条「`parse` 必须和进程启动分开」是同一条规矩。
fn parse_run_args(args: &[String]) -> Result<RunArgs, String> {
    let mut positional = Vec::new();
    let mut cancel_after = None;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--cancel-after" => {
                let Some(raw) = args.get(i + 1) else {
                    return Err("--cancel-after 后面要跟一个数字，例如：--cancel-after 2".into());
                };
                cancel_after = Some(
                    raw.parse::<usize>()
                        .map_err(|_| format!("--cancel-after 要跟一个非负整数，当前是 `{raw}`"))?,
                );
                i += 2;
            }
            other if other.starts_with("--") => {
                return Err(format!(
                    "不认识的选项：{other}\n\nrun 只认 --cancel-after N"
                ));
            }
            other => {
                positional.push(other.to_string());
                i += 1;
            }
        }
    }

    if positional.len() > 2 {
        return Err(format!(
            "run 最多两个位置参数（计划文件、输出文件），给了 {} 个：{}",
            positional.len(),
            positional.join(" ")
        ));
    }

    Ok(RunArgs {
        positional,
        cancel_after,
    })
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

#[cfg(test)]
// 测试名用中文；夹着大写 ASCII 会触发 non_snake_case。
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 只给计划文件() {
        let p = parse_run_args(&args(&["plan.json"])).unwrap();
        assert_eq!(p.positional, vec!["plan.json"]);
        assert_eq!(p.cancel_after, None);
    }

    #[test]
    fn 计划加输出() {
        let p = parse_run_args(&args(&["plan.json", "out.jsonl"])).unwrap();
        assert_eq!(p.positional, vec!["plan.json", "out.jsonl"]);
    }

    #[test]
    fn 选项插在位置参数中间也要认得出输出路径() {
        // 这是抽成 parse_run_args 的理由：按 args[2] 取的话，
        // 输出路径会被默默忽略，报告写到默认位置且没有任何提示。
        let p = parse_run_args(&args(&["plan.json", "--cancel-after", "1", "out.jsonl"])).unwrap();
        assert_eq!(p.positional, vec!["plan.json", "out.jsonl"]);
        assert_eq!(p.cancel_after, Some(1));
    }

    #[test]
    fn 选项放最后也一样() {
        let p = parse_run_args(&args(&["plan.json", "out.jsonl", "--cancel-after", "2"])).unwrap();
        assert_eq!(p.positional, vec!["plan.json", "out.jsonl"]);
        assert_eq!(p.cancel_after, Some(2));
    }

    #[test]
    fn 拼错的选项要报错不能静默忽略() {
        // --cancel-aftr 少一个字母。静默跳过的话，用户以为设了取消，
        // 结果整轮跑完——这就是这个项目一直在防的「安静的错误」。
        let e = parse_run_args(&args(&["plan.json", "--cancel-aftr", "1"])).unwrap_err();
        assert!(e.contains("不认识的选项"), "报错要说清楚：{e}");
        assert!(e.contains("--cancel-aftr"), "报错要带上用户敲的原话：{e}");
    }

    #[test]
    fn 选项后面缺数字() {
        let e = parse_run_args(&args(&["plan.json", "--cancel-after"])).unwrap_err();
        assert!(e.contains("要跟一个数字"), "{e}");
    }

    #[test]
    fn 选项后面不是数字() {
        let e = parse_run_args(&args(&["plan.json", "--cancel-after", "甲"])).unwrap_err();
        // 报错要带上用户敲的原话，不然他不知道自己敲错了什么
        assert!(e.contains('甲'), "{e}");
    }

    #[test]
    fn 位置参数太多要报错() {
        let e = parse_run_args(&args(&["a.json", "b.jsonl", "c.jsonl"])).unwrap_err();
        assert!(e.contains("最多两个"), "{e}");
    }
}
