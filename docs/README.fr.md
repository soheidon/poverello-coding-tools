[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

> **Transition vers v0.25.2 :** Auto-Converge est conservé dans le code source, mais masqué dans la navigation normale de l’application. Le dispatch de production reste désactivé et l’isolation de Stage 2 n’est pas vérifiée. Cela ne signifie pas que la version est prête à être publiée.

**Utilisez Claude Code / Claude Desktop comme environnement de développement, routez les inférences vers des API LLM tierces, orchestrez des flux multi-agents bornés avec suivi des estimations de coût et gérez de façon centralisée les modèles externes et règles pour Google Antigravity.**

Poverello Coding Tools (affiché dans l’application sous le nom **Poverello**) est une application Windows complémentaire pour le développement logiciel assisté par IA. Elle prend en charge cinq flux de travail complémentaires :

1. **Passerelle 3P pour Claude Code / Claude Desktop** — Conservez l'exploration de dépôts, l'utilisation d'outils, l'édition de fichiers et l'exécution de tests de Claude tout en routant les inférences vers des fournisseurs tiers.
2. **LLM local dédié à Claude Code (Ollama)** — Exécutez Claude Code hors ligne avec des modèles locaux sans aucun coût d'API.
3. **Planificateur & Réviseur MCP pour Google Antigravity** — Déléguez la planification, la révision des plans et la revue post-implémentation à des modèles externes via les outils MCP `poverello/plan`, `poverello/revise` et `poverello/review`.
4. **Orchestrateur de développement multi-agents** — Exécutez les workflows Planner–Reviewer limités actuellement pris en charge et suivez l'utilisation normalisée des jetons ainsi que les estimations de coût vérifiées.
5. **Gestion des Antigravity Skills & Règles** — Configurez de manière centralisée la politique Reviewer Policy pour les compétences de flux de travail (Codex CLI seul vs revue composite Codex CLI + DeepSeek Harness), gérez les modèles standard protégés de règles globales (`codex-cli.md`, `deepseek-harness.md`), ajustez le comportement Direct Coding (`pov-direct.md`) et gérez les compétences Nerf.

> [!TIP]
> **Nouveautés :** Consultez [NEWS.md (notes de version)](../NEWS.md) pour les fonctionnalités, détails d'implémentation et changements de chaque version.

---

## Cinq flux de travail principaux

### 1. Claude Code / Claude Desktop avec la passerelle 3P

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
```

- **Séparation environnement & modèle** : Conservez les outils agentiques de Claude tout en routant les inférences vers des fournisseurs tiers.
- **Routage multi-profils dynamique** : Changez de fournisseur actif, de profil OpenRouter et de route de modèle depuis l'interface graphique.
- **Guide de configuration** : [Configuration de la passerelle 3P pour Claude Desktop / Cowork](THIRD_PARTY_INFERENCE.fr.md)

### 2. LLM locaux dédiés pour Claude Code (Ollama)

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1 (Local sur l'appareil)
```

- **Gratuit, hors ligne et privé**: Exécutez Claude Code sans clé API, sans limites d'utilisation ni transfert de données vers le cloud.
- **Routage dédié et découverte**: Route Claude Code séparée (`active_route: "ollama"`) avec découverte automatique des modèles via `/api/tags`.
- **Mode réflexion et contexte**: Prise en charge des budgets de réflexion (1024 tokens) et du contexte (`r`num_ctx`) par alias.
- **Isolation totale**: Claude Desktop, Cowork on 3P et Google Antigravity MCP continuent d'utiliser les modèles cloud en toute indépendance.
- **Guide de configuration**: [Guide Ollama Local](OLLAMA_LOCAL.fr.md)

### 3. Antigravity avec planificateur et réviseur MCP

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Modèle externe configuré (planificateur / réviseur)
    ↓
Plan d'implémentation / Verdict de révision
    ↓
Antigravity implémente et teste
en utilisant la capacité d'abonnement
```

- **Séparation planification / exécution** : Les modèles externes génèrent le plan de haut niveau ou le verdict de révision ; la capacité d'abonnement Antigravity exécute les modifications de code gourmandes en tokens.
- **Configuration GUI en temps réel** : Le changement de fournisseur, de modèle ou d'effort de raisonnement du planificateur ou du réviseur prend effet immédiatement à la prochaine invocation.
- **Guide de configuration** : [Configuration de Google Antigravity + Poverello MCP](ANTIGRAVITY_MCP.fr.md)

L’exécutable et les identifiants des outils MCP conservent le nom `anthro-bridge` pour assurer la compatibilité.

Les commandes Antigravity canoniques sont `/pov-mcp-plan`, `/pov-mcp-revise` et `/pov-mcp-review` ; les anciennes commandes `/anthro-*` restent disponibles pour assurer la compatibilité.

**Commandes globales Antigravity :**

- **`/pov-mcp-plan`** — Délègue la planification d'implémentation au modèle externe configuré.
- **`/pov-mcp-revise`** — Révise un plan existant en fonction de nouveaux retours ou contraintes.
- **`/pov-mcp-review`** — Révise une implémentation terminée par rapport au plan approuvé avant le commit, avec des verdicts explicites READY / NOT READY.

Workflows autonomes : `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (gérés sous **Paramètres > Antigravity Skills & Rules** avec synchronisation de la Reviewer Policy).

**Flux de travail recommandé :**

```text
/pov-mcp-plan → Implémentation & Tests → /pov-mcp-review → Commit
```

### 4. Orchestrateur de développement multi-agents (workflows limités et coûts)

```text
Tâche / Spécification
        ↓
+-------------------------------------------------------------+
| Poverello Orchestrator                                      |
|                                                             |
|  [Profil Planner]  ⇄  [Profil Reviewer]                     |
|  (Générer / Réviser)    (Vérifier / Verdicts)               |
|                                                             |
|  • Boucle plan/revue bornée (limite de tours)               |
|  • Journaux complets (%APPDATA%\Poverello\runs\*.json)      |
|  • Estimations en nano-dollars entiers & vérification       |
|  • Badges à sûreté intégrée (Complete / Partial / N/A)      |
+-------------------------------------------------------------+
        ↓
Plan Approuvé / Journal d'Audit
        ↓
Implémentation séparée et contrôlée par l'utilisateur
```

- **Workflows d'orchestration bornés** : Configurez les profils d'agents et les plafonds d'itération pour les flux pris en charge.
- **Invariants de sécurité et validation humaine** : Les actions nécessitant une approbation respectent des points de contrôle stricts ; tout nouveau plan principal requiert une confirmation humaine explicite.
- **Journaux d'exécution vérifiables** : `%APPDATA%\Poverello\runs\<run_id>.json` consigne la configuration, le prompt, les métadonnées, l'utilisation normalisée et les estimations de coût.
- **Utilisation normalisée et estimations vérifiées** : Calculs en nano-dollars entiers (`u64 amount_nanos`) sans approximations flottantes, avec rejet sécurisé en cas de tarification inconnue.

### 5. Gestion des Antigravity Skills & Règles

```text
Poverello GUI (Paramètres > Antigravity Skills & Rules)
          ↓ (Reviewer Policy / Protection de Modèles / Rollback)
%USERPROFILE%\.gemini\config\rules\
  • codex-cli.md / deepseek-harness.md (Modèles Standard de Règles Globales)
  • pov-direct.md (Comportement Direct Coding)
~/.gemini/antigravity/skills/
  • pov-full-cycle, pov-plan-only, pov-build-review (Compétences Synchronisées)
```

- **Politique Reviewer Policy centralisée** : Basculez instantanément les compétences autonomes (`/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review`) entre `codex_only` (Codex CLI indépendant seul) et `codex_plus_deepseek` (revue composite Codex CLI + DeepSeek Harness) avec vérification SHA-256 et synchronisation automatique.
- **Modèles protégés de règles globales** : Installation en un clic et suppression sécurisée des modèles standard (`codex-cli.md`, `deepseek-harness.md`) vérifiés par SHA-256, avec protection de plateforme (`can_remove`) sous Windows.
- **Contrôle de portée et de direction (Poverello Direct Coding)** : Configurez des règles légères pour éviter toute expansion non sollicitée du périmètre ou refactorisation excessive.
- **Intégrité de fichier unique et application sécurisée** : Protégé par vérification préalable SHA-256, détection des conflits de suppression et restauration vérifiée.

## Fournisseurs pris en charge

| Fournisseur | Connexion | Familles prises en charge | Contrôles de raisonnement |
|---|---|---|---|
| **Anthropic** | API directe | Claude Opus 5.5, Sonnet 5.5, Haiku 4.5, Sonnet 3.7 | Thinking adaptatif / effort |
| **DeepSeek** | API directe | `deepseek-flash` (V4.1 Flash 1017), `deepseek-v4-pro` (V4 Pro 0813) | Normal / Thinking · Low / High / Max |
| **GLM (Z.ai)** | API directe | GLM-5.3, GLM-5.3-Flash, GLM-5.3-FlashX, GLM-5.2 | GLM-5.3 : Low / High / Max · GLM-5.2 : Normal / Thinking |
| **Kimi Code** | API directe | kimi-for-coding, kimi-for-coding-highspeed | Mode thinking |
| **MiniMax** | API directe | MiniMax M3, M2.7 | Spécifique au modèle |
| **Kimi / Moonshot** | API directe | Kimi K2.x, Kimi K3 | Thinking / Effort de raisonnement |
| **MiMo / Xiaomi** | API directe | MiMo V2.6 Flash, Pro, Pro-UltraSpeed (rétrocompatible V2.5) | Normal / Thinking |
| **Ollama Local** | Boucle locale | Gemma 4, Qwen 2.5, Llama 3.3, DeepSeek-R1 | Thinking (budget 1024) / Disabled |
| **OpenRouter** | Passerelle multi-profils | Voir la section OpenRouter ci-dessous | Spécifique au modèle / au profil |
| **Passerelle multi-fournisseurs** | Routes personnalisées | Toute combinaison de fournisseurs admissibles | Contrôle par route |

### Anthropic (Direct)

Accès direct officiel via `https://api.anthropic.com` avec `ANTHROPIC_API_KEY`. Modèles pris en charge : `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5`, `claude-3-7-sonnet`, `claude-3-5-sonnet` et `claude-3-5-haiku`. Les contrôles natifs d'adaptive thinking et d'effort de raisonnement sont pris en charge.

### GLM / Z.ai (Direct)

Accès direct à l'API officielle Z.ai compatible avec Anthropic Messages (`https://api.z.ai/api/anthropic/v1/messages`) avec `ZAI_API_KEY`. La famille GLM-5.3 permet de choisir l'effort (`low`, `high`, `max`) ; GLM-5.2 propose un sélecteur Normal / Thinking.

### DeepSeek (Direct)

Le préréglage intégré **Direct DeepSeek** route : Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low.

- `deepseek-flash` — Identifiant API canonique et stable de DeepSeek-V4.1-Flash-1017 ; contexte de 1M et jusqu’à 384K tokens en sortie.
- `deepseek-v4-pro` — Identifiant API ; la page officielle des tarifs actuelle le répertorie comme DeepSeek-V4-Pro-0813. Contexte de 1M et jusqu’à 384K tokens en sortie.
- Flash prend en charge la vision, contrairement à Pro. DeepSeek accepte encore les anciens identifiants `deepseek-v4-flash` et `deepseek-v4-flash-vision-exp` comme alias Flash et les facture au tarif Flash.

### Kimi Code (Direct)

API spécialisée dans le code (`KIMI_CODE_API_KEY`), distincte de Moonshot Kimi :

- `kimi-for-coding` — Modèle de codage pleine qualité.
- `kimi-for-coding-highspeed` — Variante à faible latence.

### MiMo / Xiaomi (Direct)

Préréglages intégrés **Direct MiMo** :
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- Modèle par défaut : `mimo-v2.6-flash`

Modèles : `mimo-v2.6-flash`, `mimo-v2.6-pro`, `mimo-v2.6-pro-ultraspeed` (sélectionnables). Les trois prennent en charge une fenêtre de contexte de 1 million de tokens et des capacités multimodales natives (texte, image, vidéo).

**Normal / Thinking** : MiMo utilise un simple commutateur Normal/Thinking — sans niveaux d'effort de raisonnement.

**Rétrocompatibilité V2.5** : Les routes `mimo-v2.5`, `mimo-v2.5-pro` et `mimo-v2.5-pro-ultraspeed` enregistrées sont préservées et continuent de fonctionner. Les anciens paramètres par défaut non modifiés migrent automatiquement vers V2.6 au démarrage.

### LLM local dédié (Ollama Local)

Backend d’inférence locale **dédié à Claude Code CLI** :
- Point de terminaison : `http://127.0.0.1:11434/v1` (boucle locale du protocole Anthropic Messages natif)
- Découverte dynamique des modèles locaux installés via `/api/tags`
- Budgets de réflexion (1024 tokens) et fenêtres de contexte (`r`num_ctx`) configurables par alias
- Voir le [Guide Ollama Local](OLLAMA_LOCAL.fr.md) pour la configuration détaillée

### OpenRouter

Prend en charge plusieurs profils nommés. Catalogue actuel de modèles OpenAI dans la liste déroulante unique (15 modèles) :

| ID du modèle | Nom d'affichage |
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

**GPT-6 Astra** : contexte de 1,05M · effort de raisonnement : `low / medium / high / xhigh / max`.
**GPT-6 Astra Pro** : contexte de 1,05M · raisonnement Pro toujours activé (`reasoning.mode = pro`), effort non sélectionnable par l'utilisateur.
**GPT Astra Latest** : alias suivant le dernier modèle de la famille Astra.

Préréglage intégré **OpenRouter: chatGPT** : Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium.

Également disponibles : **OpenRouter: Gemini** (Gemini 3.8 Flash · effort de raisonnement `low / medium / high`), **OpenRouter: Poolside**, **OpenRouter: Tencent**, **OpenRouter: InclusionAI**, **OpenRouter: StepFun**.

---

## Tarifs de l’API DeepSeek (vérifiés le 19-10-2026)

| ID du modèle | Entrée pointe (cache miss / hit) | Entrée heures creuses (cache miss / hit) | Sortie pointe | Sortie heures creuses |
|---|---:|---:|---:|---:|
| `deepseek-flash` | \$0.30 (\$0.006) | \$0.15 (\$0.003) | \$1.20 | \$0.60 |
| `deepseek-v4-pro` | \$1.32 (\$0.044) | \$0.66 (\$0.022) | \$3.96 | \$1.98 |

Tous les tarifs DeepSeek sont par million de tokens. Les heures de pointe sont de 01:00 à 04:00 et de 06:00 à 10:00 UTC en semaine, hors jours fériés chinois ; les week-ends et jours fériés sont entièrement en heures creuses. Poverello inclut les jours fériés chinois et jours travaillés compensatoires vérifiés pour 2026 et signale l’absence du calendrier de l’année suivante. Les tarifs des autres fournisseurs sont des références affichées dans l’application et peuvent changer. L’estimateur de coûts Orchestrator ne chiffre actuellement que les appels Anthropic Direct admissibles avec une utilisation du cache explicitement nulle ; il n’estime pas les coûts DeepSeek. Pour Pro, la page tarifaire actuelle publie des tarifs distincts et confirme la poursuite du service API après le 14 septembre 2026 ; toutefois, une annonce du 10 septembre décrivait un routage temporaire vers V4.1 Flash au tarif Flash jusqu’au lancement de V4.1-Pro. Poverello ne peut pas vérifier le routage ni la facturation de chaque requête ; les tarifs affichés sont ceux publiés, pas une facture ni une garantie. Voir les [tarifs actuels](https://api-docs.deepseek.com/quick_start/pricing/) et l’[annonce du 10 septembre](https://api-docs.deepseek.com/news/news260910/).

---

## Installation

Téléchargez le dernier installateur Windows actuellement publié depuis la page [Releases](https://github.com/soheidon/poverello-coding-tools/releases) et exécutez-le.

L’installateur Windows prend en charge 8 langues. Poverello conserve l’identité existante de l’application et prend en charge la mise à niveau sur place depuis Anthro Bridge. Au premier lancement, il est conçu pour copier les paramètres et données d’exécution de `%APPDATA%\Anthro Bridge` (ou `%APPDATA%\Anthro Bridge Dev` pour les versions de développement) vers le dossier Poverello correspondant, tout en conservant le dossier d’origine comme copie de récupération. L’installation, la migration, la mise à niveau, la réinstallation, l’annulation et la désinstallation réelles sous Windows n’ont pas été vérifiées ; sauvegardez les données importantes avant la mise à niveau.

---

## Démarrage rapide

### Flux de travail 1 : Passerelle 3P pour Claude Code / Claude Desktop

1. Ouvrez Poverello **Paramètres > Clé API** et configurez une clé API pour le fournisseur souhaité.
2. Sélectionnez votre fournisseur ou profil OpenRouter sur le tableau de bord.
3. Cliquez sur **Start Gateway** (s'exécute sur `http://127.0.0.1:4000`).
4. Connectez Claude Code ou Claude Desktop :
   - **Claude Code** : Cliquez sur **Copy Claude Code launch command** dans les Paramètres et collez la commande dans PowerShell.
   - **Claude Desktop / Cowork** : Suivez le [Guide de configuration 3P pour Claude Desktop](THIRD_PARTY_INFERENCE.fr.md)

### Flux de travail 2 : LLM locaux dédiés pour Claude Code (Ollama)

1. Vérifiez qu’Ollama fonctionne localement (`http://127.0.0.1:11434`).
2. Sélectionnez **Ollama Local** dans le tableau de bord Poverello ou configurez les modèles dans **Paramètres > Claude Code / Local LLMs**.
3. Cliquez sur **Copy Claude Code launch command** et collez la commande dans PowerShell.
4. Claude Code se connecte directement à votre instance Ollama locale.

### Flux de travail 3 : Planificateur & Réviseur MCP pour Google Antigravity

1. Configurez une clé API pour le modèle planificateur/réviseur choisi dans Poverello.
2. Sélectionnez l'onglet **MCP** (ou **Paramètres > Antigravity MCP**) et configurez votre modèle dans **Paramètres > Antigravity MCP > Paramètres détaillés du plan MCP**.
3. Enregistrez `anthro-bridge.exe` avec `["--mcp-server"]` sous la clé de serveur `poverello` dans la configuration MCP d'Antigravity (ou cliquez sur **Configurer automatiquement** dans Poverello).
4. Utilisez `/pov-mcp-plan` pour concevoir des plans, `/pov-mcp-revise` pour mettre à jour des plans, et `/pov-mcp-review` pour réviser les implémentations avant le commit.
5. Suivez le [Guide complet de configuration du MCP Antigravity](ANTIGRAVITY_MCP.fr.md).

### Flux de travail 4 : Orchestrateur de développement multi-agents

**Note d'accès :** Dans la version 0.25.2, l’onglet Orchestrator est masqué dans la navigation habituelle tout en conservant son implémentation sous-jacente. Lancez les workflows autonomes dans Antigravity avec `/pov-full-cycle`, `/pov-plan-only` ou `/pov-build-review`.

### Flux de travail 5 : Gestion des Antigravity Skills & Règles

1. Ouvrez **Paramètres > Antigravity Skills & Rules**.
2. Sélectionnez votre politique Reviewer Policy (`codex_only` ou `codex_plus_deepseek`) pour synchroniser les compétences.
3. Installez ou gérez les modèles standard protégés de règles globales (`codex-cli.md`, `deepseek-harness.md`).
4. Configurez les paramètres Direct Coding et cliquez sur **Appliquer** pour synchroniser `pov-direct.md`.

## Clés API

| Fournisseur | Variable d'environnement |
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

## Documentation

- [Notes de version (NEWS.md)](../NEWS.md) — Fonctionnalités et détails d'implémentation par version
- [Configuration de la passerelle 3P pour Claude Desktop / Cowork](THIRD_PARTY_INFERENCE.fr.md)
- [Ollama Local](OLLAMA_LOCAL.fr.md)
- [Configuration de Google Antigravity + Poverello MCP](ANTIGRAVITY_MCP.fr.md)
- [Référence de configuration (`config.json`)](CONFIGURATION.md)
- [Détails des fournisseurs & contrôles de raisonnement](PROVIDERS.md)
- [Guide de développement & de vérification](DEVELOPMENT.md)

---

## Dépannage

### Le port 4000 est déjà utilisé
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### Les paramètres reviennent à leur état précédent après une mise à niveau
Redémarrez l'application pour que les migrations puissent s'exécuter. La configuration est stockée dans `%APPDATA%\Poverello\config.json`.

### Les appels du planificateur MCP échouent
Assurez-vous qu'une clé API est définie pour le fournisseur sélectionné sous l'onglet **MCP**, ou exportée dans vos variables d'environnement utilisateur Windows (par exemple, `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`). La passerelle 3P n'a pas besoin d'être en cours d'exécution pour le MCP.

---

## Licence

Licence MIT. Voir [LICENSE](../LICENSE).
