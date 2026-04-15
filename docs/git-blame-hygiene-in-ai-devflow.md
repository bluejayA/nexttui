# Git Blame Hygiene in AI-Assisted Development (claude-code + devflow)

> 작성: 2026-04-15 (nexttui BL-P2-063 PR #70 컨텍스트에서 정리)
> 목적: `git blame` / `.git-blame-ignore-revs` / merge commit SHA 개념을 기초부터 정리하고, claude-code + devflow 플러그인을 활용한 AI 협업 개발 환경에서 어떻게 활용·강화할지 가이드.
> 독자: nexttui 프로젝트 컨트리뷰터 및 AI 협업 워크플로우를 도입하려는 팀

---

## 0. TL;DR

| 개념 | 한 줄 요약 |
|------|-----------|
| `git blame` | 파일의 각 줄을 "누가/언제/어떤 commit에서 마지막으로 수정했는지" 보여주는 기초 도구 |
| Blame의 약점 | 대규모 mechanical 변경(`cargo fmt`, codemod 등)이 진짜 저자/의도를 가림 |
| `.git-blame-ignore-revs` | 그런 mechanical commit을 blame에서 **투명하게 스킵**시키는 설정 파일 |
| Commit SHA | Git이 commit 고유 식별용으로 쓰는 40자 해시. 내용 기반 생성 |
| Merge commit SHA | PR이 target branch에 편입되며 만들어지는 새 commit의 SHA |
| Squash merge | feature의 여러 commit을 1개로 합쳐 target에 올림 (feature SHA는 버려짐) |
| AI-시대 특수성 | Mechanical 변경 빈도 + AI가 blame을 읽는 빈도 모두 급증 → blame hygiene이 **AI 협업 인프라**가 됨 |

**결론**: `.git-blame-ignore-revs`는 "preference" 수준이 아니라 AI-assisted development에서 **세션 효율·비용**을 실제로 좌우하는 레벨의 인프라.

---

## 1. `git blame` 기초

### 1.1. 개념

`git blame <파일>`은 각 줄이 **언제·누가·어떤 commit에서** 마지막으로 수정되었는지 표시한다.

```bash
git blame src/context/switcher.rs
```

출력 예시:
```
c95526f  Jay  2026-04-13  pub async fn switch(&self, request: ContextRequest) -> Result<...> {
c95526f  Jay  2026-04-13      let target = self.resolver.resolve(request).await?;
bab45d7  Jay  2026-04-14      let epoch = self.state.try_begin(target.clone())?;
bab45d7  Jay  2026-04-14      //                                     ↑ Council C1 반영으로 변경됨
e7ff042  Jay  2026-04-13      let handle = self.session.begin(&target, epoch).await?;
```

각 줄 앞의 해시(`c95526f`, `bab45d7`, `e7ff042`)가 **그 줄을 만든 commit의 SHA**.

### 1.2. 주요 사용 시나리오

| 상황 | 질문 | 해결 경로 |
|------|------|-----------|
| 버그 조사 | "이 로직 어느 PR에서 들어왔지?" | `git blame -L <start>,<end> <file>` → commit SHA → `git log <sha>` → PR 문맥 |
| 코드 리뷰 | "왜 이렇게 짰어요?" | 줄 blame → commit msg 확인 → 의도 파악 |
| 제도적 기억 | "3년 전에 왜 이 캐시 만들었지?" | 최초 commit 발견 → 당시 맥락 복원 |
| GitHub UI | PR 리뷰 중 맥락 확인 | 파일 옆 ⋮ → "View blame" |

### 1.3. Blame의 약점

`git blame`은 **"줄의 마지막 수정 commit"** 만 추적한다. 이게 문제를 만든다:

- **누가** 수정했는지는 알려주지만 **왜**는 commit message 품질에 의존
- 한 번 **대규모 mechanical 변경**이 생기면 진짜 저자 정보가 한 겹 가려진다

---

## 2. 대규모 Mechanical 변경의 문제

### 2.1. 시나리오 — nexttui 사례

**2025-12-01**: Jay가 Action enum에 `DeleteServer` 추가
```rust
DeleteServer { id: String, name: String },
```
→ commit `8a3f9c2` (intentional feature)

**2026-04-15**: BL-P2-063 T2.5에서 `cargo fmt --all` 실행 → 해당 줄이 여러 줄로 분할
```rust
DeleteServer {
    id: String,
    name: String,
},
```
→ commit `b709b61` (mechanical)

**1개월 후**, 누군가 `git blame src/action.rs` 실행하면:
```
b709b61  Jay  2026-04-15      DeleteServer {
b709b61  Jay  2026-04-15          id: String,
b709b61  Jay  2026-04-15          name: String,
b709b61  Jay  2026-04-15      },
```

**rustfmt commit만 보인다.** 진짜 저자(`8a3f9c2`) 정보는 한 겹 뒤로 밀려났다.

### 2.2. 왜 이게 비용인가

| 시나리오 | Blame 오염의 비용 |
|----------|------------------|
| 버그 조사 | 진짜 commit 찾느라 `git log -L`, `git log --follow` 추가 명령 → 5~10분 낭비 |
| 코드 리뷰 | "왜 이렇게?" 답을 못 찾고 작성자에게 직접 질문 → 비동기 왕복 |
| 엔지니어링 분석 | "최근 분기 auth 기여자" 통계가 rustfmt 실행자로 왜곡 |
| 제도적 기억 | 신입이 `is_refresh_safe` 같은 가드 함수 의도 파악 못 하고 제거 → **보안 회귀 가능** |

특히 마지막 시나리오는 **간접적으로 보안 영역**. "blame이 정확해야 보안 의도가 유지된다"는 경로.

---

## 3. `.git-blame-ignore-revs` 해법

### 3.1. 동작 원리

프로젝트 루트에 `.git-blame-ignore-revs` 파일 두고 **blame에서 스킵할 commit SHA 목록** 기록:

```
# .git-blame-ignore-revs
# BL-P2-063 T2.5: cargo fmt --all (rustfmt reformat, 83 files)
b709b61a6f04e32d547e039daeb2e73821146390c

# 향후 mechanical commit은 여기에 추가
```

파일이 있으면 blame이 해당 commit을 **투명하게 통과**해 **바로 이전 commit**으로 넘어간다.

적용 후 같은 `git blame src/action.rs:11`:
```
8a3f9c2  Jay  2025-12-01      DeleteServer {
8a3f9c2  Jay  2025-12-01          id: String,
8a3f9c2  Jay  2025-12-01          name: String,
8a3f9c2  Jay  2025-12-01      },
```

**진짜 저자 복원.** rustfmt commit은 투명하게 스킵.

### 3.2. 활성화 방법

**방법 A — 로컬 git config (선택적)**:
```bash
git config --local blame.ignoreRevsFile .git-blame-ignore-revs
```
이후 로컬 `git blame`이 자동으로 파일 참조.

**방법 B — GitHub UI (자동)**:
GitHub은 저장소 루트의 `.git-blame-ignore-revs`를 **자동 인식**. 별도 설정 불필요. PR/파일 blame view 모두 반영.

### 3.3. 쓰면 안 되는 경우

- **정상 리팩터링 commit**: "저자 감추고 싶어서" 넣지 말 것. 오직 **의미론 변경 없는 mechanical transformation**만.
- **rebase로 hash가 바뀔 예정인 commit**: 확정된 hash를 써야 함.

### 3.4. 주석 원칙

파일에 commit 등록할 때 **왜 스킵하는지** 주석을 남긴다. 미래 AI 세션/신입이 파일 열고 "이게 뭐지?" 하지 않도록.

```
# mechanical: cargo fmt --all in PR #70 (BL-P2-063)
abc1234def5678...

# mechanical: clippy --fix bulk in PR #85
fedcba9876...
```

---

## 4. Commit SHA와 Merge 전략

### 4.1. Commit SHA 자체

**SHA** (Secure Hash Algorithm): Git이 각 commit에 부여하는 고유 지문.
```
8a3f9c2e8b4f5d6a7c9e1f2b3d4a5c6e7f8a9b1c
```
40자 hex 문자열. 앞 7자 (`8a3f9c2`)만 써도 대체로 고유 (대규모 repo에서는 collision 이론적 가능).

**생성 기반**: commit의 모든 내용(변경 파일 + 부모 commit SHA + author + message + timestamp)으로 계산되는 해시.
- 내용이 조금만 바뀌어도 SHA 완전히 달라짐
- 같은 변경을 다른 시간·환경에서 commit하면 다른 SHA
- 따라서 SHA는 "이 commit과 이 commit이 같은가"를 비교하는 무결성 앵커

### 4.2. PR 머지 방식 3가지

GitHub PR 머지 버튼은 3가지 모드를 제공:

| 방식 | 동작 | Feature branch SHA | Main에 생기는 SHA |
|------|------|-----------------|-----------------|
| **Merge commit** | 별도 merge commit 생성, feature commit 모두 보존 | 그대로 main에 편입 | 기존 + merge commit 1개 추가 |
| **Squash merge** | feature의 모든 commit을 1개로 합침 | **버려짐** (feature branch 이력만에 남음) | 새 SHA 1개 |
| **Rebase merge** | feature commit을 main 위에 하나씩 다시 찍음 | 버려지고 **새 SHA로 재생성** | N개 (feature branch 각 commit이 new SHA로 됨) |

### 4.3. Squash Merge의 특성

nexttui를 포함한 많은 프로젝트가 squash 선호:
- 장점: main 히스토리 깔끔, PR 단위 = 1 commit
- 단점: feature의 세부 commit 구조가 main에서 사라짐 (PR 페이지로만 열람)

**예시 — PR #68**:

Feature branch (`feature/runtime-context-switch`) 22개 commit:
```
c95526f feat: Unit 3b T1
6891ce3 feat: Unit 3b T2
2842f73 fix: C1+C2
...
```

Squash merge 후 main:
```
20cf637 feat: BL-P2-031 runtime context switching (PR1 — switch-core + Keystone adapters) (#68)
```

단 1개 commit. 원래 22개 SHA는 main에 **존재하지 않음**. main 기준으로 blame하면 모든 줄이 `20cf637`로 표시.

### 4.4. `.git-blame-ignore-revs`에 어떤 SHA를 써야 하나

**원칙**: main 쓰는 사람/도구가 실제로 `git blame`에서 만나는 SHA여야 한다.

**잘못된 예** (squash 방식에서 feature SHA):
```
# .git-blame-ignore-revs
b709b61    ← feature branch에만 존재. main에 없음
```
Git이 `b709b61`을 main 히스토리에서 찾다가 실패 → blame 스킵 **작동 안 함**.

**올바른 예** (squash merge 후 main의 SHA):
```
# .git-blame-ignore-revs
5f8a3c2    ← PR #70 squash 결과 main에 생긴 SHA
```
이 SHA가 main에 실존하므로 Git이 정상적으로 스킵.

### 4.5. 실무 순서 (squash 방식)

1. Feature branch에서 mechanical commit 작업 (예: `b709b61`)
2. PR 생성·리뷰·머지 (squash) → main에 새 SHA (예: `5f8a3c2`)
3. **머지 후 follow-up PR**:
   - main에서 새 branch
   - `.git-blame-ignore-revs`에 `5f8a3c2` 추가
   - 주석으로 원 PR·의도 기록
   - commit → PR → merge

Merge 방식이면 feature SHA 그대로 써도 OK, rebase 방식이면 새로 생성된 SHA 확인 후 기록.

---

## 5. Git Blame의 근본 목적 — "Code Archaeology"

### 5.1. 목적 재정의

`git blame` / `.git-blame-ignore-revs`는 어떤 범주 도구인가?

| 관점 | 이 도구들의 역할 |
|------|----------------|
| **보안** | ❌ 관계 없음. 모든 commit은 SHA로 무결성 보장. blame-ignore는 표시 층만 바꾸고 히스토리 불변 |
| **Traceability (규제/감사)** | ❌ 아님. 스킵되는 commit도 **그대로 저장·조회 가능**. 단지 blame UI에서 필터만 |
| **Attribution 품질** | ✅ "누가 진짜로 이 로직 썼나"를 정확히 복원 |
| **Debugging 속도** | ✅ 버그 추적 시 mechanical commit에서 시작점 안 잃음 |
| **Onboarding / 제도적 기억** | ✅ 신입이 코드 역사 읽을 때 노이즈 제거 |

### 5.2. 한 줄 결론

> **"Mechanical noise가 진짜 저자와 진짜 의도를 가리는 것을 방지해서, 미래의 어느 개발자든 코드 맥락을 빠르게 복원할 수 있게 한다"**

- 본질: **엔지니어링 생산성** + **institutional memory 보존**
- 부차적: **attribution 정확성**
- 간접적: 의도 보존 → 잘못된 제거 방지 → 보안·안정성 간접 기여
- **직접적 보안 도구 아님** (인증·권한·무결성과 무관)
- **직접적 compliance 도구 아님** (모든 정보 여전히 조회 가능)

범주는 **Developer Experience (DX)** / **코드 고고학**.

---

## 6. AI-Assisted Development에서의 특수성

### 6.1. AI가 bring하는 3가지 변화

#### 변화 A — Mechanical 변경의 **밀도**가 높아짐

| | Pre-AI | AI (claude-code + devflow) |
|--|--------|--------------------------|
| `cargo fmt --all` 빈도 | 드문 이벤트 (몇 달에 한 번) | 일상 (한 세션에 필요하면 바로) |
| Clippy bulk fix | 대규모 리팩터 PR 시 | routine cleanup |
| Codemod (rename, API shape 변경) | 큰 결정, 보류 많음 | AI가 파일별 일관성 확보하며 수행 |

예시: nexttui PR #70에서 `cargo clippy --fix` + `cargo fmt --all`로 **1세션에 100+ 파일 기계적 변경**.

→ Blame 오염 속도가 가속. `.git-blame-ignore-revs` 유지 자체가 routine work가 됨.

#### 변화 B — Commit 저자 identity의 복잡성

claude-code commit은 보통:
```
Author: Jay
Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
```

실체:
- Jay가 방향 지시 (PR 범위, 옵션 선택)
- Claude가 구체 구현 + 리뷰 + 리팩터
- Jay가 최종 승인

"누가 썼나?"의 답이 **애매**. Blame의 기존 의미론("인증된 1인 저자")이 흐려짐.

→ `.git-blame-ignore-revs`는 **"인간 저자" 대신 "의도 있는 commit vs mechanical commit"** 으로 관점을 전환. AI 협업의 정확한 정신모델.

#### 변화 C — 자동화된 리뷰 루프 commit의 증가

nexttui BL-P2-031 PR1 경과:
- T1 (feat: KeystoneRescopeAdapter)
- T2 (feat: ScopedAuthPort)
- P0 C1+C2 (fix: review finding)
- P0 S1 (fix: review finding)
- doc sync (docs)
- state sync (docs)

**한 기능에 6~7 commit.** AI 리뷰 루프(R1+Codex)가 돌면 review-finding commit이 추가됨.

→ Blame 측면에서 진짜 feature commit + review fix commit은 **모두 보고 싶음**, 반면 fmt commit은 **스킵하고 싶음**. 세분화된 분류 필요.

### 6.2. AI가 Blame을 읽는 쪽이기도 하다 — 핵심 포인트

claude-code 세션이 프로젝트 이해할 때 실제로 실행하는 것들:
```bash
git log --oneline origin/main..HEAD
git show <sha> --name-only
git blame <file>
```

**AI 세션은 컨텍스트 윈도우가 0에서 출발**. 프로젝트 맥락 복원을 위해 repo 메타데이터(commit, blame, log)를 **적극적으로 읽는다**.

**깨끗한 blame의 이득**:
- AI가 "왜 이 함수가 있지?" → `git blame <line>` → commit msg 읽음 → BL-P2-031 C1 fix 문맥 즉시 파악 → 해당 BL 관련 코드를 건드리지 않는 방향으로 작업 진행
- **Token 절약**: commit msg 1개 읽고 끝. 비싼 코드 전체 재해석 불필요

**오염된 blame의 손실**:
- AI가 rustfmt commit만 봄 → 맥락 모름 → 파일/폴더 전수 읽음 → **context window 낭비**
- 잘못 이해하면 **의도 파괴하는 변경** 가능 (예: `#[allow(..., reason = "BL-P2-060")]` 을 "왜 있지?" 하며 제거 → 회귀)

→ **AI가 코드 archaeology를 몇 배 자주 함**. Blame noise 비용이 **선형이 아닌 비례 증가**.

### 6.3. 구체적 우려 — AI가 commit 의도를 잘못 읽을 때

nexttui에 실존하는 리스크 예시:

**Case 1 — `#[allow(clippy::large_enum_variant, reason = "tracked by BL-P2-061")]`**

AI 세션이 이 코드를 blame 없이 만났을 때:
- "왜 이 `#[allow]`이 있지? 삭제해도 되나?"
- 맥락 없으면 "clippy 경고 무시하는 나쁜 패턴, 제거하자" 판단 가능
- 결과: BL-P2-061 벤치 없이 Box로 감싸서 성능 역행 유발

**깨끗한 blame + reason 필드**가 있으면:
- 해당 줄 blame → commit msg → "tracked by BL-P2-061 — pending bench-based boxing decision"
- "건드리지 말자" 즉시 판단

`reason` 필드는 AI 친화적 signal의 대표적 예.

---

## 7. devflow + AI 협업에서의 통합 전략

### 7.1. Commit 카테고리와 자동 분류

devflow는 이미 commit 카테고리 습관이 정착:

| Commit prefix | 성격 | Blame 전략 |
|---------------|------|-----------|
| `feat:` / `fix:` | 의도 있는 변경 | **Blame 유지** |
| `chore(clippy):` / `chore(fmt):` | AI 기계적 적용 | **Blame-ignore** |
| `chore(deps):` | 버전 bump | **Blame-ignore** |
| `docs:` | 의도 있는 문서 수정 | Blame 유지 |
| `refactor:` | 의도 있는 구조 변경 | Blame 유지 (주의) |
| `test:` | 테스트 추가 | Blame 유지 |

→ 규칙이 명확하면 **자동화 가능**.

### 7.2. 자동 유지 Hook 설계

devflow 플러그인의 `PostToolUse` 또는 `Stop` hook을 이용한 자동화:

```bash
# 예시: .claude/hooks/update-blame-ignore.sh
# 새 commit이 mechanical 카테고리면 .git-blame-ignore-revs에 자동 추가

latest=$(git log -1 --format="%H %s")
sha=$(echo "$latest" | awk '{print $1}')
msg=$(echo "$latest" | cut -d' ' -f2-)

# 머지 방식이 squash인 repo는 featurebranch sha를 바로 쓰면 안 됨.
# 대신 머지 후 자동 follow-up PR을 생성하는 방식을 권장.
# 여기서는 로컬 경험 개선 목적만 (선택적 ignoreRevsFile 설정 시).

if [[ "$msg" =~ ^chore\((fmt|clippy|deps|codemod)\) ]]; then
    echo "$sha  # $msg" >> .git-blame-ignore-revs
fi
```

**고려 사항**:
- Squash merge 방식이면 feature SHA를 ignore에 넣어도 main에는 무효. 머지 후 새 merge commit SHA를 등록하는 **follow-up 단계 필요**.
- 완전 자동화: GitHub Action으로 "mechanical prefix PR이 머지되면 머지 commit SHA를 `.git-blame-ignore-revs`에 자동 추가하는 PR 생성".

### 7.3. Commit Message 표준화로 AI 파싱 최적화

claude-code가 생성하는 commit message는 일관성이 높다. 이를 활용:

```
chore(fmt): BL-P2-063 T2.5 — cargo fmt --all auto-apply
...
Blame-Ignore: true                            # ← footer에 명시적 마킹
Co-Authored-By: Claude Opus 4.6 ...
```

**효과**:
- 사람 리뷰어: prefix로 판단
- 자동화 스크립트: `Blame-Ignore: true` 파싱해서 등록 대상으로 표시
- AI 세션: "이 commit은 의미 없음" 명시적 signal → 분석 스킵

### 7.4. `devflow-docs/audit.md`와 상호 보완

`audit.md`는 이미 **시간 순 append-only 기록**. blame이 "file:line → commit"이라면, audit는 "session → 작업 요약":

- blame 정확성 = "코드 archaeology"
- audit.md = "세션 archaeology"

AI가 복잡한 버그 추적 시:
1. `audit.md`로 "이 기능이 어떤 세션에서 만들어졌나" 찾음
2. 해당 세션의 commit을 `git show`로 읽음 (blame 깨끗해야 정확)
3. 관련 BL 문서로 이동

→ 세 겹의 메타데이터가 상호 보완. Blame 품질이 좋을수록 전체 retrieval 속도 개선.

### 7.5. AI 시대의 새 위험과 완화

#### Risk A — AI가 너무 많이 blame-ignore에 넣음

AI는 "귀찮음 비용"이 0이라 자동화 편향. 결과:
- 모든 refactor를 blame-ignore로 올림
- 3년 후 정상 commit도 안 보이는 반대 문제

**완화**: ignore 기준을 **엄격 문서화**. 위 7.1 표의 카테고리만 허용. AI에게 판단 기준을 명시 주입.

#### Risk B — Attribution 감사 혼선

Compliance 환경(SOC2, SOX 등)에서 "누가 이 코드 썼나" 증빙 필요:
- `Co-Authored-By Claude`의 법적 해석
- "기계적 변경을 스킵한 blame"이 감사관 시각에서 **은폐**로 오해될 수 있음

**완화**:
- `.git-blame-ignore-revs`와 별개로 **모든 commit은 보존** (이 점을 팀/감사관에 설명)
- `git log`로 여전히 모든 이력 접근 가능
- nexttui 같은 dev tool은 이 이슈 적음. Enterprise/금융/의료 도메인은 정책 검토 필요.

#### Risk C — Commit message 품질 저하

AI가 양산하는 commit은 포맷 일관되지만 **내용 깊이**가 들쑥날쑥. blame이 정확해도 commit msg가 허술하면 무용지물.

**완화**:
- devflow에서 commit msg 템플릿 강제
- **BL ID + 리뷰 근거 + 검증 명령** 필수 포함 (현재 nexttui 습관대로)

---

## 8. 정리 — Pre-AI vs claude-code + devflow 시대

| 축 | Pre-AI | claude-code + devflow |
|----|--------|---------------------|
| Mechanical 변경 빈도 | 희귀 | 일상 |
| `.git-blame-ignore-revs` 유지 비용 | 수동, 잊기 쉬움 | hook으로 자동화 가능 |
| Blame 읽는 주체 | 주로 human | human + AI 세션 (AI가 훨씬 빈번) |
| Attribution 의미 | 1인 저자 명확 | 인간-AI 공조, "의도 vs 기계"가 더 중요한 축 |
| ROI | 가끔 편리 | AI 세션 token 비용에 직접 영향 |

**한 줄 결론**:
> AI 시대에는 `.git-blame-ignore-revs`가 **"blame 선호"가 아니라 "AI 협업 인프라"**에 가깝다. 사람이 1년에 한 번 쓰는 도구가 아니라 **매 세션 AI가 코드 archaeology할 때의 signal/noise ratio**를 결정하는 요소.

---

## 9. 권장 사항 / 체크리스트

### 9.1. nexttui에 즉시 적용할 것

- [ ] PR #70 머지 후 **follow-up PR**:
  - 루트에 `.git-blame-ignore-revs` 추가
  - PR #70 머지 commit SHA 등록
  - 주석으로 BL ID + "mechanical: cargo fmt --all & cargo clippy --fix" 설명
- [ ] (선택) 로컬 `git config --local blame.ignoreRevsFile .git-blame-ignore-revs` 설정
  → GitHub blame은 자동 인식이지만 CLI 경험도 일치시키려면 필요

### 9.2. 중기 도입 고려 (backlog 후보)

- [ ] devflow 플러그인에 **auto-update blame-ignore hook** 추가
  - mechanical commit 감지 → blame-ignore 자동 갱신 PR 생성
  - Squash merge 정책이면 merge commit SHA로 작성 (feature SHA 아님)
- [ ] Commit message 템플릿에 `Blame-Ignore: true` footer 옵션 추가
- [ ] CI에서 `.git-blame-ignore-revs` 포맷·SHA 유효성 검증 (optional)

### 9.3. 팀/조직 차원 정책 (신규 도입 시)

- [ ] `Co-Authored-By Claude` 정책 문서화 (compliance 이슈 가능성 고려)
- [ ] Mechanical commit 판정 기준 (위 7.1 표)을 CONTRIBUTING.md에 명시
- [ ] AI 세션이 `.git-blame-ignore-revs`에 수동 추가 vs 자동화 승인 프로세스 구분

### 9.4. 하지 말 것

- ❌ 정상 리팩터 commit을 blame-ignore에 넣기 (저자 감추기 목적으로)
- ❌ 확정되지 않은 hash (rebase/squash 미완) 등록
- ❌ Module-level `#[allow(clippy::)]`로 경고 무력화 (item-level + reason 필수)
- ❌ 주석 없이 SHA만 나열 (미래 맥락 손실)

---

## 10. 참고 자료

### 공식 문서
- [`git-blame` man page](https://git-scm.com/docs/git-blame) — `--ignore-revs-file` 옵션 참조
- [GitHub docs: Viewing a file](https://docs.github.com/en/repositories/working-with-files/using-files/viewing-a-file) — `.git-blame-ignore-revs` 자동 인식

### 실제 적용 사례
- [`rust-lang/rust` .git-blame-ignore-revs](https://github.com/rust-lang/rust/blob/master/.git-blame-ignore-revs)
- [`psf/black` (Python formatter) 사용 권장 문서](https://black.readthedocs.io/en/stable/guides/introducing_black_to_your_project.html#avoiding-ruining-git-blame)

### nexttui 내부 참조
- `devflow-docs/backlog.md` BL-P2-063 (이 주제의 출발점)
- `devflow-docs/audit.md` (세션 level archaeology trail)
- `.claude/` 하위 (devflow 플러그인 설정, hook 위치)

---

## 부록 A — 현재 nexttui의 실제 수치

BL-P2-063 완료 시점(2026-04-15):
- `cargo clippy --lib --tests -- -D warnings`: 40 → 0 errors
- `cargo fmt --all -- --check`: fail → clean (83 files touched by T2.5)
- `cargo test --lib`: 1240 passed (회귀 0)
- CI workflow: 없음 → `.github/workflows/ci.yml` 신설 (4-gate: fmt / test / clippy / bin)
- `#[allow(clippy::)]` count: 0 → 2 (모두 item-level + reason + BL ID)

위 수치가 이 문서의 권고가 실제로 적용된 결과를 보여준다. 같은 패턴을 다른 프로젝트에 이식할 때 참고 가능.

---

**문서 버전**: 1.0 (2026-04-15 초판, BL-P2-063 PR #70 직후 작성)
**변경 기록**: 이 문서 자체도 진화하므로, 주요 수정이 있으면 부록 B로 변경 이력 추가할 것.
