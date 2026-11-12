# Monde Ensemble

Application de bureau francophone qui prépare un nouveau serveur Minecraft Paper pour joueurs Java et Bedrock, et protège les dossiers de serveur par sauvegarde et restauration dans une copie neuve.

## Ce que fait cette version

- détecte le système, l’architecture, la mémoire et l’espace libre local ;
- consulte les versions annoncées par GeyserMC et PaperMC, et bloque la préparation si leur catalogue est inaccessible ;
- prépare un nouveau dossier Paper + Geyser + Floodgate avec un runtime Java inclus et vérifié par SHA-256 ;
- règle des ports distincts (Java 25566, Bedrock 19133) pour éviter de perturber un serveur existant ;
- adapte la mémoire Java à la mémoire détectée et bloque les machines sous 2 Go ;
- crée une archive complète du dossier serveur, avec empreinte SHA-256, sans remplacer une archive qui porte déjà le même nom ;
- vérifie le contrôle d’intégrité et les chemins d’une archive avant restauration, puis restaure dans un nouveau dossier ;
- explique la préparation du Raspberry Pi et permet de vérifier un accès SSH déjà configuré, sans écrire sur le Pi.

Toutes les installations neuves et restaurations sont faites à côté des dossiers existants. L’acceptation de l’EULA est requise avant de préparer un serveur. Le serveur n’est pas démarré automatiquement.

## Ce que cette version ne fait pas

- Elle n’installe pas encore le serveur à distance sur un Raspberry Pi. La vérification SSH est en lecture seule.
- Elle ne met pas à jour un serveur existant et ne garantit pas que les plugins privés/tiers de ton monde sont compatibles. Elle ne touche donc pas à l’installation active.
- Elle ne gère pas encore le démarrage, l’arrêt, les joueurs, les mods, ni les packs de ressources.
- Le message Bedrock `InitialConnection-149` n’est pas diagnostiqué depuis les logs du serveur par cette version.
- Les consoles nécessitent parfois un relais d’accès; les mods Java côté client et les packs Java ne deviennent pas universels sur Bedrock.
- Une compilation sur Mac ne valide pas les installateurs Windows ou Raspberry Pi. Il faut encore compiler et vérifier ces cibles sur leurs machines correspondantes.

Ces limites sont affichées dans l’application afin de ne pas présenter les versions cataloguées comme une vérification complète du monde existant.

## Compatibilité du profil neuf

Le profil utilise un serveur Java Paper avec les versions officielles de Geyser et Floodgate consultées au moment de la préparation. Il permet le jeu partagé Java/Bedrock pour les versions indiquées par le catalogue Geyser. Les versions Bedrock peuvent différer des versions Java. L’application ne prétend pas que les mods, packs, extensions ou consoles fonctionnent tous sans configuration.

## Raspberry Pi

L’application ouvre le guide officiel Raspberry Pi Imager et explique le choix d’une image 64 bits lorsqu’elle est proposée pour la carte. Imager peut effacer le support sélectionné : le nom et la capacité doivent être vérifiés avant écriture. L’app sait mesurer le Pi via une connexion SSH déjà configurée, mais pas encore y installer le serveur. Sur Raspberry Pi OS Lite, l’interface doit rester sur le Mac/PC et le Pi héberge uniquement le serveur.

## Compiler

Prérequis : Node.js, Rust et les prérequis de compilation Tauri correspondant au système.

```sh
npm install
npm run build
cd src-tauri && cargo check
```

Pour ouvrir l’application pendant le développement ou produire un paquet local :

```sh
npm run tauri dev
npm run tauri build
```

Les versions Windows et Linux/Raspberry Pi doivent être produites et vérifiées sur des appareils de ces plateformes. Une compilation locale Mac seule ne suffit pas pour les certifier.

## Principes de sécurité

- pas de remplacement de dossier lors d’une installation ou restauration ;
- vérification HTTPS et SHA-256 des fichiers téléchargés ;
- pas de mot de passe SSH enregistré ni de validation automatique de clé d’hôte ;
- sauvegarde refusée si l’utilisateur ne confirme pas l’arrêt du serveur ;
- restauration bloquée pour les liens et chemins d’archive dangereux ;
- aucune modification du serveur actif par les fonctions disponibles.
