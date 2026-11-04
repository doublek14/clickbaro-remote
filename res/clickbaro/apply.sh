#!/usr/bin/env bash
# 클릭바로 원격지원 — 빌드 직전에 공식 RustDesk 기본값 세 가지를 우리 것으로 바꾼다.
#  ① 안내 서버: support.clickbaro.com (NAS 의 hbbs/hbbr)
#  ② 서버 공개 열쇠: NAS hbbs 가 만든 id_ed25519.pub (공개값이다 — 개인 열쇠는 NAS 밖으로 나오지 않는다)
#  ③ 앱 이름: ClickbaroRemote — RustDesk 가 「맞춤 프로그램」으로 보고 공식 자동 업데이트를 끈다
#     (업데이트되면 공식 서버로 바뀌어 우리 서버를 못 쓴다)
set -euo pipefail
SERVER="${CLICKBARO_RENDEZVOUS:-support.clickbaro.com}"
KEY="${CLICKBARO_RS_PUB_KEY:-GBHcN5xIDrRE3lyK2kbRzrw5GgAlX5rmLOveazqnLqU=}"
F=libs/hbb_common/src/config.rs
grep -q 'pub const RENDEZVOUS_SERVERS: &\[&str\] = &\["rs-ny.rustdesk.com"\];' "$F"
grep -q 'pub const RS_PUB_KEY: &str = "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=";' "$F"
grep -q 'pub static ref APP_NAME: RwLock<String> = RwLock::new("RustDesk".to_owned());' "$F"
python3 - "$F" "$SERVER" "$KEY" <<'PY'
import sys
p, server, key = sys.argv[1:]
s = open(p, encoding="utf-8").read()
s = s.replace('pub const RENDEZVOUS_SERVERS: &[&str] = &["rs-ny.rustdesk.com"];',
              f'pub const RENDEZVOUS_SERVERS: &[&str] = &["{server}"];')
s = s.replace('pub const RS_PUB_KEY: &str = "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=";',
              f'pub const RS_PUB_KEY: &str = "{key}";')
s = s.replace('pub static ref APP_NAME: RwLock<String> = RwLock::new("RustDesk".to_owned());',
              'pub static ref APP_NAME: RwLock<String> = RwLock::new("ClickbaroRemote".to_owned());')
open(p, "w", encoding="utf-8").write(s)
PY
grep -n 'RENDEZVOUS_SERVERS: \|RS_PUB_KEY: \|APP_NAME: ' "$F"
