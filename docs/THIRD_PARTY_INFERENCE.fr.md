[English](THIRD_PARTY_INFERENCE.md) | [日本語](THIRD_PARTY_INFERENCE.ja.md) | [中文(简体)](THIRD_PARTY_INFERENCE.zh-CN.md) | [中文(繁體)](THIRD_PARTY_INFERENCE.zh-TW.md) | [한국어](THIRD_PARTY_INFERENCE.ko.md) | [Français](THIRD_PARTY_INFERENCE.fr.md) | [Deutsch](THIRD_PARTY_INFERENCE.de.md) | [Español](THIRD_PARTY_INFERENCE.es.md)

# Utiliser Poverello avec Claude Desktop / Cowork on 3P

Poverello peut être utilisé comme une passerelle locale compatible Anthropic pour
Claude Desktop / Cowork on 3P.

Claude Desktop / Cowork on 3P supporte l'inférence tierce via la
fenêtre de configuration intégrée.

Documentation officielle :

- [https://claude.com/docs/cowork/3p/installation](https://claude.com/docs/cowork/3p/installation)
- [https://claude.com/docs/cowork/3p/configuration](https://claude.com/docs/cowork/3p/configuration)

## 1. Démarrer Poverello

Démarrez Poverello d'abord et assurez-vous que la passerelle est en cours d'exécution.

Par défaut, Poverello écoute sur :

```text
http://127.0.0.1:4000
```

Laissez Poverello en cours d'exécution pendant l'utilisation de Claude Desktop / Cowork on 3P.

## 2. Activer le mode développeur dans Claude Desktop

Ouvrez Claude Desktop.

Sous Windows, ouvrez le menu de l'application en haut à gauche.

Sélectionnez ensuite :

```text
Help → Troubleshooting → Enable Developer Mode
```

Après l'activation du mode développeur, un nouveau menu `Developer` apparaîtra.

## 3. Ouvrir les paramètres d'inférence tierce

Ouvrez :

```text
Developer → Configure third-party inference
```

Ceci ouvre la fenêtre de configuration de l'inférence tierce.

## 4. Configurer la connexion

Dans la section `Connection`, sélectionnez :

```text
Gateway
```

Entrez ensuite les valeurs suivantes.

| Champ                 | Valeur                                      |
| --------------------- | ------------------------------------------ |
| Gateway base URL      | `http://127.0.0.1:4000`                    |
| Gateway API key       | `sk-local-gateway`                         |
| Gateway auth scheme   | `bearer`                                   |
| Gateway extra headers | Laissez vide sauf si vous avez besoin d'en-têtes personnalisés |

La `Gateway API key` doit correspondre à la clé API locale configurée dans Poverello.

## 5. Configurer l'identité et les modèles

Dans la section `Identity & Models`, ajoutez les IDs de modèles que Claude Desktop doit afficher dans le sélecteur de modèles.

Exemple :

```text
claude-sonnet-4-6
claude-haiku-4-5
```

Vous pouvez également donner à chaque modèle un libellé d'affichage.

Exemple :

| ID du modèle         | Libellé d'affichage |
| -------------------- | ------------------- |
| `claude-sonnet-4-6`  | `Gateway Pro`       |
| `claude-haiku-4-5`   | `Gateway Flash`     |

Le premier modèle de la liste est utilisé comme entrée par défaut du sélecteur.

Pour chaque modèle, développez la ligne et confirmez que le `Model ID` est exactement le nom du modèle que vous souhaitez que Claude Desktop envoie à Poverello.

N'activez `Offer 1M-context variant` que si votre fournisseur upstream et le modèle sélectionné supportent réellement la fenêtre de contexte étendue.

## 6. Exemple de configuration

Les paramètres ci-dessus correspondent à la configuration d'inférence tierce suivante :

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:4000",
  "inferenceGatewayApiKey": "sk-local-gateway",
  "inferenceGatewayAuthScheme": "bearer",
  "inferenceModels": [
    {
      "name": "claude-sonnet-4-6",
      "labelOverride": "Gateway Pro"
    },
    {
      "name": "claude-haiku-4-5",
      "labelOverride": "Gateway Flash"
    }
  ]
}
```

## 7. Appliquer et redémarrer Claude Desktop

Après avoir configuré la passerelle et la liste des modèles, appliquez les paramètres localement.

Redémarrez Claude Desktop si vous y êtes invité.

Une fois Claude Desktop redémarré, les requêtes de Cowork on 3P seront envoyées à Poverello. Poverello route ensuite les requêtes vers le fournisseur upstream configuré dans Poverello.

## API DeepSeek directe

Lorsque Poverello est configuré avec le fournisseur Direct DeepSeek, les requêtes de Claude Desktop / Cowork on 3P sont acheminées vers le point de terminaison DeepSeek compatible avec Anthropic Messages.

Les ID API canoniques de DeepSeek sont `deepseek-flash` et `deepseek-v4-pro` ; la page officielle des tarifs actuelle les répertorie comme DeepSeek-V4.1-Flash-1017 et DeepSeek-V4-Pro-0813. Ce sont des ID de l’API upstream. Les noms visibles par Claude dans `inferenceModels` sont des entrées locales du sélecteur, associées par Poverello au profil fournisseur sélectionné.

Les deux modèles prennent en charge les appels d’outils, un contexte de 1M tokens et jusqu’à 384K tokens en sortie. Seul Flash prend en charge les images en entrée. DeepSeek accepte encore les anciens ID `deepseek-v4-flash` et `deepseek-v4-flash-vision-exp` comme alias upstream de Flash. Les paramètres Direct DeepSeek intégrés à Poverello utilisent les ID canoniques. Les sources officielles divergent sur le routage et la facturation de Pro : la page actuelle publie des tarifs Pro distincts et confirme la poursuite du service API après le 14 septembre 2026, tandis qu’une annonce du 10 septembre décrivait un routage temporaire vers V4.1 Flash au tarif Flash jusqu’au lancement de V4.1-Pro. Poverello ne peut pas vérifier le routage ni la facturation de chaque requête ; les tarifs affichés sont ceux publiés, pas une facture ni une garantie. Voir les [tarifs actuels](https://api-docs.deepseek.com/quick_start/pricing/) et l’[annonce du 10 septembre](https://api-docs.deepseek.com/news/news260910/).

Consultez les [tarifs officiels de DeepSeek](https://api-docs.deepseek.com/quick_start/pricing/) et le [guide des détails des fournisseurs](PROVIDERS.md#deepseek) pour les tarifs par token et les règles de périodes de pointe et hors pointe. Le calendrier des jours fériés fourni avec Poverello influence l’estimation locale ; sa couverture et le comportement de repli sont décrits dans le guide des fournisseurs.

## Fournisseur Xiaomi MiMo

Lorsque Poverello est configuré pour utiliser le fournisseur Direct MiMo, les requêtes d'inférence de Claude Desktop sont acheminées vers l'API MiMo de Xiaomi.

### Modèles MiMo-V2.6

| Model ID | Nom d'affichage | Multimodal |
| --- | --- | --- |
| `mimo-v2.6-flash` | MiMo-V2.6-Flash | Image + Video |
| `mimo-v2.6-pro` | MiMo-V2.6-Pro | Image + Video |
| `mimo-v2.6-pro-ultraspeed` | MiMo-V2.6-Pro-UltraSpeed | Image + Video |

Tous les modèles V2.6 supportent une fenêtre de contexte de 1 000 000 tokens.

### Normal / Thinking

MiMo utilise un simple bouton bascule Normal/Thinking. Il n'y a pas de niveaux d'effort de raisonnement (Low/Medium/High).

| Paramètre | Paramètre upstream | Comportement |
| --- | --- | --- |
| Normal | `thinking: {"type": "disabled"}` | Inférence standard |
| Thinking | `thinking: {"type": "enabled"}` | Réflexion étendue |

### Routes par défaut

Les routes intégrées du preset Direct MiMo :

| Modèle passerelle | Modèle upstream | Thinking |
| --- | --- | --- |
| `claude-opus-5` | `mimo-v2.6-pro` | Thinking |
| `claude-sonnet-5` | `mimo-v2.6-pro` | Normal |
| `claude-haiku-4-5` | `mimo-v2.6-flash` | Thinking |

### Compatibilité ascendante V2.5

Les configurations `mimo-v2.5`, `mimo-v2.5-pro` et `mimo-v2.5-pro-ultraspeed` sauvegardées sont préservées et continuent de fonctionner. Seules les routes par défaut d'usine non modifiées sont automatiquement migrées vers V2.6 au démarrage. Les routes personnalisées, les paramètres de mode Thinking et les valeurs `default_model` personnalisées ne sont jamais écrasés.

> [!NOTE]
> **Claude Desktop / Cowork on 3P utilise les fournisseurs cloud via la passerelle**
>
> Claude Desktop et Cowork on 3P envoient des requêtes d\'inférence à la passerelle Poverello (`http://127.0.0.1:4000`), qui les achemine vers votre fournisseur cloud en amont configuré (DeepSeek, MiMo, Kimi, MiniMax, OpenRouter).
>
> Le backend **Ollama Local** est strictement réservé à **Claude Code CLI** et reste totalement isolé de Claude Desktop et du MCP. Pour la configuration du LLM local avec Claude Code, veuillez consulter le [Guide Ollama Local](OLLAMA_LOCAL.fr.md).

---

## Mode Thinking de MiniMax-M3

MiniMax-M3 permet d'activer ou de désactiver le thinking étendu via l'API compatible avec Anthropic.

| Réglage | Paramètre de l'API upstream | Comportement |
| --- | --- | --- |
| Thinking activé | `thinking: {"type": "adaptive"}` | Active le thinking étendu |
| Thinking désactivé | `thinking: {"type": "disabled"}` | Désactive le thinking étendu |
| Par défaut (non défini) | *omis* | Valeur par défaut de l'API : thinking désactivé |

Les paramètres de Poverello affichent un commutateur Thinking/Normal pour MiniMax-M3. Les modèles MiniMax-M2.x (par exemple `MiniMax-M2.7-highspeed`) ne permettent pas de désactiver le thinking et restent en mode Thinking uniquement.

## Configuration de Laguna S/XS 2.1 (OpenRouter)

Par défaut, Poverello route les modèles OpenRouter `claude-opus-5` et `claude-sonnet-5` vers `poolside/laguna-s-2.1`, et `claude-haiku-4-5` vers `poolside/laguna-xs-2.1`.

### Comportement du thinking

Des tests tiers ont observé que la structure du prompt système peut influencer le comportement Thinking de Laguna S 2.1. Un persona professionnel clair accompagné de critères d'acceptation explicites aurait réduit le raisonnement inutile. À la suite de ces résultats, le mode par défaut de Laguna Opus (`claude-opus-5`) est passé à Normal (thinking désactivé) ; Thinking peut être réactivé dans les paramètres.

### Limitation connue : aucune sortie exploitable à la limite de jetons

Dans certaines conditions — notamment lorsque le modèle atteint sa limite de jetons par tour pendant le raisonnement — Laguna S 2.1 peut renvoyer uniquement du contenu de raisonnement, sans texte ni appel d'outil, avec `stop_reason: "max_tokens"`. Poverello détecte cette situation et consigne un avertissement. Si « Reasoning-only response reached the per-turn token limit » apparaît régulièrement, envisagez d'augmenter le maximum de jetons de sortie ou de désactiver Thinking pour ce modèle.

Ces observations proviennent de tests communautaires tiers ; les résultats peuvent varier selon la configuration du fournisseur et la révision du modèle.

## Notes

Poverello est une passerelle locale non officielle compatible Anthropic.

Il n'est pas affilié à Anthropic, Moon Bridge ou à un quelconque fournisseur de modèles upstream.

Claude Desktop / Cowork on 3P est configuré via les paramètres d'inférence tierce de Claude. Les libellés de menu et les champs de configuration peuvent changer à mesure qu'Anthropic met à jour Claude Desktop.
