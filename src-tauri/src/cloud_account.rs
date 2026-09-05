//! Cloud credentials belong to the OS vault, separate from local backend JWTs.
use keyring::{Entry, Error};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Default)]
pub struct BrowserLoginState(Mutex<Option<BrowserLoginFlow>>);

struct BrowserLoginFlow {
    state: String,
    result: Arc<Mutex<Option<Result<String, String>>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for BrowserLoginFlow {
    fn drop(&mut self) { self.task.abort(); }
}

fn callback_code(request: &str, port: u16, state: &str) -> Option<String> {
    let mut lines = request.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    if first.next()? != "GET" { return None; }
    let target = first.next()?;
    if !target.starts_with("/callback?") { return None; }
    let expected_host = format!("127.0.0.1:{port}");
    if !lines.any(|line| line.split_once(':').is_some_and(|(key, value)| key.eq_ignore_ascii_case("host") && value.trim() == expected_host)) { return None; }
    let url = tauri::Url::parse(&format!("http://{expected_host}{target}")).ok()?;
    let pairs: Vec<_> = url.query_pairs().collect();
    // Reject duplicate parameters instead of silently choosing an interpretation.
    if pairs.len() != 2 { return None; }
    let returned_state = pairs.iter().find(|(key, _)| key == "state")?.1.as_ref();
    let code = pairs.iter().find(|(key, _)| key == "code")?.1.as_ref();
    if returned_state != state || code.len() != 43 || !code.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-') { return None; }
    Some(code.to_string())
}

async fn receive_callback(mut stream: TcpStream, port: u16, state: &str) -> Option<String> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while request.len() < 8192 {
        let count = stream.read(&mut buffer).await.ok()?;
        if count == 0 { return None; }
        request.extend_from_slice(&buffer[..count]);
        if request.windows(4).any(|part| part == b"\r\n\r\n") { break; }
    }
    let code = callback_code(std::str::from_utf8(&request).ok()?, port, state);
    let (status, body) = if code.is_some() {
        ("200 OK", "<!doctype html><meta charset=utf-8><title>灵枢网页登录</title><h1>网页验证完成</h1><p>请返回灵枢，软件正在完成登录。你可以关闭此页面。</p>")
    } else { ("400 Bad Request", "Invalid login callback. Please return to LingShu and try again.") };
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
    code
}

#[tauri::command]
pub async fn cloud_browser_login_start(window: tauri::WebviewWindow, login: tauri::State<'_, BrowserLoginState>, state: String) -> Result<String, String> {
    require_main(&window)?;
    if state.len() != 64 || !state.bytes().all(|c| c.is_ascii_hexdigit()) { return Err("无效的登录请求".into()); }
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|_| "无法启动本机登录回调")?;
    let port = listener.local_addr().map_err(|_| "无法读取本机登录端口")?.port();
    let result = Arc::new(Mutex::new(None));
    let task_result = result.clone();
    let task_state = state.clone();
    let task = tokio::spawn(async move {
        let received = tokio::time::timeout(Duration::from_secs(300), async {
            loop {
                let (stream, _) = listener.accept().await.map_err(|_| "网页登录回调已中断".to_string())?;
                if let Ok(Some(code)) = tokio::time::timeout(Duration::from_secs(3), receive_callback(stream, port, &task_state)).await { return Ok(code); }
            }
        }).await.unwrap_or_else(|_| Err("网页登录已超时，请重试".into()));
        if let Ok(mut slot) = task_result.lock() { *slot = Some(received); }
    });
    *login.0.lock().map_err(|_| "登录状态不可用")? = Some(BrowserLoginFlow { state, result, task });
    Ok(format!("http://127.0.0.1:{port}/callback"))
}

#[tauri::command]
pub fn cloud_browser_login_poll(window: tauri::WebviewWindow, login: tauri::State<'_, BrowserLoginState>, state: String) -> Result<Option<String>, String> {
    require_main(&window)?;
    let mut flow = login.0.lock().map_err(|_| "登录状态不可用")?;
    let current = flow.as_ref().filter(|flow| flow.state == state).ok_or("登录已取消，请重试")?;
    let value = current.result.lock().map_err(|_| "登录状态不可用")?.take();
    if let Some(result) = value {
        *flow = None;
        if result.is_ok() { let _ = window.show(); let _ = window.set_focus(); }
        result.map(Some)
    } else { Ok(None) }
}

#[tauri::command]
pub fn cloud_browser_login_cancel(window: tauri::WebviewWindow, login: tauri::State<'_, BrowserLoginState>, state: String) -> Result<(), String> {
    require_main(&window)?;
    let mut flow = login.0.lock().map_err(|_| "登录状态不可用")?;
    if flow.as_ref().is_some_and(|flow| flow.state == state) { *flow = None; }
    Ok(())
}

const SERVICE: &str = "com.lingshu.desktop.cloud-account";
const ACCOUNT: &str = "account.lingeringdream.com";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|_| "无法访问系统凭据存储".to_string())
}

fn require_main(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("云账户仅允许在主控制台管理".into());
    }
    Ok(())
}

#[tauri::command]
pub fn cloud_credential_load(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    require_main(&window)?;
    match entry()?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(Error::NoEntry) => Ok(None),
        Err(_) => Err("无法读取云账户凭据，请检查系统凭据存储是否已解锁".into()),
    }
}

#[tauri::command]
pub fn cloud_credential_save(window: tauri::WebviewWindow, value: String) -> Result<(), String> {
    require_main(&window)?;
    if value.is_empty() || value.len() > 2048 {
        return Err("无效的云账户凭据".into());
    }
    entry()?.set_password(&value).map_err(|_| "无法安全保存云账户登录状态".into())
}

#[tauri::command]
pub fn cloud_credential_clear(window: tauri::WebviewWindow) -> Result<(), String> {
    require_main(&window)?;
    match entry()?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(_) => Err("无法清除云账户凭据，请重试退出".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_requires_matching_state_host_and_unambiguous_code() {
        let state = "a".repeat(64);
        let code = "b".repeat(43);
        let request = format!("GET /callback?state={state}&code={code} HTTP/1.1\r\nHost: 127.0.0.1:54321\r\n\r\n");
        assert_eq!(callback_code(&request, 54321, &state), Some(code));
        assert!(callback_code(&request, 54321, &"c".repeat(64)).is_none());
        assert!(callback_code(&request.replace("Host: 127.0.0.1", "Host: attacker.example"), 54321, &state).is_none());
        assert!(callback_code(&request.replace(" HTTP/1.1", "&code=another HTTP/1.1"), 54321, &state).is_none());
        assert!(callback_code(&request.replace("GET /", "POST /"), 54321, &state).is_none());
    }

    #[test]
    fn callback_receives_a_real_loopback_request() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let state = "a".repeat(64);
            let code = "b".repeat(43);
            let task = tokio::spawn(async move {
                let (stream, address) = listener.accept().await.unwrap();
                assert!(address.ip().is_loopback());
                receive_callback(stream, port, &state).await
            });
            let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let request = format!("GET /callback?state={}&code={code} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n", "a".repeat(64));
            client.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).await.unwrap();
            assert!(response.starts_with("HTTP/1.1 200 OK"));
            assert!(response.contains("网页验证完成"));
            assert!(!response.contains(&code));
            assert_eq!(task.await.unwrap(), Some(code));
        });
    }

    // Explicit opt-in: exercises a unique test entry in the real OS vault.
    #[test]
    #[ignore]
    fn system_vault_round_trip() {
        let account = format!("test-{}", std::process::id());
        let entry = Entry::new(SERVICE, &account).unwrap();
        entry.set_password("test-refresh-token").unwrap();
        let value = entry.get_password().unwrap();
        entry.delete_credential().unwrap();
        assert_eq!(value, "test-refresh-token");
        assert!(matches!(entry.get_password(), Err(Error::NoEntry)));
    }
}
