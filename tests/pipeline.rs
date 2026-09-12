#![allow(non_snake_case)]
//! 全链路集成测试：从计划文件一路跑到报告。
//!
//! 测试名用中文；夹着大写 ASCII 会触发 non_snake_case，见文件末的 allow。
//!
//! 单元测试盯的是单个函数，集成测试盯的是**模块之间的接缝**——
//! 一个模块单独测都对、拼起来就错，这类问题只有集成测试能抓到。

use cpe_mini::compare::{compare, DeltaKind};
use cpe_mini::report::{read_jsonl, rows_from, tally, to_csv, write_jsonl, CSV_COLUMNS};
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

// ---- 两轮对比 -------------------------------------------------------------

#[test]
fn 两个固件版本的对比抓得出回归() {
    let v1 = run_plan(Path::new("fixtures/plan_v1.json")).expect("v1 应该能跑通");
    let v2 = run_plan(Path::new("fixtures/plan_v2.json")).expect("v2 应该能跑通");

    let diff = compare(&v1, &v2, true);
    assert_eq!(diff.deltas.len(), 4);

    // 四种变化各一条，这正是这两份样例计划设计出来演示的
    assert_eq!(diff.count(DeltaKind::Regressed), 1);
    assert_eq!(diff.count(DeltaKind::SlowerButStillSameVerdict), 1);
    assert_eq!(diff.count(DeltaKind::Fixed), 1);
    assert_eq!(diff.count(DeltaKind::Unchanged), 1);

    // 判定变坏的必须排在最前面：读的人从上往下看就是「先处理最要紧的」
    assert_eq!(diff.deltas[0].kind(), DeltaKind::Regressed);
    assert!(diff.deltas[0].title.contains("TCP 下行"));

    assert!(diff.has_regression(), "有回归就该拦，退出码要是 1");
}

#[test]
fn 和自己比没有任何变化() {
    let rows = run_plan(Path::new("fixtures/plan.json")).unwrap();
    let diff = compare(&rows, &rows, true);

    assert_eq!(diff.count(DeltaKind::Unchanged), rows.len());
    assert!(!diff.has_regression(), "同一份结果自己跟自己比不该报回归");
}

#[test]
fn 计划变了会报成新增和缺失() {
    let a = run_plan(Path::new("fixtures/plan.json")).unwrap();
    let b = run_plan(Path::new("fixtures/plan_v1.json")).unwrap();

    let diff = compare(&a, &b, false);
    // 两份计划的测试项完全不同，所以是「全部缺失 + 全部新增」
    assert_eq!(diff.count(DeltaKind::Disappeared), a.len());
    assert_eq!(diff.count(DeltaKind::Added), b.len());
    // 但这不算回归——计划换了而已，设备表现无从比较
    assert!(!diff.has_regression());
}

// ---- 丢包只进诊断（ADR-17）------------------------------------------------

#[test]
fn 高丢包不改判定但进得了报告() {
    let rows = run_plan(Path::new("fixtures/plan_loss.json")).expect("应该能跑通");
    assert_eq!(rows.len(), 3);

    // 丢包 31.4%，但 RX 平均到了门限 → PASS
    assert_eq!(rows[0].verdict, Verdict::Pass, "丢包不是判定输入（ADR-17）");
    assert_eq!(rows[0].udp_loss, Some(31.4));
    assert!(rows[0].diagnostics.iter().any(|d| d.contains("丢包率")));

    // 零丢包，但 RX 没到门限 → RATE_FAIL。两条加起来才说得清这条规则。
    assert_eq!(rows[1].verdict, Verdict::RateFail);
    assert_eq!(rows[1].udp_loss, Some(0.0));

    // 没采丢包 = None，不是 0.0
    assert_eq!(rows[2].udp_loss, None);
}

#[test]
fn 丢包字段能存能读回() {
    let rows = run_plan(Path::new("fixtures/plan_loss.json")).unwrap();
    let path = std::env::temp_dir().join("cpe-mini-it-loss.jsonl");

    write_jsonl(&path, &rows).unwrap();
    let (loaded, skipped) = read_jsonl(&path).unwrap();

    assert!(skipped.is_empty());
    assert_eq!(loaded[0].udp_loss, Some(31.4));
    assert_eq!(loaded[2].udp_loss, None, "没采到要读回 None，不能变成 0.0");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn 老报告没有丢包字段照样读得出来() {
    // 模拟加字段之前存下来的一行：JSON 里根本没有 udp_loss
    let path = std::env::temp_dir().join("cpe-mini-it-oldrow.jsonl");
    let old = r#"{"time":"1","task_id":"a","parent_id":"a","task":"TCP","transport":"tcp","param":"tcp / ab / 900 Mbps","verdict":"PASS","execution_status":"COMPLETED","reason_code":"RX_TARGET_MET","reason_detail":"","diagnostics":[],"round":1,"rx_avg":950.0}"#;
    std::fs::write(
        &path,
        format!(
            "{old}
"
        ),
    )
    .unwrap();

    let (rows, skipped) = read_jsonl(&path).expect("历史报告必须还能读");
    assert!(skipped.is_empty(), "缺新字段不该被当成坏行跳过");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].udp_loss, None);
    assert_eq!(rows[0].verdict, Verdict::Pass);

    let _ = std::fs::remove_file(&path);
}

// ---- CSV 导出 -------------------------------------------------------------

#[test]
fn 导出的CSV列数和表头对得上() {
    let rows = run_plan(Path::new("fixtures/plan_loss.json")).unwrap();
    let text = to_csv(&rows);
    let mut lines = text.lines();

    let header = lines.next().unwrap().trim_start_matches('\u{feff}');
    assert_eq!(header.split(',').count(), CSV_COLUMNS.len());

    let mut count = 0;
    for line in lines {
        // 这几份样例里没有需要转义的字段
        assert_eq!(line.split(',').count(), CSV_COLUMNS.len(), "错位了：{line}");
        count += 1;
    }
    assert_eq!(count, rows.len());
}
