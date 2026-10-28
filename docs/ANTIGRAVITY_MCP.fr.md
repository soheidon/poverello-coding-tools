[English](ANTIGRAVITY_MCP.md) | [日本語](ANTIGRAVITY_MCP.ja.md) | [中文(简体)](ANTIGRAVITY_MCP.zh-CN.md) | [中文(繁體)](ANTIGRAVITY_MCP.zh-TW.md) | [한국어](ANTIGRAVITY_MCP.ko.md) | [Français](ANTIGRAVITY_MCP.fr.md) | [Deutsch](ANTIGRAVITY_MCP.de.md) | [Español](ANTIGRAVITY_MCP.es.md)

[← Retour au README Poverello](README.fr.md)

# Utiliser le MCP Poverello avec Google Antigravity

Poverello ne nécessite aucun exécutable de serveur MCP séparé. Le fichier unique `anthro-bridge.exe` installé fournit à la fois l'application de bureau GUI et le serveur MCP. Antigravity démarre le mode MCP en exécutant ce même fichier avec l'argument `--mcp-server`.

La clé canonique du serveur MCP est `poverello` et expose `poverello/plan`, `poverello/revise` et `poverello/review`. L’exécutable `anthro-bridge.exe`, l’ancienne clé `anthro-bridge` et les anciens outils `anthro-bridge/*` sont conservés uniquement pour la compatibilité. Le produit s’appelle Poverello Coding Tools et l’application affiche Poverello.

```text
Lancement normal
anthro-bridge.exe
→ Application de bureau Poverello / Passerelle 3P

Lancement MCP
anthro-bridge.exe --mcp-server
→ Serveur MCP stdio headless pour Antigravity
```

Cela permet aux environnements d'agents tels que Google Antigravity de déléguer la conception architecturale et la planification d'implémentation à des modèles LLM externes (DeepSeek V4, MiMo, Kimi, MiniMax ou modèles OpenRouter) via `poverello/plan`, tout en effectuant les modifications de code, commandes de terminal, compilations et tests intensifs en tokens avec le quota d'abonnement Antigravity.

---

## 1. Fonctionnement de ce flux de travail

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Modèle planificateur externe configuré
    ↓
Plan d'implémentation structuré renvoyé
    ↓
Antigravity exécute les modifications,
la compilation et les tests via son abonnement
```

---

## 2. Prérequis

1. **Poverello** installé sous Windows.
2. Authentification du fournisseur configurée dans Poverello ou dans les variables d'environnement système pour le fournisseur choisi.
3. **Google Antigravity** installé et en cours d'exécution.

---

## 3. Configurer le serveur MCP dans Antigravity

### Méthode 1 — Configuration via l'interface GUI d'Poverello (Recommandée)

1. Ouvrez Poverello et accédez à **Paramètres** (onglet `[Paramètres]`) > sous-navigation gauche **Antigravity**.
2. Dans la carte **Intégration Google Antigravity** :
   - **Exécutable cible** : Affiche par défaut le chemin de l'exécutable `anthro-bridge.exe` en cours. Pour utiliser un autre binaire (build portable ou personnalisé), cliquez sur **Modifier** (`antigravity.btnChangeExe`) et sélectionnez l'exécutable.
   - **Enregistrer / Mettre à jour** : Cliquez sur **Mettre à jour la configuration Antigravity** (`antigravity.btnUpdate`) pour enregistrer ou mettre à jour en toute sécurité l'entrée `anthro-bridge` dans `%USERPROFILE%\.gemini\config\mcp_config.json`, tout en conservant intacts les autres serveurs MCP.
   - **Supprimer** : Cliquez sur **Supprimer la configuration** (`antigravity.btnRemove`) pour désinscrire le serveur d'Antigravity.
   - **Ouvrir le dossier** : Cliquez sur **Ouvrir le dossier des paramètres** (`antigravity.btnOpenFolder`) pour inspecter le dossier dans l'Explorateur Windows.

---

### Méthode 2 — Configuration manuelle (Avancée)

1. Dans Poverello **Paramètres > Antigravity**, cliquez sur **Ouvrir le dossier des paramètres** pour ouvrir `%USERPROFILE%\.gemini\config\` dans l'Explorateur Windows.
2. Ouvrez ou créez `mcp_config.json` et ajoutez l'entrée `anthro-bridge` sous `mcpServers` :

```json
{
  "mcpServers": {
    "anthro-bridge": {
      "command": "C:\\Users\\<USER>\\AppData\\Local\\Poverello\\anthro-bridge.exe",
      "args": ["--mcp-server"]
    }
  }
}
```

Pour les builds de développement, pointez directement vers l'exécutable Release :
```json
{
  "mcpServers": {
    "anthro-bridge": {
      "command": "C:\\Users\\<USER>\\path\\to\\poverello-coding-tools\\gui\\src-tauri\\target\\release\\anthro-bridge.exe",
      "args": ["--mcp-server"]
    }
  }
}
```

> [!TIP]
> Vous n'avez **pas** besoin d'écrire de clés API dans le fichier `mcp_config.json` d'Antigravity. Le serveur MCP exploite le système de résolution de clés d'Poverello (lecture des variables d'environnement Windows comme `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`, `MOONSHOT_API_KEY`, `MINIMAX_API_KEY`, `XIAOMI_API_KEY`, ou paramètres enregistrés).

---

## 4. Vérifier la connexion MCP

Dans la vue **Installed MCP Servers** d'Antigravity, confirmez que `anthro-bridge` est bien reconnu :

```text
anthro-bridge
  1 tool enabled
  - plan
```

---

## 5. Configurer les modèles planificateurs dans Poverello

Poverello sépare clairement le choix du planificateur de la gestion détaillée des paramètres :

1. **Onglet de premier niveau `MCP` (`MCP for Antigravity`)** :
   - Affiche les cartes des fournisseurs disponibles (DeepSeek, OpenRouter, MiniMax, MiMo (V2.6), Kimi et GLM), ainsi que les profils.
   - Cliquez sur une carte pour basculer instantanément le planificateur actif.
2. **`Paramètres` > `Antigravity`** :
   - Carte **Paramètres détaillés du plan MCP** : Configurez le modèle, le mode Thinking et le niveau Reasoning Effort par fournisseur/profil.
   - Carte **Intégration Google Antigravity** : Gérez l'enregistrement du serveur MCP, les commandes Antigravity (Global Skills) et les règles globales (`deepseek-harness.md`, `codex-cli.md`).

> [!NOTE]
> Le serveur MCP recharge dynamiquement la configuration à chaque appel de l'outil `plan()`. Vous **n'avez pas** besoin de redémarrer le serveur MCP ou Antigravity lors de la modification des paramètres dans l'interface GUI.

---

## 6. Utilisation des commandes Antigravity (`/pov-mcp-plan`, `/pov-mcp-revise` & `/pov-mcp-review`) (Recommandé)

Depuis **Paramètres > Antigravity > Intégration Google Antigravity**, installez les compétences globales pour utiliser les commandes slash dans tous vos espaces de travail Antigravity :

- Cliquez sur **Tout installer** (`antigravity.btnInstallAll`) ou sur **Installer** (`antigravity.commandBtnInstall`) à côté de chaque commande.

### Créer un nouveau plan d'implémentation :
```text
/pov-mcp-plan <description de la tâche ou de la fonctionnalité à implémenter>
```
*Collecte le contexte du dépôt, appelle `poverello/plan` et s'arrête proprement après avoir présenté le plan sans modifier de fichiers ni lancer de builds.*

### Réviser un plan d'implémentation existant :
```text
/pov-mcp-revise <retours ou nouvelles contraintes à intégrer>
```
*Identifie le plan actuel (contexte actif ou `implementation_plan.md`), transmet le plan et vos retours à `poverello/plan`, et met à jour le plan tout en préservant les sections intactes.*

### Réviser l’implémentation par rapport au plan approuvé :
```text
/pov-mcp-review
```
*Examine le plan approuvé, les différences de l’arbre de travail, les fichiers pertinents non suivis et les résultats des tests, puis présente la revue de l’implémentation. Cette commande est en lecture seule et ne modifie aucun fichier.*

> [!IMPORTANT]
> Lors de l'exécution via `/pov-mcp-plan`, `/pov-mcp-revise` ou `/pov-mcp-review` (ou les anciens `/anthro-*`), la commande gère l'appel d'outil. Aucune règle d'espace de travail ne déclenchera d'appel redondant.

---

---

## 7. Commandes Antigravity de niveau supérieur (`/pov-full-cycle`, `/pov-plan-only` & `/pov-build-review`)

En plus des compétences MCP directes, Poverello fournit trois compétences de flux de travail autonomes de niveau supérieur installées dans `~/.gemini/antigravity/skills/` :

### Développement en cycle complet :
```text
/pov-full-cycle <description de la tâche>
```
*Gère le cycle de bout en bout : rédaction du plan, revue externe indépendante du plan, révision du plan, implémentation autorisée, vérification du build et des tests, revue externe indépendante de l'implémentation et passes correctives bornées.*

### Cycle de vie dédié à la planification :
```text
/pov-plan-only <description de la tâche>
```
*Dédié à la rédaction et à la revue externe indépendante du plan. S'arrête proprement dès l'approbation du plan (`APPROVED` / `APPROVED_WITH_NON_BLOCKING_FINDINGS`) sans modifier de fichiers ni procéder à l'implémentation.*

### Cycle de vie de build et de revue :
```text
/pov-build-review
```
*Implémente un plan déjà approuvé, exécute les suites de tests, effectue la revue externe indépendante de l'implémentation et gère les limites de preuves et les modifications correctives autorisées.*
## 8. Automatisation de la planification via une Workspace Rule

Vous pouvez créer une règle Workspace facultative propre au projet à partir de l’exemple ci-dessous. Poverello ne fournit ni n’installe de règle de planification spécifique à DeepSeek ; le fournisseur et le modèle MCP configurés se sélectionnent dans les paramètres Poverello :

```markdown
---
trigger: model_decision
description: Use for implementation, debugging, architectural changes, or multi-file code changes where an external implementation plan would be useful. Do not use for trivial text-only edits.
---

# Poverello Planner Rule (exemple)

For non-trivial implementation tasks in this repository:

1. If the current task is being executed through the `/pov-mcp-plan` or `/pov-mcp-revise` command (or legacy `/anthro-plan` / `/anthro-revise`), do NOT invoke `poverello/plan` (or `anthro-bridge/plan`) separately. The command workflow owns the planner call.
2. First inspect the repository yourself and identify the files and code relevant to the task.
3. Summarize only the context necessary for implementation planning.
4. Call the `poverello/plan` MCP tool exactly once with:
   - the user's task;
   - the relevant repository context;
   - important constraints.
   Note: "Exactly once" means duplicate planner calls are prohibited once a successful usable result is obtained. If the tool call itself fails or returns an unusable response (e.g. transport or decoding error), exactly 1 recovery retry is permitted.
5. Use the returned plan as the basis for implementation.
6. Perform file edits, builds, and tests yourself.
7. Do not call `poverello/plan` again unless the implementation encounters a major unresolved problem.
8. Do not ask the user to repeat this workflow.
9. Do not use the planner for trivial tasks such as a one-word text change unless planning would materially help.
```

### Politique de déclenchement :
- **Tâches mineures / localisées (Trivial / localized tasks)** (correction de coquille, modification d'une ligne, ajustement syntaxique) : Le planificateur n'est pas appelé.
- **Tâches complexes (Non-trivial tasks)** (modifications d'architecture, fonctionnalités multi-fichiers, débogage complexe) : Antigravity analyse le code, appelle `poverello/plan` 1 seule fois, puis implémente le code selon le plan retourné.

---

## 9. Flux de travail automatisé typique

```text
Utilisateur : "Refactorisez la fonctionnalité X pour prendre en charge plusieurs profils."
    ↓
Antigravity inspecte les fichiers et résume le contexte
    ↓
Antigravity déclenche automatiquement l'appel à poverello/plan (1 fois)
    ↓
Poverello envoie la requête au modèle externe sélectionné
    ↓
Antigravity reçoit le plan d'implémentation structuré
    ↓
L'utilisateur examine et approuve le plan
    ↓
Antigravity applique les modifications et exécute les tests
```

---

## 10. Remarques importantes

- **Fonctionnement indépendant** : Le serveur MCP fonctionne de manière totalement indépendante de la passerelle 3P Gateway. Il n'est pas nécessaire d'activer la passerelle 3P pour utiliser le MCP.
- **Facturation séparée** : Les appels à `poverello/plan` sont facturés par le fournisseur d'API externe concerné. Les modifications et tests ultérieurs utilisent le quota de votre abonnement Antigravity.
- **Prise en compte immédiate** : Les changements de paramètres dans l'interface GUI d'Poverello s'appliquent immédiatement dès le prochain appel à `plan()`.

---
> [!NOTE]
> **La planification et la revue MCP utilisent les fournisseurs cloud**
>
> Antigravity MCP utilise le fournisseur de modèles cloud actif configuré dans Poverello (`mcp.provider` / `mcp.targets`). Le backend **Ollama Local** est exclusivement réservé à **Claude Code CLI** et n\'affecte pas les opérations MCP. Pour l\'inférence locale avec Claude Code, consultez le [Guide Ollama Local](OLLAMA_LOCAL.fr.md).
