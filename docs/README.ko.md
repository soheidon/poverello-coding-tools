[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Claude Code / Claude Desktop를 코딩 하네스로 활용하면서 추론은 서드파티 LLM API로 라우팅하고, 외부 모델을 Google Antigravity의 플래너 및 리뷰어로 사용하세요.**

Poverello Coding Tools(앱 표시 이름 **Poverello**)는 AI 기반 소프트웨어 개발을 위한 Windows 전용 컴패니언 애플리케이션입니다. 세 가지 보완적인 워크플로를 지원합니다:

1. **Claude Code / Claude Desktop용 3P 게이트웨이** — Claude의 저장소 탐색, 도구 사용, 파일 편집, 테스트 실행 기능을 유지하면서 추론을 서드파티 프로바이더로 라우팅합니다.
2. **Claude Code 전용 로컬 LLM (Ollama)** — 로컬 모델을 사용해 Claude Code를 오프라인으로 실행할 수 있습니다.
3. **Google Antigravity용 MCP 플래너 & 리뷰어** — `anthro-bridge/plan` 및 `anthro-bridge/review` MCP 도구를 통해 구현 계획 수립과 구현 후 리뷰를 외부 모델에 위임합니다.

---

## 세 가지 주요 워크플로

### 1. 3P 게이트웨이를 활용한 Claude Code / Claude Desktop

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo
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
- **선택형 Planner–Reviewer Auto-Convergence**: Orchestrator에서 제한된 수렴 워크플로를 활성화하면 계획 후보를 생성하고 독립적으로 검토합니다. 범위 내 변경으로 승인된 내용은 자동으로 추가될 수 있지만, 새 기본 계획은 반드시 사용자의 명시적 확인이 필요합니다. 실제 모델 동작은 공급자와 어댑터에 따라 다르며 모든 조합을 충분히 스모크 테스트하지는 않았습니다.

실행 파일과 MCP 도구 식별자는 호환성을 위해 `anthro-bridge` 이름을 유지합니다.

**Antigravity 전역 명령어:**

- **`/anthro-plan`** — 구현 계획 수립을 설정된 외부 모델에 위임합니다.
- **`/anthro-revise`** — 새로운 피드백이나 제약 조건에 따라 기존 계획을 수정합니다.
- **`/anthro-review`** — 커밋 전에 완료된 구현을 승인된 계획과 대조하여 리뷰하고, 명시적으로 READY / NOT READY 판정을 내립니다.

**권장 워크플로:**

```text
/anthro-plan → 구현 및 테스트 → /anthro-review → 커밋
```

---

## 지원 프로바이더

| 프로바이더 | 연결 방식 | 지원 모델군 | 추론 제어 |
|---|---|---|---|
| **DeepSeek** | 직접 API | DeepSeek V4.1 Flash, V4 Pro 0813 | Normal / Low / High / Max |
| **Kimi Code** | 직접 API | kimi-for-coding, kimi-for-coding-highspeed | 생각 모드 |
| **MiniMax** | 직접 API | MiniMax M3, M2.7 | 모델별 상이 |
| **Kimi / Moonshot** | 직접 API | Kimi K2.x, Kimi K3 | Thinking / Reasoning effort |
| **MiMo / Xiaomi** | 직접 API | MiMo V2.6 Flash, Pro, Pro-UltraSpeed (V2.5 하위 호환) | Normal / Thinking |
| **OpenRouter** | 멀티 프로필 게이트웨이 | 아래 OpenRouter 섹션 참고 | 모델별 / 프로필별 상이 |

### DeepSeek (직접 연결)

기본 제공 **Direct DeepSeek** 프리셋: Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low.

- `deepseek-v4.1-flash` — 현재 최신 추론 모델 (입력 $0.27 / 1M · 출력 $1.10 / 1M).
- `deepseek-v4-pro-0813` — 확장 추론 없이 고품질 기준 성능 제공 (입력 $0.27 / 1M · 출력 $1.10 / 1M).

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

여러 개의 명명된 프로필을 지원합니다. 전체 OpenAI 모델 카탈로그 (단일 드롭다운):

| 모델 ID | 표시 이름 |
|---|---|
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

## 모델 가격 (v0.23.0 기준)

| 모델 | 입력 | 출력 |
|---|---|---|
| DeepSeek V4.1 Flash | \$0.27 / 1M | \$1.10 / 1M |
| DeepSeek V4 Pro 0813 | \$0.27 / 1M | \$1.10 / 1M |
| GPT-6 Astra / Astra Pro / Astra Latest | \$10 / 1M | \$50 / 1M |
| GPT-5.6 Sol / Terra / Luna | \$5 / 1M | \$25 / 1M |
| GPT-5.6 Sol Pro / Terra Pro / Luna Pro | \$5 / 1M | \$25 / 1M |
| Gemini 3.8 Flash (OpenRouter) | \$0.75 / 1M | \$3.75 / 1M |
| MiMo-V2.6-Flash | \$0.14 / 1M | \$0.28 / 1M |
| MiMo-V2.6-Pro | \$0.435 / 1M | \$0.87 / 1M |
| MiMo-V2.6-Pro-UltraSpeed | \$4.35 / 1M | \$8.70 / 1M |

---

## 설치

[Releases](https://github.com/soheidon/poverello-coding-tools/releases) 페이지에서 최신 공개 Windows 설치 프로그램을 다운로드하여 실행하세요.

Windows 설치 프로그램은 8개 언어를 지원합니다. Poverello v0.24.1은 기존 애플리케이션 ID를 유지하며 Anthro Bridge에서 같은 설치 위치로 업그레이드하는 경로를 포함합니다. 첫 실행 시 `%APPDATA%\Anthro Bridge`(개발 빌드는 `%APPDATA%\Anthro Bridge Dev`)의 설정과 실행 데이터를 해당 Poverello 데이터 폴더로 복사하고 원래 폴더를 복구용 사본으로 보존하도록 설계되었습니다. 실제 Windows 환경에서의 설치, 데이터 마이그레이션, 업그레이드, 재설치, 취소 및 제거는 검증되지 않았으므로 업그레이드 전에 중요한 데이터를 백업하세요.

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
4. `/anthro-plan`으로 계획을 수립하고, `/anthro-revise`로 계획을 수정하며, `/anthro-review`로 커밋 전 구현을 리뷰합니다.
5. 전체 [Antigravity MCP 설정 가이드](ANTIGRAVITY_MCP.ko.md)를 참고하세요.

---

## API 키

| 프로바이더 | 환경 변수 |
|---|---|
| DeepSeek | `DEEPSEEK_API_KEY` |
| Kimi Code | `KIMI_CODE_API_KEY` |
| Kimi / Moonshot | `MOONSHOT_API_KEY` |
| MiniMax | `MINIMAX_API_KEY` |
| MiMo / Xiaomi | `XIAOMI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

---

## 문서

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
