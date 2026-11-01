[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Utilisez Claude Code / Claude Desktop comme environnement de développement, routez les inférences vers des modèles cloud ou locaux et gérez les modèles externes et les règles pour Google Antigravity.**

Poverello Coding Tools (affiché dans l’application sous le nom **Poverello**) est une application Windows complémentaire pour le développement logiciel assisté par IA. Elle prend en charge cinq flux de travail complémentaires :

1. **Passerelle 3P pour Claude Code / Claude Desktop** — Conservez l'exploration de dépôts, l'utilisation d'outils, l'édition de fichiers et l'exécution de tests de Claude tout en routant les inférences vers des fournisseurs tiers.
2. **LLM local dédié à Claude Code (Ollama)** — Exécutez Claude Code hors ligne avec des modèles locaux sans aucun coût d'API.
3. **Planificateur & Réviseur MCP pour Google Antigravity** — Déléguez la planification, la révision des plans et la revue post-implémentation à des modèles externes via les outils MCP `poverello/plan`, `poverello/revise` et `poverello/review`.
4. **Antigravity Autocode et Skills & Rules** — Configurez la revue externe et le comportement de codage dans Antigravity Autocode ; gérez les Antigravity Workflow Skills, Task Presets, Global Rules et Nerf Skills dans Settings. Les CLI partagées ont leur propre page Settings.
5. **Codex Autocode et Skills & Rules** — Configurez les réviseurs des Codex Workflow Skills dans Codex Autocode ; gérez les trois Skills Codex natifs et la section AGENTS.md gérée par Poverello dans Settings. Les CLI partagées se gèrent séparément dans Settings.

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
Modèle cloud ou local configuré
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

Antigravity Workflow Skills: `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (installation et réparation dans **Paramètres > Antigravity Skills & Rules** ; Reviewer Policy dans **Antigravity Autocode**). Les Skills Codex natifs portant les mêmes noms de commande se gèrent séparément dans Settings > Codex Skills & Rules.

**Flux de travail recommandé :**

```text
/pov-mcp-plan → Implémentation & Tests → /pov-mcp-review → Commit
```

### 4. Antigravity Autocode et gestion des Skills & Rules
```text
Antigravity Autocode tab
  • External Review policy / reviewer configuration
  • Customize Coding Settings / Direct Coding

Settings > Antigravity Skills & Rules
  • Workflow Commands / repair
  • Task Presets
  • Global Rules / Nerf skills

Settings > CLI Tools
  • Shared Node.js / npm and CLI management
```

- External Review et Customize Coding Settings se configurent dans Antigravity Autocode. Settings > Antigravity Skills & Rules gère les Antigravity Workflow Skills, Task Presets, Global Rules et Nerf Skills ; les CLI partagées ont une page Settings distincte.
### 5. Codex Autocode et Skills & Rules

Gérez les Codex Workflow Skills (/pov-plan-only, /pov-build-review, /pov-full-cycle) dans Settings > Codex Skills & Rules, puis choisissez les réviseurs et le réviseur prioritaire dans Codex Autocode. Poverello ne modifie que sa section balisée dans CODEX_HOME/AGENTS.md et conserve les autres instructions utilisateur.

```text
Codex Workflow Skills
       ↓ configured reviewers
Codex CLI / optional DeepSeek Harness or Gemini CLI
       ↓
Codex Autocode review settings

Settings > Codex Skills & Rules
  • /pov-plan-only, /pov-build-review, /pov-full-cycle
  • Poverello-managed instructions in CODEX_HOME/AGENTS.md
Settings > CLI Tools
  • Shared Codex CLI, Gemini CLI, DeepSeek Harness, Node.js / npm status
```


Gemini CLI est un réviseur facultatif à activer dans Settings > CLI Tools. L’activer ne le sélectionne pas et ne lance aucune revue. Sous Windows, Gemini Review est indisponible si un sandbox en lecture seule ne peut pas être vérifié. Les états d’installation et d’authentification sont affichés séparément. Node.js/npm sont détectés comme prérequis, mais ne sont pas installés automatiquement par Poverello.

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
4. Suivez le [Guide complet de configuration du MCP Antigravity](ANTIGRAVITY_MCP.fr.md).

### Workflow 4 : Antigravity Autocode et Skills & Rules
1. Configurez External Review et Customize Coding Settings dans **Antigravity Autocode**. La politique choisie détermine les réviseurs appelés par les Workflow Skills.
2. Gérez les Antigravity Workflow Skills, Task Presets, Global Rules et Nerf Skills dans **Settings > Antigravity Skills & Rules**. La réparation des Skills demande confirmation et conserve une sauvegarde. Gérez les CLI partagées dans **Settings > CLI Tools**.
3. Auto-Converge reste masqué ; le dispatch de production est désactivé et l’isolation Stage 2 n’est pas vérifiée.
### Workflow 5 : Codex Autocode et Skills & Rules
1. Vérifiez Node.js/npm et l’état des CLI dans Settings > CLI Tools. Poverello n’installe pas Node.js automatiquement.
2. Gérez /pov-plan-only, /pov-build-review, /pov-full-cycle et la section Poverello de CODEX_HOME/AGENTS.md dans Settings > Codex Skills & Rules.
3. Choisissez les réviseurs et le réviseur prioritaire dans Codex Autocode. Gemini CLI apparaît uniquement après activation dans Settings > CLI Tools ; l’activation ne le sélectionne pas. Sans sandbox Windows en lecture seule vérifié, Gemini Review est indisponible.

## Documentation

- [Notes de version (NEWS.md)](../NEWS.md) — Fonctionnalités et détails d'implémentation par version
- [Configuration de la passerelle 3P pour Claude Desktop / Cowork](THIRD_PARTY_INFERENCE.fr.md)
- [Ollama Local](OLLAMA_LOCAL.fr.md)
- [Configuration de Google Antigravity + Poverello MCP](ANTIGRAVITY_MCP.fr.md)
- [Référence de configuration (`config.json`)](CONFIGURATION.md)
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
