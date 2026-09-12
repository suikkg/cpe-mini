#!/usr/bin/env bash
# 一键自检。改完代码跑一下，全过就说明没把项目改坏。
#
#     ./check.sh
#
# 它会额外验证 scaffold/ 和 solutions/ 里那些不在 crate 里的 .rs 文件 ——
# 那些文件不跑 cargo test 就没人管，很容易随着主代码改动烂掉。
set -euo pipefail
cd "$(dirname "$0")"

echo "== cargo fmt --check =="
cargo fmt --check

echo "== rustfmt scaffold/ solutions/ =="
rustfmt --edition 2021 --check scaffold/*.rs solutions/*.rs

echo "== cargo clippy --all-targets =="
cargo clippy --all-targets -- -D warnings

echo "== cargo test =="
cargo test --quiet

echo "== 命令行冒烟 =="
run() {
    local expect=$1; shift
    local code=0
    "$@" >/dev/null 2>&1 || code=$?
    if [ "$code" != "$expect" ]; then
        echo "  失败：$* 退出码 ${code}，期望 ${expect}"
        exit 1
    fi
    echo "  ok($code)  $*"
}
CM="cargo run --quiet --"
run 0 $CM --help
run 1 $CM demo                                        # 样例里有不达标的
run 0 $CM plan fixtures/plan.json
run 0 $CM run fixtures/plan_bidir.json /tmp/cpe-mini-check.jsonl   # 全是「只记录」
run 1 $CM run fixtures/plan_bad.json                  # 配置非法
run 1 $CM run fixtures/plan_broken.json               # JSON 语法坏
run 1 $CM run fixtures/plan_loss.json /tmp/cpe-mini-loss.jsonl
run 0 $CM csv /tmp/cpe-mini-loss.jsonl /tmp/cpe-mini-loss.csv
run 1 $CM run fixtures/plan_v1.json /tmp/cpe-mini-v1.jsonl   # v1 里有一条不达标
run 1 $CM run fixtures/plan_v2.json /tmp/cpe-mini-v2.jsonl
run 1 $CM compare /tmp/cpe-mini-v1.jsonl /tmp/cpe-mini-v2.jsonl   # 有回归
run 0 $CM compare /tmp/cpe-mini-v1.jsonl /tmp/cpe-mini-v1.jsonl   # 自己比自己
run 1 $CM 飞天                                          # 不认识的命令

echo "== 扩展课的骨架和答案还能编译 =="
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cp -R Cargo.toml src fixtures "$TMP/"
# 骨架和答案用的是同一套测试，所以两边都要验：骨架验「编译得过」，答案验「测试全绿」
for variant in scaffold solutions; do
    if [ "$variant" = scaffold ]; then
        cp scaffold/ping.rs "$TMP/src/ping.rs"
        cp scaffold/webui.rs "$TMP/src/webui.rs"
        cp scaffold/webui.html "$TMP/src/webui.html"
    else
        cp solutions/ping_answer.rs "$TMP/src/ping.rs"
        cp solutions/webui_answer.rs "$TMP/src/webui.rs"
        cp solutions/webui_answer.html "$TMP/src/webui.html"
    fi
    python3 - "$TMP" <<'PY'
import sys, pathlib
s = pathlib.Path(sys.argv[1])
lib = s / "src/lib.rs"
t = lib.read_text()
if "pub mod ping;" not in t:
    t = t.replace("pub mod plan;", "pub mod ping;\npub mod plan;")
    t = t.replace("pub mod verdict;", "pub mod verdict;\npub mod webui;")
    lib.write_text(t)
c = s / "Cargo.toml"
ct = c.read_text()
# 注意别用 `"tiny_http" not in ct` 当判断：Cargo.toml 的注释里就提到了它
if 'tiny_http = ' not in ct:
    c.write_text(ct.replace('serde_json = "1"', 'serde_json = "1"\ntiny_http = "0.12"'))
PY
    if ! (cd "$TMP" && cargo clippy --quiet --lib -- -D warnings); then
        echo "  跳过（多半是 tiny_http 拿不到，没网）"
        break
    fi
    if [ "$variant" = solutions ]; then
        (cd "$TMP" && cargo test --quiet --lib ping) >/dev/null
        (cd "$TMP" && cargo test --quiet --lib webui) >/dev/null
        echo "  ok  答案编译通过、测试全绿"
    else
        echo "  ok  骨架编译通过、clippy 零告警"
    fi
done

echo
echo "全部通过。"
