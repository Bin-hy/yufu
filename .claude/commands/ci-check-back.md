按照 CI 流程依次对后端 Rust 代码进行检查。在项目根目录下按顺序执行以下步骤，每步失败则停止并报告错误：

1. **格式检查** — `cargo fmt --all -- --check`
   - 若失败，列出未格式化的文件，提示运行 `cargo fmt --all` 修复

2. **Clippy 静态分析** — `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - 若失败，列出所有 warning/error，并给出修复建议

3. **编译构建** — `cargo build --workspace --all-targets --all-features --release`
   - 若失败，分析编译错误原因

<!-- 4. **单元测试** — `cargo test --workspace --all-targets --all-features` -->
   <!-- - 若失败，列出失败的测试用例及错误信息 -->

每步完成后输出 ✓ / ✗ 状态。全部通过后给出总结。
