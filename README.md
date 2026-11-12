# Monde Ensemble 0.5.0

Application de bureau francophone qui prépare un nouveau serveur Minecraft Paper pour joueurs Java et Bedrock, et protège les dossiers de serveur par sauvegarde et restauration dans une copie neuve.

## Ce que fait cette version

- détecte le système, l’architecture, la mémoire et l’espace libre local ;
- consulte les versions annoncées par GeyserMC et PaperMC, et bloque la préparation si leur catalogue est inaccessible ;
- prépare un nouveau dossier Paper + Geyser + Floodgate avec un runtime Java inclus et vérifié par SHA-256 ;
- règle des ports distincts (Java 25566, Bedrock 19133) pour éviter de perturber un serveur existant ;
- adapte la mémoire Java à la mémoire détectée et bloque les machines sous 2 Go ;
- crée une archive complète du dossier serveur, avec empreinte SHA-256, sans remplacer une archive qui porte déjà le même nom ;
- vérifie le contrôle d’intégrité et les chemins d’une archive avant restauration, puis restaure dans un nouveau dossier ;
- recherche les Raspberry Pi qui annoncent SSH sur le réseau local et affiche leur modèle et leur adresse ;
- vérifie l’architecture, la mémoire, l’espace libre et les ports avant d’installer sur un Pi déjà configuré ;
- transfère une version Linux ARM64 du serveur, la décompresse dans un nouveau dossier, démarre Minecraft et attend le signal « serveur prêt » dans le journal ;
- affiche l’adresse et les ports à saisir dans Minecraft, ainsi que le chemin du dossier et des journaux sur le Pi.
- conserve le Pi installé dans l’application, revérifie son état à l’ouverture, affiche les dernières lignes du journal et peut relancer uniquement cette installation si ses ports sont libres.

Toutes les installations neuves et restaurations sont faites à côté des dossiers existants. Sur le Pi, l’application vérifie que les ports choisis sont libres et ne touche pas aux autres dossiers. L’acceptation de l’EULA est requise. La connexion SSH doit être déjà configurée sur cet ordinateur; aucune clé inconnue n’est approuvée automatiquement. Le compte choisi doit pouvoir écrire dans son dossier personnel.

## Ce que cette version ne fait pas

- Elle n’installe pas encore le système Raspberry Pi OS sur une carte SD ou un SSD vierge : elle prend en charge un Pi déjà démarré et configuré.
- Elle n’installe pas encore un service qui relance le serveur automatiquement après un redémarrage du Pi; l’application permet de le relancer lorsqu’elle est ouverte.
- Elle n’a pas encore vérifié ni configuré une réservation DHCP dans la Freebox : l’adresse affichée est celle observée sur le réseau et peut changer.
- Elle ne met pas à jour un serveur existant et ne garantit pas que les plugins privés/tiers de ton monde sont compatibles. Elle ne touche donc pas à l’installation active.
- Elle ne gère pas encore l’arrêt, les joueurs, les mods, ni les packs de ressources. Le redémarrage du Pi est disponible seulement pour le serveur enregistré par l’application.
- Le message Bedrock `InitialConnection-149` n’est pas diagnostiqué depuis les logs du serveur par cette version.
- Les consoles nécessitent parfois un relais d’accès; les mods Java côté client et les packs Java ne deviennent pas universels sur Bedrock.
- Une compilation sur Mac ne valide pas les installateurs Windows ou Raspberry Pi. Il faut encore compiler et vérifier ces cibles sur leurs machines correspondantes.

Ces limites sont affichées dans l’application afin de ne pas présenter les versions cataloguées comme une vérification complète du monde existant.

## Compatibilité du profil neuf

Le profil utilise un serveur Java Paper avec les versions officielles de Geyser et Floodgate consultées au moment de la préparation. Il permet le jeu partagé Java/Bedrock pour les versions indiquées par le catalogue Geyser. Les versions Bedrock peuvent différer des versions Java. L’application ne prétend pas que les mods, packs, extensions ou consoles fonctionnent tous sans configuration.

## Raspberry Pi

Sur un Pi déjà démarré, choisis « Sur un Raspberry Pi », saisis le nom de son compte, puis sélectionne « Trouver mon Pi ». L’application cherche sur le réseau local; si plusieurs Pi répondent, choisis celui qui convient. Elle utilise uniquement une clé SSH déjà approuvée et refuse les connexions qui demandent un mot de passe ou une clé inconnue. Ensuite, confirme l’EULA et choisis « Installer et démarrer le serveur ». La confirmation indique l’adresse et les ports. Les clients du même réseau local utilisent cette adresse; l’accès depuis l’extérieur demande aussi un réglage du routeur.

Un Pi neuf, sans système installé, n’est pas encore automatisé : il faut préparer son système et son accès distant avant que l’application puisse le trouver. L’application le signale explicitement. Sur Raspberry Pi OS Lite, l’interface reste sur le Mac/PC et le Pi héberge uniquement le serveur.

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
- aucune mise à jour ni remplacement du monde existant par les fonctions disponibles.
