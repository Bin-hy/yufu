# cobotmap

CoBotrix 的 Google Maps JavaScript API 代理模块。

## 配置

```bash
cp conf/cobotmap.example.toml conf/cobotmap.toml
```

```toml
[http]
listen = "127.0.0.1:8080"

[maps]
key = "google-maps-api-key"

[proxy]
url = "socks5h://127.0.0.1:7897"

[log]
level = "trace"
```

也可以用本地文件离线加载入口 JS：

```toml
[maps]
input = "./maps-api.js"
```

## 启动

```bash
just cobotmap-run conf/cobotmap.toml
```

访问：

- `http://127.0.0.1:8080/maps/api/js`
- `http://127.0.0.1:8080/demo.html`

验证 Maps JavaScript API 入口：

```bash
just cobotmap-verify 127.0.0.1:8080
```

## 入口

- `src/cobotmap.rs`：服务入口
- `src/cobotmap_verify.rs`：验证入口
- `cobotmap/src/app.rs`：HTTP 服务
- `cobotmap/src/upstream.rs`：SOCKS5 上游请求
- `cobotmap/src/routes.rs`：URL 改写
