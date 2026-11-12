//! Server-rendered pages for RP-initiated logout.
//!
//! The pages deliberately use inline styles and inline SVG only: the identity
//! origin serves them with strict response headers and no external assets.

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const STYLE: &str = r#"
:root {
  color-scheme: light;
  --background: 200 20% 97%;
  --foreground: 202 32% 12%;
  --card: 0 0% 100%;
  --card-foreground: 202 32% 12%;
  --primary: 174 72% 24%;
  --primary-foreground: 0 0% 100%;
  --muted: 200 16% 95%;
  --muted-foreground: 200 10% 42%;
  --accent: 172 34% 93%;
  --accent-foreground: 174 65% 20%;
  --destructive: 356 70% 46%;
  --destructive-foreground: 0 0% 100%;
  --success: 158 62% 30%;
  --border: 200 16% 88%;
  --input: 200 14% 82%;
  --ring: 174 72% 30%;
  --radius: 18px;
  --shadow-sm: 0 1px 2px hsl(202 30% 12% / 0.04);
  --shadow-lg: 0 28px 80px -24px hsl(202 30% 12% / 0.32);
}
@media (prefers-color-scheme: dark) {
  :root {
    color-scheme: dark;
    --background: 200 28% 8%;
    --foreground: 195 20% 92%;
    --card: 200 24% 11%;
    --card-foreground: 195 20% 92%;
    --primary: 170 56% 62%;
    --primary-foreground: 200 40% 9%;
    --muted: 200 18% 15%;
    --muted-foreground: 195 12% 66%;
    --accent: 192 22% 18%;
    --accent-foreground: 170 50% 75%;
    --destructive: 356 70% 66%;
    --destructive-foreground: 356 50% 10%;
    --success: 156 55% 58%;
    --border: 200 14% 22%;
    --input: 200 12% 30%;
    --ring: 170 56% 62%;
    --shadow-sm: 0 1px 2px hsl(0 0% 0% / 0.28);
    --shadow-lg: 0 30px 90px -24px hsl(0 0% 0% / 0.72);
  }
}
* { box-sizing: border-box; }
html, body { min-height: 100%; }
body {
  margin: 0;
  min-width: 320px;
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 24px;
  font-family: 'Inter', system-ui, -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', sans-serif;
  font-size: 14px;
  line-height: 1.5;
  color: hsl(var(--foreground));
  background:
    radial-gradient(1100px 560px at 8% -12%, hsl(var(--primary) / 0.18), transparent 60%),
    radial-gradient(900px 520px at 100% 0%, hsl(var(--accent) / 0.72), transparent 55%),
    hsl(var(--background));
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}
::selection { background: hsl(var(--primary) / 0.22); }
.card {
  position: relative;
  width: min(100%, 460px);
  padding: 34px 32px 26px;
  overflow: hidden;
  background: hsl(var(--card));
  color: hsl(var(--card-foreground));
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius);
  box-shadow: var(--shadow-lg);
  animation: rise 240ms cubic-bezier(0.22, 1, 0.36, 1) both;
}
.card::before {
  content: "";
  position: absolute;
  inset: 0 0 auto;
  height: 4px;
  background: linear-gradient(90deg, hsl(var(--primary)), hsl(var(--primary) / 0.28));
}
.icon {
  display: grid;
  place-items: center;
  width: 56px;
  height: 56px;
  margin-bottom: 22px;
  color: hsl(var(--primary));
  background: linear-gradient(145deg, hsl(var(--primary) / 0.16), hsl(var(--primary) / 0.05));
  border: 1px solid hsl(var(--primary) / 0.18);
  border-radius: 18px;
  box-shadow: inset 0 1px 0 hsl(0 0% 100% / 0.35);
}
.icon svg { width: 27px; height: 27px; }
.icon-success {
  color: hsl(var(--success));
  background: linear-gradient(145deg, hsl(var(--success) / 0.16), hsl(var(--success) / 0.05));
  border-color: hsl(var(--success) / 0.2);
}
h1 {
  margin: 0 0 8px;
  font-size: 23px;
  font-weight: 650;
  line-height: 1.25;
  letter-spacing: -0.02em;
}
.lead {
  margin: 0 0 20px;
  color: hsl(var(--muted-foreground));
  font-size: 14px;
  line-height: 1.7;
}
.lead strong {
  color: hsl(var(--foreground));
  font-weight: 600;
}
.scope {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  margin-bottom: 26px;
  padding: 12px 14px;
  color: hsl(var(--muted-foreground));
  font-size: 12.5px;
  line-height: 1.6;
  background: hsl(var(--muted));
  border: 1px solid hsl(var(--border));
  border-radius: 11px;
}
.scope svg {
  flex: none;
  width: 16px;
  height: 16px;
  margin-top: 2px;
  color: hsl(var(--primary));
}
.actions { display: grid; gap: 10px; }
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 44px;
  padding: 12px 18px;
  font: inherit;
  font-size: 14px;
  font-weight: 600;
  line-height: 1;
  text-align: center;
  text-decoration: none;
  border: 1px solid transparent;
  border-radius: 9px;
  cursor: pointer;
  transition: background-color 130ms ease, border-color 130ms ease, color 130ms ease, box-shadow 130ms ease, transform 100ms ease;
}
.btn:active { transform: translateY(0.5px); }
.btn:focus-visible { outline: 2px solid hsl(var(--ring)); outline-offset: 2px; }
.btn-danger {
  color: hsl(var(--destructive-foreground));
  background: hsl(var(--destructive));
  box-shadow: 0 1px 2px hsl(var(--destructive) / 0.24);
}
.btn-danger:hover { background: hsl(var(--destructive) / 0.88); }
.btn-primary {
  color: hsl(var(--primary-foreground));
  background: hsl(var(--primary));
  box-shadow: 0 1px 2px hsl(var(--primary) / 0.28), inset 0 1px 0 hsl(0 0% 100% / 0.12);
}
.btn-primary:hover { background: hsl(var(--primary) / 0.9); }
.btn-secondary {
  color: hsl(var(--foreground));
  background: hsl(var(--card));
  border-color: hsl(var(--input));
  box-shadow: var(--shadow-sm);
}
.btn-secondary:hover {
  color: hsl(var(--accent-foreground));
  background: hsl(var(--accent) / 0.6);
  border-color: hsl(var(--primary) / 0.42);
}
.footer {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  margin-top: 26px;
  padding-top: 18px;
  color: hsl(var(--muted-foreground));
  font-size: 11.5px;
  border-top: 1px solid hsl(var(--border));
}
.footer svg { width: 13px; height: 13px; }
@keyframes rise {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: none; }
}
@media (prefers-reduced-motion: reduce) {
  .card { animation: none; }
  .btn { transition: none; }
}
@media (max-width: 480px) {
  body { padding: 16px; }
  .card { padding: 28px 22px 22px; border-radius: 16px; }
  h1 { font-size: 21px; }
}
"#;

pub(crate) fn confirmation(
    application_name: Option<&str>,
    form_action: &str,
    hidden_fields: &[(&str, &str)],
    cancel_url: Option<&str>,
) -> String {
    let application = escape(application_name.unwrap_or("当前应用"));
    let action = escape(form_action);
    let hidden = hidden_fields
        .iter()
        .map(|(name, value)| {
            format!(
                r#"<input type="hidden" name="{}" value="{}">"#,
                escape(name),
                escape(value)
            )
        })
        .collect::<String>();
    let cancel = match cancel_url {
        Some(url) => format!(
            r#"<a class="btn btn-secondary" href="{}">取消，返回应用</a>"#,
            escape(url)
        ),
        None => r#"<a class="btn btn-secondary" href="/">取消</a>"#.to_owned(),
    };
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<meta name="referrer" content="same-origin">
<title>确认退出 · cobotusr</title>
<style>{STYLE}</style>
</head>
<body>
<main class="card" aria-labelledby="title">
  <div class="icon" aria-hidden="true">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
      <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>
      <polyline points="16 17 21 12 16 7"/>
      <line x1="21" y1="12" x2="9" y2="12"/>
    </svg>
  </div>
  <h1 id="title">确认退出</h1>
  <p class="lead"><strong>{application}</strong> 请求退出当前浏览器的统一登录会话。</p>
  <div class="scope">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="9"/>
      <line x1="12" y1="11" x2="12" y2="16"/>
      <line x1="12" y1="8" x2="12.01" y2="8"/>
    </svg>
    <span>退出后，该浏览器上的应用授权将一并撤销；其他浏览器不受影响。</span>
  </div>
  <form method="post" action="{action}">
    {hidden}
    <div class="actions">
      <button class="btn btn-danger" type="submit">确认退出</button>
      {cancel}
    </div>
  </form>
  <footer class="footer">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="4" y="10" width="16" height="11" rx="2"/>
      <path d="M8 10V7a4 4 0 0 1 8 0v3"/>
    </svg>
    <span>cobotusr 统一身份认证</span>
  </footer>
</main>
</body>
</html>"#
    )
}

pub(crate) fn completed() -> String {
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>已退出 · cobotusr</title>
<style>{STYLE}</style>
</head>
<body>
<main class="card" aria-labelledby="title">
  <div class="icon icon-success" aria-hidden="true">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="9"/>
      <path d="m8.5 12 2.5 2.5 4.5-5"/>
    </svg>
  </div>
  <h1 id="title">已退出</h1>
  <p class="lead">当前浏览器的统一登录会话已结束。</p>
  <div class="scope">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="9"/>
      <line x1="12" y1="11" x2="12" y2="16"/>
      <line x1="12" y1="8" x2="12.01" y2="8"/>
    </svg>
    <span>如需继续使用，请重新登录。其他浏览器中的会话不受影响。</span>
  </div>
  <div class="actions">
    <a class="btn btn-primary" href="/">返回登录</a>
  </div>
  <footer class="footer">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="4" y="10" width="16" height="11" rx="2"/>
      <path d="M8 10V7a4 4 0 0 1 8 0v3"/>
    </svg>
    <span>cobotusr 统一身份认证</span>
  </footer>
</main>
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_escape_dynamic_values_and_keep_the_form_contract() {
        let page = confirmation(
            Some("<script>alert(1)</script>"),
            "/oidc/end-session",
            &[("flow_id", "a&b"), ("csrf_token", "x\"y")],
            Some("https://app.test/?a=1&b=2"),
        );
        assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!page.contains("<script>alert(1)</script>"));
        assert!(page.contains(r#"name="flow_id" value="a&amp;b""#));
        assert!(page.contains(r#"name="csrf_token" value="x&quot;y""#));
        assert!(page.contains(r#"href="https://app.test/?a=1&amp;b=2""#));
        assert!(page.contains(r#"action="/oidc/end-session""#));
        assert!(page.contains("确认退出"));
    }

    #[test]
    fn completed_page_has_a_safe_return_link() {
        let page = completed();
        assert!(page.contains("已退出"));
        assert!(page.contains(r#"href="/""#));
    }
}
