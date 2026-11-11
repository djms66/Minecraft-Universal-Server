# Monde Ensemble

Application macOS pour préparer une nouvelle installation Paper avec jeu commun Java/Bedrock.

## Profil protégé

- Paper reste sur la ligne Minecraft 26.2 jusqu’à validation explicite d’un profil compatible complet.
- Le dernier build stable Paper de cette ligne est téléchargé depuis l’API officielle PaperMC v3 et son SHA-256 est vérifié.
- Geyser, Floodgate, ViaVersion et ViaBackwards sont récupérés depuis leurs catalogues officiels.
- Un runtime Eclipse Temurin Java 25 pour Mac Apple Silicon est inclus dans le dossier préparé.
- Une sauvegarde choisie peut restaurer le monde, les configurations et les plugins; seuls Paper et les quatre ponts sont remplacés.
- L’installation locale ou distante est créée dans un nouveau dossier unique. Rien n’est écrasé et le serveur n’est pas démarré.

Le profil conserve les plugins de contenu de la sauvegarde (dont les extensions sous licence); ils ne sont jamais récupérés par une source tierce ou remplacés automatiquement. Les versions de ces plugins doivent être mises à jour uniquement après validation de compatibilité.

## Limites de compatibilité

Geyser permet à Bedrock de rejoindre un serveur Java. Les mods client requis ne sont pas compatibles avec toutes les consoles et tous les appareils mobiles. Les packs Java ne sont pas convertis automatiquement en packs Bedrock. Les consoles nécessitent une méthode d’accès compatible aux serveurs tiers.

La préparation d’un dossier ne garantit pas l’accès réseau de l’extérieur. Le routeur doit laisser passer le trafic requis; Bedrock utilise le port configuré dans Geyser.

## Lancer

`./script/build_and_run.sh`

