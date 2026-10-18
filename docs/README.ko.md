[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Claude Code / Claude Desktop를 코딩 하네스로 활용하면서 추론은 서드파티 LLM API로 라우팅하고, 외부 모델을 Google Antigravity의 플래너 및 리뷰어로 사용하세요.**

Poverello Coding Tools(앱 표시 이름 **Poverello**)는 AI 기반 소프트웨어 개발을 위한 Windows 전용 컴패니언 애플리케이션입니다. 다섯 가지 보완적인 워크플로를 지원합니다:

1. **Claude Code / Claude Desktop용 3P 게이트웨이** — Claude의 저장소 탐색, 도구 사용, 파일 편집, 테스트 실행 기능을 유지하면서 추론을 서드파티 프로바이더로 라우팅합니다.
2. **Claude Code 전용 로컬 LLM (Ollama)** — 로컬 모델을 사용해 Claude Code를 오프라인으로 실행할 수 있습니다.
3. **Google Antigravity용 MCP 플래너 & 리뷰어** — `poverello/plan`, `poverello/revise`, `poverello/review` MCP 도구를 통해 구현 계획, 계획 수정 및 구현 후 리뷰를 외부 모델에 위임합니다.
4. **멀티 에이전트 개발 오케스트레이터** — 현재 지원되는 제한된 Planner–Reviewer 계획·검토 워크플로를 실행하고 정규화된 토큰 사용량과 검증된 비용 추정치를 기록합니다. 이 워크플로는 코드를 자동으로 구현하지 않으며, 전체 Auto-Converge는 v0.24.6에서 사용할 수 없습니다.
5. **Antigravity 규칙 관리** — Antigravity용 Poverello Direct Coding 규칙(`pov-direct.md`)을 설정, 미리보기 및 안전하게 동기화합니다.

> **새 소식:** 버전별 기능, 구현 세부 정보 및 변경 내역은 [NEWS.md(릴리스 노트)](../NEWS.md)를 참조하세요.

---

## 다섯 가지 주요 워크플로

### 1. 3P 게이트웨이를 활용한 Claude Code / Claude Desktop

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
```

- **하네스와 모델 분리**: Claude의 에이전트 도구는 유지하면서 추론을 서드파티 프로바이더로 라우팅합니다.
- **동적 멀티 프로필 라우팅**: GUI에서 활성 프로바이더, OpenRouter 프로필, 모델 라우트를 즉시 전환합니다.
- **설정 가이드**: [Claude Desktop / Cowork 3P 게이트웨이 설정](THIRD_PARTY_INFERENCE.md)

### 2. Claude Code 전용 로컬 LLM (Ollama)

Claude Code CLI를 로컬 Ollama 모델에 직접 연결할 수 있습니다. 이 경로는 3P Gateway 및 Antigravity MCP와 독립적으로 동작합니다.

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
- **Auto-Converge 안내**: 별도의 전체 Auto-Converge 워크플로는 v0.24.6에서 숨겨져 있으며 사용할 수 없습니다. 여기 설명된 지원 Orchestrator 워크플로와는 별개이며, Auto-Converge는 후속 버전으로 연기되었습니다.

실행 파일과 MCP 도구 식별자는 호환성을 위해 `anthro-bridge` 이름을 유지합니다.

Antigravity의 정식 명령은 `/pov-plan`, `/pov-revise`, `/pov-review`이며, 이전 `/anthro-*` 명령도 호환성을 위해 계속 지원됩니다.

**Antigravity 전역 명령어:**

- **`/pov-plan`** — 구현 계획 수립을 설정된 외부 모델에 위임합니다.
- **`/pov-revise`** — 새로운 피드백이나 제약 조건에 따라 기존 계획을 수정합니다.
- **`/pov-review`** — 커밋 전에 완료된 구현을 승인된 계획과 대조하여 리뷰하고, 명시적으로 READY / NOT READY 판정을 내립니다.

**권장 워크플로:**

```text
/pov-plan → 구현 및 테스트 → /pov-review → 커밋
```

---

### 4. 멀티 에이전트 개발 오케스트레이터 (제한된 워크플로 및 비용)

Orchestrator는 설정 가능한 프로필과 반복 제한에 따라 계획·검토 목적의 제한된 Planner–Reviewer 루프를 실행하며 코드를 자동 구현하지 않습니다. 전체 Auto-Converge는 v0.24.6에서 숨겨져 있고 사용할 수 없습니다. 새로운 기본 계획을 만들 때는 항상 사용자의 명시적 확인이 필요합니다. `%APPDATA%\Poverello\runs\`의 실행 로그에는 사용 가능한 정규화 사용량, 감사 메타데이터 및 검증된 비용 추정치가 기록됩니다. 추정치는 청구서가 아니며, 미지원 경로는 비용을 계산하지 않습니다.

### 5. Antigravity 규칙 관리 (Poverello Direct Coding)

GUI에서 `pov-direct.md` 규칙을 설정하고 미리보기 및 동기화할 수 있습니다. 사용자 지정 frontmatter와 주석은 보존되며, 검증된 적용·백업·롤백 보호를 제공합니다.

## 지원 프로바이더

| 프로바이더 | 연결 방식 | 지원 모델군 | 추론 제어 |
|---|---|---|---|
| **Anthropic** | 직접 API | Claude Opus 5.5, Sonnet 5.5, Haiku 4.5, Sonnet 3.7 | Adaptive thinking / 추론 강도 |
| **DeepSeek** | 직접 API | `deepseek-flash` (V4.1 Flash 1017), `deepseek-v4-pro` (V4 Pro 0813) | Normal / Thinking · Low / High / Max |
| **GLM (Z.ai)** | 직접 API | GLM-5.3, GLM-5.3-Flash, GLM-5.3-FlashX, GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **Kimi Code** | 직접 API | kimi-for-coding, kimi-for-coding-highspeed | 생각 모드 |
| **MiniMax** | 직접 API | MiniMax M3, M2.7 | 모델별 상이 |
| **Kimi / Moonshot** | 직접 API | Kimi K2.x, Kimi K3 | Thinking / Reasoning effort |
| **MiMo / Xiaomi** | 직접 API | MiMo V2.6 Flash, Pro, Pro-UltraSpeed (V2.5 하위 호환) | Normal / Thinking |
| **Ollama Local** | 로컬 루프백 | Gemma 4, Qwen 2.5, Llama 3.3, DeepSeek-R1 | Thinking (1024 예산) / Disabled |
| **OpenRouter** | 멀티 프로필 게이트웨이 | 아래 OpenRouter 섹션 참고 | 모델별 / 프로필별 상이 |
| **멀티 프로바이더 게이트웨이** | 사용자 지정 라우트 | 적격 프로바이더의 임의 조합 | 라우트별 제어 |

### Anthropic (직접 연결)

`ANTHROPIC_API_KEY`를 사용해 공식 API `https://api.anthropic.com`에 직접 연결합니다. 지원 모델은 `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5`, `claude-3-7-sonnet`, `claude-3-5-sonnet`, `claude-3-5-haiku`이며, 업스트림 모델의 adaptive thinking 및 추론 강도 제어를 지원합니다.

### GLM / Z.ai (직접 연결)

`ZAI_API_KEY`를 사용해 Z.ai 공식 Anthropic Messages 호환 API(`https://api.z.ai/api/anthropic/v1/messages`)에 직접 연결합니다. GLM-5.3 제품군은 추론 강도(`low`, `high`, `max`)를 선택할 수 있고, GLM-5.2는 Normal / Thinking 전환을 제공합니다.

### DeepSeek (직접 연결)

기본 제공 **Direct DeepSeek** 프리셋은 공식 안정 API ID `deepseek-flash` (현재 모델 DeepSeek-V4.1-Flash-1017)를 사용합니다: Opus 5 → Max · Sonnet 5 → High · Haiku 4.5 → Low.

- `deepseek-v4-pro` — API ID입니다. 현재 공식 가격 페이지에는 DeepSeek-V4-Pro-0813으로 기재되어 있습니다.
- 두 모델 모두 컨텍스트 1M, 최대 출력 384K 토큰을 지원합니다. Flash는 이미지 입력을 지원하지만 Pro는 지원하지 않습니다.
- 이전 API ID `deepseek-v4-flash` 및 `deepseek-v4-flash-vision-exp`는 DeepSeek에서 Flash 별칭으로 계속 수락되며 Flash 요금으로 청구됩니다.

### Kimi Code (직접 연결)

Moonshot Kimi와 별도로 운용되는 코딩 전문 API (`KIMI_CODE_API_KEY`):

- `kimi-for-coding` — 풀 품질 코딩 모델.
- `kimi-for-coding-highspeed` — 저지연 변형 모델.

### MiMo / Xiaomi (직접 연결)

기본 제공 **Direct MiMo** 프리셋 라우트:
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- 기본 모델: `mimo-v2.6-flash`

모델: `mimo-v2.6-flash`, `mimo-v2.6-pro`, `mimo-v2.6-pro-ultraspeed` (선택 가능). 세 모델 모두 100만 토큰 컨텍스트 윈도우와 네이티브 멀티모달 기능(텍스트, 이미지, 동영상)을 지원합니다.

**Normal / Thinking**: MiMo는 단순한 Normal/Thinking 전환 방식을 사용 — 추론 강도 레벨은 없습니다.

**V2.5 하위 호환성**: 저장된 `mimo-v2.5`, `mimo-v2.5-pro`, `mimo-v2.5-pro-ultraspeed` 라우트는 계속 정상 작동합니다. 변경되지 않은 레거시 기본값은 시작 시 자동으로 V2.6으로 마이그레이션됩니다.

### OpenRouter

여러 개의 명명된 프로필을 지원합니다. 단일 드롭다운에서 선택할 수 있는 현재 OpenAI 모델 카탈로그(15개 모델):

| 모델 ID | 표시 이름 |
|---|---|
| `openai/gpt-6.1-sol` | GPT-6.1 Sol |
| `openai/gpt-6.1-sol-pro` | GPT-6.1 Sol Pro |
| `openai/gpt-6-sol` | GPT-6 Sol |
| `openai/gpt-6-sol-pro` | GPT-6 Sol Pro |
| `openai/gpt-6-luna` | GPT-6 Luna |
| `openai/gpt-6-luna-pro` | GPT-6 Luna Pro |
| `openai/gpt-6-astra` | GPT-6 Astra |
| `openai/gpt-6-astra-pro` | GPT-6 Astra Pro |
| `openai/gpt-astra-latest` | GPT Astra Latest |
| `openai/gpt-5.6-sol` | GPT-5.6 Sol |
| `openai/gpt-5.6-sol-pro` | GPT-5.6 Sol Pro |
| `openai/gpt-5.6-terra` | GPT-5.6 Terra |
| `openai/gpt-5.6-terra-pro` | GPT-5.6 Terra Pro |
| `openai/gpt-5.6-luna` | GPT-5.6 Luna |
| `openai/gpt-5.6-luna-pro` | GPT-5.6 Luna Pro |

**GPT-6 Astra**: 컨텍스트 1.05M · 추론 강도: `low / medium / high / xhigh / max`.
**GPT-6 Astra Pro**: 컨텍스트 1.05M · 항상 활성화된 Pro 추론 (`reasoning.mode = pro`), 사용자 선택 불가.
**GPT Astra Latest**: 최신 Astra 패밀리 모델을 추적하는 별칭.

기본 제공 **OpenRouter: chatGPT** 프리셋: Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium.

추가 제공: **OpenRouter: Gemini** (Gemini 3.8 Flash · 추론 강도 `low / medium / high`), **OpenRouter: Poolside**, **OpenRouter: Tencent**, **OpenRouter: InclusionAI**, **OpenRouter: StepFun**.

---

## DeepSeek API 가격 (2026-10-18 확인)

| 모델 ID | 피크 입력 (캐시 미스 / 적중) | 비피크 입력 (캐시 미스 / 적중) | 피크 출력 | 비피크 출력 |
|---|---:|---:|---:|---:|
| `deepseek-flash` | \$0.30 (\$0.006) | \$0.15 (\$0.003) | \$1.20 | \$0.60 |
| `deepseek-v4-pro` | \$1.32 (\$0.044) | \$0.66 (\$0.022) | \$3.96 | \$1.98 |

DeepSeek 요금은 100만 토큰 기준입니다. 평일 UTC 01:00–04:00 및 06:00–10:00(중국 공휴일 제외)은 피크이며 주말과 공휴일은 하루 종일 비피크입니다. Poverello에는 확인된 2026년 중국 공휴일 및 대체 근무일 데이터가 포함되며, 다음 해 달력이 없으면 앱에서 알립니다. 다른 공급자 가격은 앱에 포함된 참고값이며 변경될 수 있습니다. Orchestrator 비용 추정은 현재 캐시 사용량이 명시적으로 0인 적격 Anthropic Direct 실행만 계산하며 DeepSeek 비용은 추정하지 않습니다. Pro의 경우 현재 공식 가격 페이지에 별도 요금이 게시되어 있고 2026년 9월 14일 이후 API 서비스가 계속되었다고 안내합니다. 반면 9월 10일 공지는 V4.1-Pro 출시 전까지 V4.1 Flash로 임시 라우팅하고 Flash 요금을 적용한다고 설명했습니다. Poverello는 요청별 라우팅이나 청구액을 확인할 수 없습니다. 표시된 값은 공개 요금이며 청구서나 보장이 아닙니다. [현재 가격 페이지](https://api-docs.deepseek.com/quick_start/pricing/)와 [9월 10일 공지](https://api-docs.deepseek.com/news/news260910/)를 참조하세요.

---

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
2. **MCP** 탭을 선택하고 **설정 > Antigravity > MCP Plan Settings**에서 모델을 설정합니다.
3. Antigravity의 MCP 설정에 `anthro-bridge.exe`를 `["--mcp-server"]` 인수와 함께 등록하거나, Poverello에서 **Configure Automatically**를 클릭합니다.
4. `/pov-plan`으로 계획을 수립하고, `/pov-revise`로 계획을 수정하며, `/pov-review`로 커밋 전 구현을 리뷰합니다.
5. 전체 [Antigravity MCP 설정 가이드](ANTIGRAVITY_MCP.ko.md)를 참고하세요.

### 워크플로 4: 멀티 에이전트 개발 오케스트레이터

1. 상단 탭에서 **Orchestrator**를 엽니다.
2. **Planner**와 **Reviewer** 역할의 프로필을 선택하거나 사용자 지정합니다.
3. 선택한 지원 Orchestrator 워크플로의 작업 목표와 반복 횟수 제한을 설정합니다.
4. 진행 상황, 감사 판정, 비용 추정 상태 및 `%APPDATA%\Poverello\runs\`의 실행 로그를 확인합니다.

### 워크플로 5: Antigravity 규칙 관리

1. 상단 탭에서 **Antigravity Rules**를 엽니다.
2. 규칙 모드를 선택하고 미리보기를 확인합니다.
3. **Apply Settings**를 클릭해 `pov-direct.md`를 안전하게 적용합니다.
4. 사용자 지정 주석, frontmatter 필드 및 백업은 보존됩니다.

---

## API 키

| 프로바이더 | 환경 변수 |
|---|---|
| Anthropic | `ANTHROPIC_API_KEY` |
| DeepSeek | `DEEPSEEK_API_KEY` |
| GLM / Z.ai | `ZAI_API_KEY` |
| Kimi Code | `KIMI_CODE_API_KEY` |
| Kimi / Moonshot | `MOONSHOT_API_KEY` |
| MiniMax | `MINIMAX_API_KEY` |
| MiMo / Xiaomi | `XIAOMI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

---

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
