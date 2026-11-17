# 验收

验收只看协议行为，不依赖控制台页面。三条行为全部成立才算接入完成。

## 行为一：登录

| 观测 | 期望 |
| --- | --- |
| 访问 `/login` | 302 到 issuer 的 `/oidc/authorize`，带 `state`、`nonce`、`code_challenge`、`code_challenge_method=S256` |
| 登录并同意 | 回调带 `code` 与相同 `state` |
| 回调完成 | 本地会话建立，能读到稳定的 `sub` |

命令：

```bash
bash "$SKILL_DIR/scripts/verify-app.sh"
```

它用已登录会话请求授权端点，校验 client、redirect、scope 的配对关系，返回 2xx/3xx 即通过。

## 行为二：令牌校验

| 观测 | 期望 |
| --- | --- |
| 无 `Authorization` 头 | `401`，带 `WWW-Authenticate: Bearer error="missing_token"` |
| 有效令牌 | `200`，claims 中 `sub` 可读 |
| audience 不符 | `401 invalid_token` |
| 已过期 | `401 invalid_token` |

命令：

```bash
bash "$SKILL_DIR/scripts/scaffold-resource-server.sh" --dir ./internal/auth --pkg auth
```

生成的测试覆盖上述四种情况，`go test` 失败即验收失败。

## 行为三：退出

| 观测 | 期望 |
| --- | --- |
| 访问 `/logout` | 本地会话被清除 |
| 退出后访问受保护页 | `401`，重新要求登录 |

yufu 的中心 SSO 会话独立于应用会话：在控制台退出后，`GET /api/v1/account/session`
必须从 `200` 变为 `401`。`/oidc/logout` 确认页在本机版本要求 legacy `refresh_token`
cookie，中心会话确认会返回 `400 missing refresh token`；应用侧退出不要依赖该页面。

## 部署层验收

```bash
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

三项：issuer 一致、JWKS 非空、health ready 且时钟同步。

## 自动化的边界

登录与同意页面需要真人操作，因此行为一的完整浏览器流程由人工确认；
脚本负责验证「client 与参数配对」这一最容易出错的环节。退出同理。