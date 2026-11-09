[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Claude Code / Claude Desktop을 개발 환경으로 사용하고, 추론을 구성된 클라우드 또는 로컬 모델로 라우팅하며, Google Antigravity의 외부 모델과 규칙을 관리하세요.**

Poverello Coding Tools(앱 표시 이름 **Poverello**)는 AI 기반 소프트웨어 개발을 위한 Windows 전용 컴패니언 애플리케이션입니다. 다섯 가지 보완적인 워크플로를 지원합니다:

1. **Claude Code / Claude Desktop용 3P 게이트웨이** — Claude의 저장소 탐색, 도구 사용, 파일 편집, 테스트 실행 기능을 유지하면서 추론을 서드파티 프로바이더로 라우팅합니다.
2. **Claude Code 전용 로컬 LLM (Ollama)** — 로컬 모델을 사용해 API 비용 없이 완전한 오프라인으로 Claude Code를 실행할 수 있습니다.
3. **Google Antigravity용 MCP 플래너 & 리뷰어** — `poverello/plan`, `poverello/revise`, `poverello/review` MCP 도구를 통해 구현 계획, 계획 수정 및 구현 후 리뷰를 외부 모델에 위임합니다.
4. **Antigravity Autocode 및 Skills & Rules** — Antigravity Autocode에서 외부 리뷰, 코딩 동작 및 Nerf 설정을 관리하고 Settings > Antigravity Skills & Rules에서 Workflow Skills와 Global Rules를 관리합니다.
5. **Codex Autocode 및 Skills & Rules** — Codex Autocode에서 Codex Workflow Skills의 리뷰어를 설정하고, Settings > Codex Skills & Rules에서 세 가지 Codex Skill과 Poverello가 관리하는 AGENTS.md 영역을 관리합니다. 공용 CLI Tools는 Settings에서 별도로 관리합니다.

> [!TIP]
> **새 소식:** 버전별 기능, 구현 세부 정보 및 변경 내역은 [NEWS.md(릴리스 노트)](../NEWS.md)를 참조하세요.

---

## 다섯 가지 주요 워크플로

### 1. 3P 게이트웨이를 활용한 Claude Code / Claude Desktop

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
구성된 클라우드 또는 로컬 모델
```

- **하네스와 모델 분리**: Claude의 에이전트 도구는 유지하면서 추론을 서드파티 프로바이더로 라우팅합니다.
- **동적 멀티 프로필 라우팅**: GUI에서 활성 프로바이더, OpenRouter 프로필, 모델 라우트를 즉시 전환합니다.
- **설정 가이드**: [Claude Desktop / Cowork 3P 게이트웨이 설정](THIRD_PARTY_INFERENCE.md)

### 2. Claude Code 전용 로컬 LLM (Ollama)

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1 (Local On-Device)
```

- **무료·오프라인·프라이버시**: API 키, 사용 한도, 클라우드 데이터 전송 없이 Claude Code를 실행할 수 있습니다.
- **전용 라우팅 & 모델 자동 감지**: Gateway와 Ollama(`active_route: "ollama"`)를 전환하고 `/api/tags`에서 로컬 모델을 자동 감지합니다.
- **사고 예산 & 컨텍스트 튜닝**: 별칭별 사고 토큰 예산(1024 토큰) 및 컨텍스트 윈도우 재정의(`num_ctx`)를 완벽 지원합니다.
- **완전한 격리**: Claude Desktop, Cowork on 3P, Google Antigravity MCP는 클라우드 모델을 독립적으로 계속 사용합니다.
- **설정 가이드**: [Ollama Local 설정](OLLAMA_LOCAL.ko.md)

### 3. MCP 플래너 & 리뷰어를 활용한 Antigravity

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
설정된 외부 모델 (플래너 / 리뷰어)
    ↓
구현 계획 / 리뷰 결과
    ↓
Antigravity가 구독 기반 용량으로
구현 및 테스트 수행
```

- **계획과 실행의 분리**: 외부 모델이 고수준 계획 또는 리뷰 결과를 생성하고, Antigravity 구독 용량이 토큰 집약적인 코드 편집을 실행합니다.
- **실시간 GUI 설정**: 플래너 또는 리뷰어의 프로바이더, 모델, 추론 강도를 변경하면 다음 호출 시 즉시 적용됩니다.
- **설정 가이드**: [Google Antigravity + Poverello MCP 설정](ANTIGRAVITY_MCP.ko.md)

실행 파일과 MCP 도구 식별자는 호환성을 위해 `anthro-bridge` 이름을 유지합니다.

Antigravity의 정식 명령은 `/pov-mcp-plan`, `/pov-mcp-revise`, `/pov-mcp-review`이며, 이전 `/anthro-*` 명령도 호환성을 위해 계속 지원됩니다.

**Antigravity 전역 명령어:**

- **`/pov-mcp-plan`** — 구현 계획 수립을 설정된 외부 모델에 위임합니다.
- **`/pov-mcp-revise`** — 새로운 피드백이나 제약 조건에 따라 기존 계획을 수정합니다.
- **`/pov-mcp-review`** — 커밋 전에 완료된 구현을 승인된 계획과 대조하여 리뷰하고, 명시적으로 READY / NOT READY 판정을 내립니다.

Antigravity Workflow Skills: `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (**설정 > Antigravity Skills & Rules**에서 설치 및 복구, Reviewer Policy는 **Antigravity Autocode**에서 설정). 같은 명령 이름을 사용하는 Codex 네이티브 Skill은 Settings > Codex Skills & Rules에서 별도로 관리합니다.

**권장 워크플로:**

```text
/pov-mcp-plan → 구현 및 테스트 → /pov-mcp-review → 커밋
```

### 4. Antigravity Autocode 및 Skills & Rules 관리
```text
Antigravity Autocode tab
  • External Review policy / reviewer configuration
  • Antigravity Nerf Settings / Direct Coding

Settings > Antigravity Skills & Rules
  • Workflow Commands / repair
  • Global Rules

Settings > CLI Tools
  • Shared Node.js / npm and CLI management
```

- External Review와 Nerf 설정은 Antigravity Autocode에서 구성합니다. Settings > Antigravity Skills & Rules에서는 Workflow Skills와 Global Rules를 관리하며, 공용 CLI Tools는 Settings > CLI Tools에서 관리합니다.
### 5. Codex Autocode 및 Skills & Rules

Settings > Codex Skills & Rules에서 Codex Workflow Skills(/pov-plan-only, /pov-build-review, /pov-full-cycle)를 관리하고, Codex Autocode에서 리뷰어와 우선 리뷰어를 선택합니다. Poverello는 CODEX_HOME/AGENTS.md의 표시된 관리 영역만 수정하며 다른 사용자 지침은 보존합니다.

```text
Codex Workflow Skills
       ↓ configured reviewers
Codex 세션 내 리뷰 / 선택적 외부 DeepSeek Harness 또는 Gemini CLI
       ↓
Codex Autocode review settings

Settings > Codex Skills & Rules
  • /pov-plan-only, /pov-build-review, /pov-full-cycle
  • Poverello-managed instructions in CODEX_HOME/AGENTS.md
Settings > CLI Tools
  • Shared Codex CLI, Gemini CLI, DeepSeek Harness, Node.js / npm status
```


Gemini CLI는 Settings > CLI Tools에서 활성화할 수 있는 선택적 리뷰어입니다. 활성화만으로 리뷰어가 선택되거나 리뷰가 실행되지는 않습니다. Windows에서 읽기 전용 sandbox를 검증할 수 없으면 Gemini 리뷰를 사용할 수 없습니다. 설치 상태와 인증 상태는 별도로 표시됩니다. Node.js/npm은 요구 사항으로 감지되지만 자동 설치되지 않습니다. v0.26.1부터 비대화형 CLI 및 Node.js/npm 상태 확인은 콘솔 창을 표시하지 않으며, 탭을 전환할 때 짧은 캐시 결과를 재사용해 프로세스가 반복 실행되지 않도록 합니다. `0xC0000142`와 같은 프로세스 초기화 오류는 명시적으로 표시합니다. 사용자가 요청한 로그인은 계속 대화형 터미널에서 실행됩니다.

v0.26.2부터 우선 리뷰어는 유효한 판정이 서로 다를 때만 조정에 사용되며, 실패 또는 미완료 리뷰나 해결되지 않은 차단 지적을 무시할 수 없습니다. 진단 로그와 백업·확인 절차를 포함한 과거 정식 Codex Skill 템플릿의 안전한 업데이트를 제공합니다. DeepSeek Harness의 읽기 전용 설정과 도구 제한은 Windows 운영체제가 강제하는 파일 시스템 또는 네트워크 격리를 증명하지 않습니다. 해당 안전 경계를 검증하지 못하면 DeepSeek 리뷰는 사용할 수 없는 상태로 유지됩니다.

## 설치

[Releases](https://github.com/soheidon/poverello-coding-tools/releases) 페이지에서 최신 공개 Windows 설치 프로그램을 다운로드하여 실행하세요.

Windows 설치 프로그램은 8개 언어를 지원합니다. Poverello는 기존 애플리케이션 ID를 유지하며 Anthro Bridge에서 같은 설치 위치로 업그레이드하는 기능을 지원합니다. 첫 실행 시 `%APPDATA%\Anthro Bridge`(개발 빌드는 `%APPDATA%\Anthro Bridge Dev`)의 설정과 실행 데이터를 해당 Poverello 데이터 폴더로 복사하고 원래 폴더를 복구용 사본으로 보존하도록 설계되었습니다. 실제 Windows 환경에서의 설치, 데이터 마이그레이션, 업그레이드, 재설치, 취소 및 제거는 검증되지 않았으므로 업그레이드 전에 중요한 데이터를 백업하세요.

---

## 빠른 시작

### 워크플로 1: Claude Code / Claude Desktop용 3P 게이트웨이

1. Poverello **설정 > API 키**를 열고 원하는 프로바이더의 API 키를 설정합니다.
2. 대시보드에서 프로바이더 또는 OpenRouter 프로필을 선택합니다.
3. **Start Gateway**를 클릭합니다 (`http://127.0.0.1:4000`에서 실행).
4. Claude Code 또는 Claude Desktop을 연결합니다:
   - **Claude Code**: 설정에서 **Copy Claude Code launch command**를 클릭하고 PowerShell에 붙여넣습니다.
   - **Claude Desktop / Cowork**: [Claude Desktop 3P 설정 가이드](THIRD_PARTY_INFERENCE.md)를 따르세요.

### 워크플로 2: Claude Code 전용 로컬 LLM (Ollama)

1. Ollama가 로컬에서 실행 중인지 확인합니다 (`http://127.0.0.1:11434`).
2. Poverello 대시보드에서 **Ollama Local**을 선택하거나 **설정 > Claude Code / Local LLMs**에서 모델을 구성합니다.
3. **Copy Claude Code launch command**를 클릭하고 PowerShell에 붙여넣습니다.
4. Claude Code가 로컬 Ollama 인스턴스에 직접 연결됩니다.

### 워크플로 3: Google Antigravity용 MCP 플래너 & 리뷰어

1. Poverello에서 선택한 플래너/리뷰어 모델의 API 키를 설정합니다.
2. **MCP** 탭(또는 **설정 > Antigravity MCP**)을 선택하고 **설정 > Antigravity MCP > MCP Plan 상세 설정**에서 모델을 설정합니다.
3. Antigravity의 MCP 설정에 `anthro-bridge.exe`를 `["--mcp-server"]` 인수와 함께 `poverello` 키로 등록하거나, Poverello에서 **자동 설정**을 클릭합니다.
4. `/pov-mcp-plan`으로 계획을 수립하고, `/pov-mcp-revise`로 계획을 수정하며, `/pov-mcp-review`로 커밋 전 구현을 리뷰합니다.
5. 전체 [Antigravity MCP 설정 가이드](ANTIGRAVITY_MCP.ko.md)를 참고하세요.

### 워크플로 4: Antigravity Autocode 및 Skills & Rules
1. **Antigravity Autocode**에서 External Review와 Antigravity Nerf Settings를 구성합니다. 선택한 정책이 Workflow Skill의 리뷰어 호출을 제어합니다.
2. **Settings > Antigravity Skills & Rules**에서 Antigravity Workflow Skills와 Global Rules를 관리합니다. Skill 복구에는 확인이 필요하며 백업이 보존됩니다. Nerf 설정은 Antigravity Autocode에 있습니다. 공용 CLI Tools는 **Settings > CLI Tools**에서 관리합니다.
3. Auto-Converge는 숨김, 프로덕션 디스패치는 비활성화, Stage 2 격리는 미검증 상태입니다.
### 워크플로 5: Codex Autocode 및 Skills & Rules
1. Settings > CLI Tools에서 Node.js/npm 및 CLI 상태를 확인합니다. Poverello는 Node.js를 자동 설치하지 않습니다.
2. Settings > Codex Skills & Rules에서 /pov-plan-only, /pov-build-review, /pov-full-cycle과 CODEX_HOME/AGENTS.md의 Poverello 관리 영역을 관리합니다.
3. Codex Autocode에서 리뷰어와 우선 리뷰어를 선택합니다. Gemini CLI는 Settings > CLI Tools에서 활성화한 경우에만 표시되며, 활성화만으로 선택되지는 않습니다. Windows 읽기 전용 sandbox를 검증하지 못하면 Gemini 리뷰를 사용할 수 없습니다.

## 문서

- [릴리스 노트 (NEWS.md)](../NEWS.md) — 버전별 기능 및 구현 세부 정보
- [Claude Desktop / Cowork 3P 게이트웨이 설정](THIRD_PARTY_INFERENCE.md)
- [Google Antigravity + Poverello MCP 설정](ANTIGRAVITY_MCP.ko.md)
- [설정 레퍼런스 (`config.json`)](CONFIGURATION.md)
- [프로바이더 상세 정보 및 추론 제어](PROVIDERS.md)
- [개발 및 검증 가이드](DEVELOPMENT.md)

---

## 문제 해결

### 포트 4000이 이미 사용 중인 경우
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### 업그레이드 후 설정이 초기화되는 경우
마이그레이션이 실행될 수 있도록 애플리케이션을 재시작하세요. 설정은 `%APPDATA%\Poverello\config.json`에 저장됩니다.

### MCP 플래너 호출이 실패하는 경우
**MCP** 탭에서 선택한 프로바이더의 API 키가 설정되어 있는지, 또는 Windows 사용자 환경 변수로 내보내져 있는지 확인하세요 (예: `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`). MCP를 사용할 때는 3P 게이트웨이가 실행 중일 필요가 없습니다.

---

## 라이선스

MIT 라이선스. [LICENSE](../LICENSE) 참고.
