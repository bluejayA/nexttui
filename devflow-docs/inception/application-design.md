# Application Design — #34 Multi-Scope Token Map

**Mode**: DETAIL
**Depth**: Minimal
**Timestamp**: 2026-03-26T16:40:00+09:00

## 변경 대상 컴포넌트

### 1. `src/port/types.rs` — TokenScope 타입 추가

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TokenScope {
    Project { name: String, domain: String },
    Unscoped,
}

impl TokenScope {
    pub fn from_credential(credential: &AuthCredential) -> Self {
        match &credential.project_scope {
            Some(p) => Self::Project {
                name: p.name.clone(),
                domain: p.domain_name.clone(),
            },
            None => Self::Unscoped,
        }
    }

    /// Compute a deterministic hash string for cache file naming.
    pub fn cache_key(&self) -> String {
        match self {
            Self::Project { name, domain } => format!("project_{name}_{domain}"),
            Self::Unscoped => "unscoped".to_string(),
        }
    }
}
```

**설계 결정:**
- `port/types.rs`에 배치 — 도메인 타입이므로 어댑터가 아닌 포트 계층에 위치
- `Hash + Eq` — HashMap 키로 사용
- `Serialize + Deserialize` — 디스크 캐시 키 매핑에 필요할 수 있으나, 실제 캐시 파일명은 `cache_key()` 문자열로 생성
- `cache_key()`는 FNV-1a 해시 대신 가독성 있는 문자열 — 디버깅 시 캐시 디렉토리를 직접 확인 가능

### 2. `src/adapter/auth/keystone.rs` — 구조체 변경

**Before:**
```rust
pub struct KeystoneAuthAdapter {
    current_token: Arc<RwLock<Option<Token>>>,
    cache_path: PathBuf,
    // ...
}
```

**After:**
```rust
pub struct KeystoneAuthAdapter {
    token_map: Arc<RwLock<HashMap<TokenScope, Token>>>,
    active_scope: TokenScope,
    cache_dir: PathBuf,  // 디렉토리 (파일 → 디렉토리로 변경)
    // ...
}
```

**핵심 변경:**
- `current_token: Option<Token>` → `token_map: HashMap<TokenScope, Token>` — 다중 scope 보관
- `cache_path: PathBuf` (단일 파일) → `cache_dir: PathBuf` (디렉토리) — scope별 파일 저장
- `active_scope: TokenScope` — 현재 활성 scope, `new()`에서 credential 기반으로 결정

**get_token() 흐름:**
```
get_token()
  → start_refresh_loop() (idempotent)
  → token_map.read().get(&active_scope)
  → valid? → return token.id
  → expired/miss? → refresh_token() → save to map + disk
```

**refresh loop:**
- active_scope 기준으로만 갱신 (변경 없음, 기존 단일 토큰과 동일)
- 갱신 시 `token_map`에 active_scope 키로 저장

### 3. `src/adapter/auth/token_cache.rs` — 캐시 경로 변경

**Before:**
```
~/.cache/nexttui/auth/{cloud_hash}          ← 단일 파일
```

**After:**
```
~/.cache/nexttui/auth/{cloud_hash}/          ← 디렉토리
  project_admin_Default                      ← scope별 파일
  unscoped
```

**변경 사항:**
- `cache_file_path(cache_key)` → `cache_dir_path(cloud_key)` — 디렉토리 경로 반환
- `save_token(token, path)` → `save_token(token, cache_dir, scope)` — scope별 파일명 생성
- `load_token(path)` → `load_all_tokens(cache_dir)` — 디렉토리 내 모든 scope 토큰 로드
- `compute_cache_key()` — 변경 없음 (cloud 레벨 키)

### 4. 영향 없는 컴포넌트

- `worker.rs` — `get_token()` 인터페이스 변경 없음
- `port/auth.rs` — AuthProvider trait 변경 없음
- `adapter/http/base.rs` — `authenticate_request()` 인터페이스 변경 없음
- `demo.rs` — 토큰 캐시 미사용

## 하위 호환성

- `AuthProvider` trait 인터페이스 변경 없음 — 외부 호출자 영향 0
- 기존 단일 캐시 파일(`~/.cache/nexttui/auth/{hash}`)은 자연 무효화 — 경로가 디렉토리로 변경되므로 기존 파일 무시
- `broadcast::Sender<Token>`은 active_scope 토큰만 브로드캐스트 — 기존 구독자 영향 없음
