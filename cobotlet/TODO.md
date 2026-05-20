# cobotlet 端侧中间件 — 已完成事项与实现思路

## 本次做了什么

把 [cobotlet](src/) 从一个仅有 `/hello` 示例路由的最小骨架，改造成机器人端侧的 agent，具备四项核心能力：

1. **设备注册 + 心跳** — 机器人本体的各服务通过 HTTP 主动注册到 cobotlet
2. **能力声明 (Capability)** — 注册时携带 "标签 + HTTP 路由描述" 的能力清单
3. **任务接受与分发** — cobotsrv 通过 MQTT 下发任务，cobotlet 按能力匹配设备并 HTTP 同步转发
4. **MQTT 改用普通 pub/sub 客户端** — 用 `rumqttc` 接 NanoMQ broker，与 cobotsrv 通信解耦于 `net4mqtt` 反向代理

## 模块结构

```
cobotlet/src/
├── config.rs        新增 [storage] [device] [mqtt] 三段配置
├── store/
│   ├── mod.rs
│   └── device.rs    DeviceStore：redb 持久化 + DashMap 内存索引
├── events.rs        EventSink trait + NoopSink（解耦 routes 与 MQTT）
├── mqtt.rs          rumqttc 客户端：订阅 tasks/+，publish 设备/结果事件
├── dispatcher.rs    Task / TaskResult / 按能力路由 → reqwest 同步转发
├── heartbeat.rs     watchdog：周期扫描，超时设备翻转为 offline
├── route/
│   ├── mod.rs       AppState 扩展为 { config, store, http_client, events }
│   ├── device.rs    POST/GET/DELETE /devices, PUT /devices/:id/heartbeat
│   ├── task.rs      POST /tasks（调试入口，复用 dispatcher）
│   └── hello.rs     原有
└── lib.rs           serve() 装配上述所有组件
```

## 关键实现思路

### 1. 数据层 — 双写 + 内存索引

`DeviceStore` 同时维护：
- **redb 单表** `devices: &str -> &str`，value 是 `Device` 的 JSON 字符串
- **`DashMap<id, Device>`** — 读热路径
- **`DashMap<capability_name, HashSet<device_id>>`** — 能力倒排索引，O(1) 找出能做某能力的设备

写策略 (性能权衡)：
- `register / remove / 状态翻转` → 写 redb + 写内存 (双写)
- `heartbeat` → 只更新内存的 `last_heartbeat` (避免 redb 高频写)；崩溃后 `last_heartbeat` 可能回退几秒，但设备会在下一次心跳恢复

启动时 `load_from_disk()` 把所有设备读回内存并重建能力索引。

### 2. 解耦层 — EventSink trait

routes 和 dispatcher 不直接依赖 `rumqttc`，而是 hold 一个 `Arc<dyn EventSink>`：

```rust
trait EventSink {
    fn publish_online(&self, device: &Device);
    fn publish_heartbeat(&self, device_id: &str);
    fn publish_offline(&self, device: &Device);
    fn publish_task_result(&self, result: &TaskResult);
}
```

两个实现：
- `NoopSink` — `[mqtt]` 段未配置时使用，所有方法是 no-op，路由代码无需关心
- `MqttSink` — 实际通过 `AsyncClient` 发布到对应 topic

好处：单元测试不需要 broker，直接用 `Arc::new(NoopSink)` 注入即可。

### 3. Topic 约定

根前缀：`cobotrix/cobotlets/{cobotlet_alias}`

| 方向 | Topic | QoS | Retain |
|------|-------|-----|--------|
| cobotlet → srv | `.../online` | 1 | yes (LWT) |
| cobotlet → srv | `.../devices/{id}/online` | 1 | yes |
| cobotlet → srv | `.../devices/{id}/heartbeat` | 1 | no |
| cobotlet → srv | `.../devices/{id}/offline` | 1 | yes |
| srv → cobotlet | `.../tasks/{task_id}` | 1 | no |
| cobotlet → srv | `.../results/{task_id}` | 1 | no |

LWT 巧用：连接到 broker 时声明遗嘱 = "向 `.../online` 发空 retained payload"。一旦 cobotlet 进程崩溃 / 网络断开，broker 自动替它发遗嘱，cobotsrv 不必探活就能感知。正常上线时 cobotlet 主动发非空 payload 覆盖。

### 4. 任务分发 — 错误装信封不 panic

`dispatch()` 流程：

```
Task → 选设备 (device_id 优先 / capability_name 兜底)
     → 找 capability → 拼 service_url + path
     → reqwest 发请求 (method/headers/body 透传)
     → 包装为 TaskResult { status, headers, body, error? }
```

任何失败 (设备不存在 / 离线 / capability 不匹配 / HTTP 失败 / body 解析失败) 都装入 `TaskResult.error` 字段，永远返回 `TaskResult` 让上层正常 publish 回去。这避免任务"消失"——cobotsrv 总能拿到一个有意义的结果。

### 5. MQTT eventloop — 单个后台任务

`mqtt::spawn()` 做三件事：
1. 用 LWT 配置建立 `MqttOptions`，创建 `(AsyncClient, EventLoop)` 对
2. 主动 publish "上线" + subscribe `tasks/+`
3. `tokio::spawn` 一个无限 loop 轮询 `eventloop.poll()`：
   - 收到 `Publish` 且 topic 匹配 `tasks/{id}` → 反序列化 → `dispatch` → `publish_task_result`
   - 出错 → 退避 1s 后继续 (rumqttc 内部会自动重连)

publish 调用本身不阻塞调用方：所有 publish 都是 `tokio::spawn(async move { client.publish(...).await })`，让 routes 不会因为 MQTT 慢被拖慢。

### 6. 心跳 watchdog — 只报"刚翻转"的事件

`heartbeat::spawn_watchdog` 周期 (默认 5s) 调用 `store.sweep_offline(timeout_secs)`，该方法只返回**这一轮刚从 online 翻成 offline** 的设备列表，watchdog 仅对这些设备发一次 offline 事件。已经离线的设备不会被重复 publish。

## 配置示例

[conf/cobotlet.toml](../conf/cobotlet.toml) 已加上模板。最小可运行配置：

```toml
[http]
listen = "[::]:3080"

[storage]
path = "data/cobotlet.redb"

[device]
heartbeat_timeout_secs = 30
request_timeout_ms = 5000
watchdog_interval_secs = 5

[mqtt]                           # 省略整段则单机运行 (HTTP 仍可用)
url = "mqtt://127.0.0.1:1883"
alias = "cobotlet-0"
```

cobotnil 开发集群的三份配置 ([conf/cobotnil/cobotlet.{0,1,2}.toml](../conf/cobotnil/)) 已带上独立的 `storage.path` 和 `mqtt.alias`，避免互相覆盖。

## 验证

- `cargo fmt --all -- --check` ✅
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` ✅
- `cargo build --workspace --all-targets --all-features` ✅
- `cargo test -p cobotlet --all-features` — **10/10 通过**
  - DeviceStore 6 项：注册查询 / 心跳复活 / 扫描下线 / 重注册换能力 / 持久化往返 / 删除清索引
  - Dispatcher 4 项：无设备 / 设备不存在 / 离线设备拒绝 / 转发失败装入 error

## 关键决策

| 决策 | 取舍 |
|------|------|
| `redb` 4.x 而不是 sled | 更新更稳定，纯 Rust，单文件，API 比 sled 简洁 |
| `rumqttc` 客户端 | Rust MQTT 生态最成熟；自动重连内置 |
| 心跳不落盘 | 性能优先；崩溃后回退几秒可接受 |
| `EventSink` trait + Noop | routes 与 MQTT 解耦，测试无需 broker |
| 错误装入 `TaskResult.error` 不 panic | cobotsrv 永远能拿到结果，方便观测 |
| `net4mqtt` feature 保留 | 不破坏现有 SOCKS-over-MQTT 反向代理；后续可独立清理 |
| 设备 ID 双策略 | body 带 `id` 用之，否则 `Uuid::new_v4()`，兼顾"设备序列号"与"零配置注册" |
| 无鉴权 | 假定 cobotlet 与设备在同主机/可信内网；后续可加 per-device token |

## 后续 (不在本次范围)

1. **cobotsrv 端对接** — 订阅 `cobotrix/cobotlets/+/devices/+/online`，把设备纳入 cobotsrv 的 `Storage`；提供 `POST /api/v1/tasks` 把任务 publish 到对应 topic
2. **NanoMQ 部署** — 把 [src/cobotnil.rs:48](../src/cobotnil.rs#L48) 内嵌的 `net4mqtt::broker` 替换为独立的 NanoMQ 容器，更新 docker-compose
3. **per-device token** — 注册时 cobotlet 颁发一次性 token，后续心跳/任务校验
4. **net4mqtt feature 清理** — 一旦确认所有调用方都走新 MQTT 路径，移除反向代理依赖
5. **能力 schema 校验** — 任务 body 按 capability.schema 做 JSON Schema 校验后再转发
