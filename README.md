# Monde Ensemble

Application de bureau francophone pour préparer et protéger un serveur Minecraft partagé entre les éditions Java et Bedrock. Interface « Minecraft » dessinée en CSS, sans ressource ou police téléchargée à l’exécution.

## État actuel

Cette version constitue le socle multiplateforme et le parcours de démarrage :

- interface Tauri 2 / React, prévue pour macOS, Windows et Linux 64 bits, dont Raspberry Pi OS ARM64 avec environnement de bureau ;
- détection locale du système, de l’architecture, du modèle Raspberry Pi, de la mémoire et du disque système ;
- parcours débutant, explications de compatibilité, sauvegarde et restauration, et mode avancé séparé ;
- préparation de zéro du Raspberry Pi expliquée depuis un Mac/PC, lien vers l’outil officiel, puis vérification SSH non destructive ;
- aucune demande de mot de passe et aucune écriture sur le Pi pendant la vérification. La clé d’hôte doit déjà être approuvée.

Les actions « installer le serveur », « sauvegarder » et « restaurer » sont ici des parcours d’interface, pas encore des opérations reliées au moteur serveur. Ne pas utiliser cette version comme installateur ni pour modifier le serveur en production. Le moteur devra prendre en charge la sauvegarde, le contrôle d’intégrité, l’installation parallèle, la validation du démarrage, le retour arrière et la gestion des composants sous licence avant que ces boutons exécutent des changements.

## Compatibilité visée

Le profil multiplateforme visé est un serveur Paper avec Geyser/Floodgate et les extensions requises pour les versions de clients choisies. Java sur PC/Mac et Bedrock sur Windows, téléphones et tablettes peuvent alors jouer dans le même monde. Les consoles demandent une méthode d’accès supplémentaire. Les mods côté client ne sont pas universels; les packs Java ne deviennent pas automatiquement des packs Bedrock.

Le futur moteur de mise à jour doit calculer un profil complet à partir des versions effectivement compatibles du serveur, de Geyser, de Floodgate et des extensions conservées. Si une dépendance ne suit pas, le profil actuellement installé reste actif. La sauvegarde comprend le monde, les configurations et données des extensions; aucune extension payante ou sous licence n’est téléchargée par l’application.

## Raspberry Pi, installation neuve

Depuis un Mac ou un PC, choisir « Sur un Raspberry Pi » et suivre le lien Raspberry Pi Imager. Choisir Raspberry Pi OS 64 bits lorsqu’il est pris en charge par la carte, sélectionner la carte ou le SSD externe comme destination, puis activer l’accès distant. Raspberry Pi Imager efface le support sélectionné : contrôler sa capacité et son nom dans Imager. Après le premier démarrage, configurer l’accès SSH et revenir vérifier le Pi dans l’application. Une future étape doit mesurer chaque disque du Pi à distance avant d’autoriser l’installation.

Sur un Pi sans environnement graphique (Raspberry Pi OS Lite), l’application de bureau s’exécute sur le Mac/PC et pilote le Pi à distance; l’application ne prétend pas afficher une interface graphique sur Lite.

## Développement et compilation

Prérequis : Node.js récent, Rust et les dépendances Tauri documentées pour le système qui compile.

```sh
npm install
./script/build_and_run.sh dev
./script/build_and_run.sh build
```

Chaque système et chaque architecture doivent être compilés et vérifiés sur une machine correspondante (macOS ARM64, Windows x64, Raspberry Pi OS ARM64). Une compilation Mac seule ne prouve pas ces autres installateurs.

## Décisions de sécurité

- détection système en lecture seule ;
- validation stricte du nom d’hôte et du compte SSH avant utilisation ;
- SSH sans saisie de mot de passe et sans ajout automatique de clé d’hôte ;
- interface sans requête web ou police distante ;
- installation future en nouveau dossier, sauvegarde vérifiée avant bascule et retour arrière explicite.
