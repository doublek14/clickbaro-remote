//! 클릭바로 원격지원 — 고객 PC 에서 이 프로그램이 켜지면 접속 정보(ID·임시 비밀번호)를
//! support.clickbaro.com 에 알린다. 그래서 고객은 화면의 번호를 불러 주거나 입력하지 않는다.
//!
//! - 어느 신청 건인지는 **내려받은 파일 이름**에 든 `cbs-<토큰>` 으로 안다
//!   (support.clickbaro.com 이 신청마다 다른 이름으로 내려준다).
//!   포터블 실행은 압축을 풀어 다른 경로에서 돌기 때문에, 원래 이름은 `RUSTDESK_APPNAME` 에 있다.
//! - 토큰이 없는 이름(그냥 RustDesk 로 받은 파일)이면 아무것도 보내지 않는다.
//! - 처음 한 번 + 값이 바뀔 때 + 1분마다(아직 켜져 있음) 보낸다. 창을 닫으면 더 보내지 않는다.
//! ⛔ 보내는 곳은 이 주소 하나다. 화면 내용·파일은 보내지 않는다.

use hbb_common::log;
use std::time::{Duration, Instant};

const REPORT_URL: &str = "https://support.clickbaro.com/api/agent/ready";
const HEARTBEAT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_secs(5);

/// 파일 이름에서 `cbs-` 뒤의 영숫자 16~64자를 꺼낸다.
pub fn token_from_name(name: &str) -> Option<String> {
    let start = name.rfind("cbs-")? + 4;
    let token: String = name[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if (16..=64).contains(&token.len()) {
        Some(token)
    } else {
        None
    }
}

fn exe_name() -> String {
    if let Ok(name) = std::env::var(crate::common::PORTABLE_APPNAME_RUNTIME_ENV_KEY) {
        if !name.is_empty() {
            return name;
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

static STARTED: std::sync::Once = std::sync::Once::new();

/// 초보 고객용 화면 — 모든 프로세스(화면·서버·연결관리)가 시작할 때 load_custom_client() 끝에서 부른다.
/// 공식 맞춤 프로그램 설정과 같은 자리(HARD/BUILTIN)에 넣는다.
///  - conn-type=incoming : 「원격 PC 조종」 칸을 없애고 내 ID·비밀번호만 보인다(수신 전용)
///  - disable-installation : 「Install」 단추·분홍 안내를 없앤다 — 설치되면 상시 접속(무인)이 가능해지므로 막는다
///  - disable-settings/ab/account, hide-*-settings : 서버 주소를 바꾸거나 계정 메뉴로 헤매지 않게
///  - hide-help-cards : UAC·설치 권유 카드를 숨긴다
pub fn apply_defaults() {
    use hbb_common::config;
    {
        let mut hard = config::HARD_SETTINGS.write().unwrap();
        for (k, v) in [
            ("conn-type", "incoming"),
            ("disable-installation", "Y"),
            ("disable-settings", "Y"),
            ("disable-ab", "Y"),
            ("disable-account", "Y"),
        ] {
            hard.insert(k.to_owned(), v.to_owned());
        }
    }
    let mut builtin = config::BUILTIN_SETTINGS.write().unwrap();
    for k in [
        "hide-help-cards",
        "hide-server-settings",
        "hide-proxy-settings",
        "hide-network-settings",
    ] {
        builtin.insert(k.to_owned(), "Y".to_owned());
    }
}

/// 화면이 켜질 때 한 번 부른다. 토큰이 없으면 바로 돌아온다.
/// 🔴 RustDesk 의 `post_request` 는 첫 시도가 실패하면 인증서 검사를 끄고 다시 보낸다 — 비밀번호를 싣는
///    이 보고에는 쓰지 않는다. reqwest 를 직접 써서 **항상 인증서를 검사**하고, 실패하면 다음 주기에 다시 한다.
pub fn start_reporter() {
    let Some(token) = token_from_name(&exe_name()) else {
        return;
    };
    STARTED.call_once(move || {
        std::thread::spawn(move || {
            let client = match reqwest::blocking::Client::builder()
                .use_rustls_tls()
                .timeout(Duration::from_secs(15))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("clickbaro: http client {e}");
                    return;
                }
            };
            let mut sent = (String::new(), String::new());
            let mut sent_at: Option<Instant> = None;
            // 서버가 거절(4xx)하면 5초마다 두드리지 않고 1분 쉰다 — 끝난 신청·잘못된 토큰이면 계속 거절된다.
            let mut wait_until: Option<Instant> = None;
            loop {
                if wait_until.map_or(false, |t| Instant::now() < t) {
                    std::thread::sleep(POLL);
                    continue;
                }
                let id = crate::ui_interface::get_id();
                let password = crate::ui_interface::temporary_password();
                if !id.is_empty() && !password.is_empty() {
                    let changed = sent.0 != id || sent.1 != password;
                    if changed || sent_at.map_or(true, |t| t.elapsed() >= HEARTBEAT) {
                        let body = serde_json::json!({
                            "token": token,
                            "id": id,
                            "password": password,
                            "version": crate::VERSION,
                        });
                        match client.post(REPORT_URL).json(&body).send() {
                            Ok(r) if r.status().is_success() => {
                                sent = (id, password);
                                sent_at = Some(Instant::now());
                            }
                            Ok(r) => {
                                log::warn!("clickbaro: report refused {}", r.status());
                                if r.status().is_client_error() {
                                    wait_until = Some(Instant::now() + HEARTBEAT);
                                }
                            }
                            Err(e) => log::warn!("clickbaro: report failed {e}"),
                        }
                    }
                }
                std::thread::sleep(POLL);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::token_from_name;

    #[test]
    fn reads_token_from_download_name() {
        assert_eq!(
            token_from_name(r"C:\Users\a\Downloads\클릭바로원격지원_cbs-0123456789abcdef0123456789abcdef.exe").as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        // 같은 이름을 두 번 받으면 브라우저가 " (1)" 을 붙인다
        assert_eq!(
            token_from_name("클릭바로원격지원_cbs-0123456789abcdef0123456789abcdef (1).exe").as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert_eq!(token_from_name("rustdesk-1.4.0.exe"), None);
        assert_eq!(token_from_name("cbs-short.exe"), None);
    }
}
