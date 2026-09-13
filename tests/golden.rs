#![allow(non_snake_case)]
//! 黄金文件测试：把「给人看的输出」逐字钉死。
//!
//! 测试名用中文；夹着大写 ASCII 会触发 non_snake_case，见文件头的 allow。
//!
//! ## 这一层在防什么
//!
//! 前面那 86 个测试都在问「这个值对不对」。它们管不了这件事：
//!
//! > 有人给 `render` 的表头多加了两个空格，或者把「无有效数据」改成了
//! > 「N/A」，所有断言照样全绿——因为没有一条断言提到过表头。
//!
//! 而下游的脚本、Excel 模板、贴进周报的截图，全都认那个格式。
//! **报告的排版是对外兼容面**，和 `Verdict` 的大写串是一个性质。
//!
//! 黄金文件就是把整段输出存成一个文件，下次跑出来的必须和它一模一样。
//! 改动是**故意的**就更新文件（一行命令），改动是**手滑**就当场红。
//!
//! ```bash
//! cargo test --test golden          # 检查
//! UPDATE_GOLDEN=1 cargo test --test golden   # 认可这次改动，更新黄金文件
//! ```
//!
//! 更新完记得 `git diff fixtures/golden/` 看一眼——**那个 diff 就是
//! 你这次改动对用户可见的全部影响**。这是黄金文件最值钱的地方。
//!
//! ## 与真实项目的对应
//!
//! `cpe-test` 的 `src/report/` 有一批同样思路的测试，钉的是 xlsx 的列顺序
//! 和 HTML 对比报告的结构。真实项目还多一件事：跨版本的报告要能互相读，
//! 所以黄金文件同时兼做「格式没变过」的证据。

use cpe_mini::cancel::CancelFlag;
use cpe_mini::{compare, report, run_plan, run_plan_cancellable};
use std::path::{Path, PathBuf};

/// 把每次跑都会变的东西换成占位符。
///
/// 时间戳是唯一一个。不脱敏的话黄金文件每秒钟都对不上，测试永远是红的，
/// 红成常态之后就没人看它了——**一个总是红的测试等于没有测试**。
///
/// 脱敏要脱得尽量少：换掉的每一处都是「这里将来变了也不会有人发现」。
fn normalize(text: &str) -> String {
    text.lines()
        .map(|line| {
            // CSV 的第一列是 unix 时间戳
            match line.split_once(',') {
                Some((head, rest))
                    if head.trim_start_matches('\u{feff}').parse::<u64>().is_ok() =>
                {
                    format!("<TIME>,{rest}")
                }
                _ => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn golden_path(name: &str) -> PathBuf {
    Path::new("fixtures/golden").join(name)
}

/// 比对；`UPDATE_GOLDEN=1` 时改成写入。
fn assert_golden(name: &str, actual: &str) {
    let actual = normalize(actual);
    let path = golden_path(name);

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&path, &actual).expect("写黄金文件失败");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "黄金文件 {} 不存在。\n第一次加这个用例时跑：\n    UPDATE_GOLDEN=1 cargo test --test golden",
            path.display()
        )
    });

    if expected != actual {
        // 直接打出第一处不同，省得在几十行里用眼睛找
        let diff = expected
            .lines()
            .zip(actual.lines())
            .enumerate()
            .find(|(_, (e, a))| e != a)
            .map(|(i, (e, a))| format!("第 {} 行：\n  黄金文件：{e}\n  这次跑出来：{a}", i + 1))
            .unwrap_or_else(|| {
                format!(
                    "行数不一样：黄金文件 {} 行，这次 {} 行",
                    expected.lines().count(),
                    actual.lines().count()
                )
            });

        panic!(
            "输出和 {} 对不上。\n\n{diff}\n\n\
             改动是故意的就更新它：\n    UPDATE_GOLDEN=1 cargo test --test golden\n\
             然后 git diff fixtures/golden/ 看一眼变了什么。",
            path.display()
        );
    }
}

fn rows_of(plan: &str) -> Vec<report::Row> {
    run_plan(Path::new(plan)).expect("计划应该能跑通")
}

#[test]
fn 渲染三种典型结果() {
    assert_golden(
        "plan.render.txt",
        &report::render(&rows_of("fixtures/plan.json")),
    );
}

#[test]
fn 渲染五种边界情况() {
    assert_golden(
        "plan_edge.render.txt",
        &report::render(&rows_of("fixtures/plan_edge.json")),
    );
}

#[test]
fn 渲染丢包诊断() {
    // 丢包 31% 却判 PASS（ADR-17）。这段文字是给人看的，改了要有人知道。
    assert_golden(
        "plan_loss.render.txt",
        &report::render(&rows_of("fixtures/plan_loss.json")),
    );
}

#[test]
fn 导出CSV的列顺序和转义() {
    // 列顺序是兼容面：下游 Excel 模板按列号取值，插一列就全错位。
    assert_golden("plan.csv", &report::to_csv(&rows_of("fixtures/plan.json")));
}

#[test]
fn 对比两轮的报告() {
    let before = rows_of("fixtures/plan_v1.json");
    let after = rows_of("fixtures/plan_v2.json");
    let diff = compare::compare(&before, &after, true);
    assert_golden("compare_v1_v2.txt", &compare::render(&diff));
}

#[test]
fn 取消之后剩下的单元还在报告里() {
    let rows = run_plan_cancellable(
        Path::new("fixtures/plan_bidir.json"),
        &CancelFlag::trip_after_polls(1),
    )
    .expect("计划应该能跑通");

    assert_golden("plan_bidir.cancelled.render.txt", &report::render(&rows));
}
