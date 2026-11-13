# 产品可用性验收证据

实施基线：`455d08bcf3f453b5cbb5a246ce1cc899ddf3ebcb`。开始时工作区只有尚未提交的 `docs/specs/product-readiness/` 四份已批准文档；未有实现修改。时间按 Asia/Shanghai 展示，机器记录使用带时区 ISO 8601。

状态：pending（未执行）、pass（实际通过）、fail（实际失败）、blocked（执行条件不满足）。历史记录不自动变为本次通过；三引擎、三平台及最终候选源码必须有对应记录。

`scripts/readiness.py run <check>` 仅运行内置本地检查，按次生成 `runs/<id>/record.json` 和脱敏 `output.log`。`verify` 核验指定证据或整个 runs 目录；附件哈希、源码摘要或测试数量不符均拒绝。`baseline` 只输出被允许源码的路径与摘要，不读取本地配置、签名私钥或环境秘密。`capabilities` 校验声明及证据引用。

只收集虚构 fixture 的输出。不得上传未经脱敏的浏览器 trace、截图、认证请求或配置文件。自动脱敏是附加防线，不能代替关闭秘密请求录制。最终报告见上一级 verification.md；本目录的存在不表示验收完成。
