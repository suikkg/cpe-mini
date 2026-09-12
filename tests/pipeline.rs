#![allow(non_snake_case)]
//! 全链路集成测试：从计划文件一路跑到报告。
//!
//! 测试名用中文；夹着大写 ASCII 会触发 non_snake_case，见文件末的 allow。
//!
//! 单元测试盯的是单个函数，集成测试盯的是**模块之间的接缝**——
//! 一个模块单独测都对、拼起来就错，这类问题只有集成测试能抓到。

use cpe_mini::report::{read_jsonl, rows_from, tally, write_jsonl};
use cpe_mini::verdict::Verdict;
use cpe_mini::{builder, executor, plan, run_plan};
use std::path::Path;

#[test]
fn 样例计划跑出预期的三种结果() {
    let rows = run_plan(Path::new("fixtures/plan.json")).expect("样例计划应该能跑通");

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].verdict, Verdict::Pass);
    assert_eq!(rows[1].verdict, Verdict::RateFail);
    assert_eq!(rows[2].verdict, Verdict::NotEvaluated);

    // 关键区分：没数据是「无法评价」，不能算成 CPE 性能失败
    assert_ne!(rows[2].verdict, Verdict::RateFail);
}

#[test]
fn 同一份计划跑两次结果完全一样() {
    let a = run_plan(Path::new("fixtures/plan.json")).unwrap();
    let b = run_plan(Path::new("fixtures/plan.json")).unwrap();

    assert_eq!(tally(&a), tally(&b));
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.verdict, y.verdict);
        assert_eq!(x.rx_avg, y.rx_avg);
        assert_eq!(x.reason_code, y.reason_code);
    }
}

#[test]
fn 双向两轮展开成六行() {
    let rows = run_plan(Path::new("fixtures/plan_bidir.json")).unwrap();
    // 2 轮 × (双向 2 腿 + 单向 1 腿) = 6
    assert_eq!(rows.len(), 6);

    // 双向单元的两行共享同一个 parent_id
    let ab = &rows[0];
    let ba = &rows[1];
    assert_eq!(ab.parent_id, ba.parent_id);
    assert_ne!(ab.task_id, ba.task_id);
}

#[test]
fn 没有门限的测试只记录不判定() {
    let rows = run_plan(Path::new("fixtures/plan_bidir.json")).unwrap();
    let udp = rows.iter().find(|r| r.transport == "udp").unwrap();
    assert_eq!(udp.verdict, Verdict::Measured);
    assert!(udp.rx_avg.is_some(), "没有门限也要测出数来");
}

#[test]
fn 轮次在报告里分得开() {
    let rows = run_plan(Path::new("fixtures/plan_bidir.json")).unwrap();
    let r1 = rows.iter().filter(|r| r.round == 1).count();
    let r2 = rows.iter().filter(|r| r.round == 2).count();
    assert_eq!(r1, 3);
    assert_eq!(r2, 3);

    // id 也必须能分开，否则 HashMap 一去重就只剩最后一轮
    let ids: std::collections::HashSet<&str> = rows.iter().map(|r| r.task_id.as_str()).collect();
    assert_eq!(ids.len(), rows.len(), "每一行都该有唯一的 task_id");
}

#[test]
fn 非法配置在开跑前就被拦下() {
    let err = run_plan(Path::new("fixtures/plan_bad.json")).unwrap_err();
    // 一次报全部问题
    assert!(err.contains("plan_id"));
    assert!(err.contains("transport"));
    assert!(err.contains("target_mbps"));
}

#[test]
fn JSON语法错误的报错能定位到行列() {
    let err = run_plan(Path::new("fixtures/plan_broken.json")).unwrap_err();
    assert!(err.contains("line"), "serde 的报错该带行号：{err}");
}

#[test]
fn 报告存盘再读回来统计一致() {
    let rows = run_plan(Path::new("fixtures/plan_edge.json")).unwrap();
    let path = std::env::temp_dir().join("cpe-mini-it-roundtrip.jsonl");

    write_jsonl(&path, &rows).unwrap();
    let (loaded, skipped) = read_jsonl(&path).unwrap();

    assert!(skipped.is_empty());
    assert_eq!(tally(&loaded), tally(&rows));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn 边界用例_刚好等于门限算通过() {
    let rows = run_plan(Path::new("fixtures/plan_edge.json")).unwrap();
    let row = rows
        .iter()
        .find(|r| r.task.contains("刚好等于门限"))
        .unwrap();
    assert_eq!(row.verdict, Verdict::Pass, ">= 而不是 >");
}

#[test]
fn 手工走一遍四个阶段和一条龙结果相同() {
    // 这个测试的意义：证明 run_plan 只是把四步串起来，没有夹带私货
    let path = Path::new("fixtures/plan.json");

    let p = plan::load(path).unwrap();
    let units = builder::build_units(&p);
    let outcomes = executor::execute_plan(&units);
    let manual = rows_from(&outcomes);

    let oneshot = run_plan(path).unwrap();

    assert_eq!(tally(&manual), tally(&oneshot));
}

#[test]
fn 只记录的计划不算失败() {
    // plan_bidir 里 UDP 那条没设门限，判 Measured。
    // 退出码判据是「rate_fail + setup_error + not_evaluated 是否为 0」，
    // 不是「pass 是否等于 total」——后者会让这份计划永远报失败。
    let rows = run_plan(Path::new("fixtures/plan_bidir.json")).unwrap();
    let t = tally(&rows);

    assert!(t.measured > 0, "这份 fixture 该有 Measured 行");
    assert_ne!(t.pass, t.total, "所以 pass == total 这个判据在这里是假的");
    assert_eq!(
        t.rate_fail + t.setup_error + t.not_evaluated,
        0,
        "但它没有任何真问题"
    );
}
