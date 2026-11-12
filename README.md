# Monde Ensemble 0.15.0

Application de bureau francophone qui prépare un nouveau serveur Minecraft Paper pour joueurs Java et Bedrock, et protège les dossiers de serveur par sauvegarde et restauration dans une copie neuve.

## Ce que fait cette version

- détecte le système, l’architecture, la mémoire et l’espace libre local ;
- consulte les versions annoncées par GeyserMC et PaperMC, et bloque la préparation si leur catalogue est inaccessible ;
- prépare un nouveau dossier Paper + Geyser + Floodgate avec un runtime Java inclus et vérifié par SHA-256 ;
- démarre ensuite ce nouveau serveur local, attend le message Minecraft « Done » et vérifie les ports Java TCP, Bedrock TCP (signalement NetherNet) et Bedrock UDP avant d’annoncer qu’il est prêt ; les dossiers de serveur précédents ne sont pas remplacés ;
- mémorise le chemin des serveurs locaux créés par l’application, affiche l’adresse, les versions, les joueurs connectés et le journal, et permet de les démarrer ou de les arrêter proprement avec le contrôle RCON limité à la boucle locale ;
- permet d’ajouter ou de retirer des comptes Java et des gamertags Bedrock de la liste d’accès d’un serveur local, puis de l’activer après avoir vérifié qu’elle contient au moins un joueur ; l’application n’ouvre pas temporairement la liste pour laisser entrer un Bedrock inconnu ;
- compare à la demande les versions locales aux catalogues officiels et bloque la validation d’une mise à jour si des mods, extensions, packs, liens, configuration Geyser modifiée ou runtime incompatible sont détectés ; ce contrôle est en lecture seule ;
- en mode avancé, recherche des plugins Paper publiés pour la version Java exacte du serveur dans le catalogue Modrinth, sans toucher au serveur ; affiche les dépendances obligatoires et ouvre la fiche officielle ;
- au démarrage demandé d’un serveur local géré, met à jour Paper, Geyser et Floodgate ensemble uniquement si les catalogues et les contenus sont compatibles ; crée une archive complète, prépare une nouvelle copie du serveur et conserve le dossier précédent ; en cas de blocage, démarre la version actuelle ;
- conserve un aperçu lisible des dernières lignes et permet d’exporter le journal complet local ou Pi vers un nouveau fichier sans écraser une copie existante ;
- choisit les ports Minecraft habituels (Java 25565, Bedrock 19132) pour que les joueurs puissent entrer seulement l’adresse du serveur ; si un autre serveur occupe l’un d’eux, utilise l’alternative 25566 ou 19133 et affiche le numéro à saisir ; bloque l’installation si les deux ports d’une édition sont déjà utilisés ;
- adapte la mémoire Java à la mémoire détectée et bloque les machines sous 2 Go ;
- crée une archive complète du dossier serveur, avec empreinte SHA-256, sans remplacer une archive qui porte déjà le même nom ;
- vérifie le contrôle d’intégrité et les chemins d’une archive avant restauration, puis restaure dans un nouveau dossier ;
- recherche les Raspberry Pi qui annoncent SSH sur le réseau local et affiche leur modèle et leur adresse ;
- repère en lecture seule les cartes SD et supports USB externes sur Mac, Windows ou Linux pour préparer un futur parcours Pi vierge ;
- lit le catalogue Raspberry Pi officiel pour proposer l’image Raspberry Pi OS Lite 64 bits avec le format de démarrage requis, sa date de publication et sa taille ;
- prépare une carte SD ou un SSD depuis l’application avec Raspberry Pi Imager officiel, téléchargé à la demande et vérifié par SHA-256 ; choisit Raspberry Pi OS Lite et configure l’accès sécurisé sans demander au débutant d’installer Imager séparément ;
- prépare Ethernet en automatique ou le Wi-Fi saisi dans l’application, crée une clé SSH si l’ordinateur n’en possède pas, attend le premier démarrage du Pi et lance ensuite l’installation du serveur ;
- affiche les modèles Pi compatibles avec cette image et compare la capacité des supports externes détectés à la place nécessaire au système et au serveur ;
- distingue la compatibilité de l’image de celle du serveur : un Pi 4 ou 5 doit avoir au moins 2 Go de mémoire ; le Pi 3 standard n’est pas adapté à ce profil ;
- vérifie l’architecture, la mémoire, l’espace libre et les ports avant d’installer sur un Pi déjà configuré ;
- transfère une version Linux ARM64 du serveur, la décompresse dans un nouveau dossier, démarre Minecraft et attend le signal « serveur prêt » dans le journal ;
- affiche l’adresse et les ports à saisir dans Minecraft, ainsi que le chemin du dossier et des journaux sur le Pi.
- conserve le Pi installé dans l’application, revérifie son état et ses versions Minecraft à l’ouverture, affiche les dernières lignes du journal et peut relancer uniquement cette installation si ses ports sont libres ; l’installation ajoute aussi une tâche de redémarrage automatique si le service de planification du Pi est actif.
- quand un serveur Pi géré est arrêté puis relancé depuis l’application, compare les catalogues Java/Bedrock et les builds Paper/Geyser/Floodgate ; si le profil est propre et que le runtime Java reste compatible, télécharge les trois fichiers contrôlés, archive le serveur complet, met le groupe à jour et vérifie le redémarrage ; en cas d’échec, restaure l’archive précédente ; chaque sauvegarde reste conservée sur le Pi.
- permet d’arrêter proprement ce serveur Pi depuis son tableau de bord ; le processus n’est jamais forcé à quitter s’il ne s’arrête pas correctement.

Toutes les installations neuves et restaurations sont faites à côté des dossiers existants. Sur le Pi, l’application vérifie que les ports choisis sont libres et ne touche pas aux autres dossiers. L’acceptation de l’EULA est requise. La connexion SSH doit être déjà configurée sur cet ordinateur; aucune clé inconnue n’est approuvée automatiquement. Le compte choisi doit pouvoir écrire dans son dossier personnel.

## Limites à connaître

- La préparation d’un Pi neuf est intégrée, mais l’écriture réelle sur carte/SSD et le premier démarrage doivent encore être validés sur chaque cible Mac, Windows et Linux, avec leurs appareils et supports physiques. Le paquet Mac local ne certifie pas les autres plateformes.
- Le démarrage automatique après redémarrage dépend du service de planification déjà actif sur le Pi. L’application en vérifie la présence; sinon elle affiche que cette fonction n’est pas configurée.
- Elle n’a pas encore configuré une réservation DHCP dans la Freebox : l’adresse affichée est celle observée sur le réseau et peut changer. Pour une adresse durable, réserve l’adresse du Pi dans le routeur ; l’application ne prétend pas l’avoir fixée.
- La mise à jour coordonnée automatique est limitée aux serveurs Pi créés par Monde Ensemble. Le serveur doit être arrêté depuis l’application ; un mod, plugin non géré, pack de données, pack client, lien symbolique, runtime Java incompatible, espace insuffisant ou catalogue indisponible reporte la mise à jour et conserve la version actuelle.
- Une mise à jour de Paper peut modifier les données de monde au premier démarrage. L’application conserve donc une archive complète avant lancement et ne valide la mise à jour qu’après le signal serveur prêt et l’ouverture des ports Java et Bedrock. Les sauvegardes automatiques s’accumulent et exigent de l’espace libre; l’application ne supprime pas les anciennes copies.
- Les profils locaux antérieurs à 0.8.0 n’ont pas le contrôle RCON local nécessaire aux boutons de démarrage, d’arrêt et de mise à jour. Une copie mise à jour locale prend de l’espace supplémentaire : l’ancienne version et l’archive sont conservées, sans suppression automatique.
- La gestion des joueurs est disponible sur les nouveaux serveurs locaux et Pi gérés. Les profils Pi plus anciens peuvent être préparés depuis le tableau de bord après arrêt du serveur; deux copies des réglages sont créées avant toute modification. Sur Bedrock, Floodgate exige que le joueur ait déjà rejoint un serveur Geyser avant que sa commande de liste d’accès puisse l’ajouter.
- Le mode avancé propose un catalogue consultatif de plugins Paper Modrinth filtré sur la version Java exacte du serveur et les versions publiées pour Paper. Aucun plugin n’est installé : les dépendances et le retour arrière après démarrage doivent être gérés avant d’autoriser une installation.
- Le contrôle du serveur repère maintenant l’absence du port TCP Bedrock, mais ne vérifie pas la redirection du routeur, le filtrage opérateur, l’authentification Microsoft, ni la compatibilité de chaque appareil. Le diagnostic `InitialConnection-149` reste donc partiel.
- Le catalogue de plugins est informatif : la prise en charge déclarée Paper/Minecraft ne garantit ni le fonctionnement réel, ni la compatibilité avec Geyser et Bedrock. Aucun téléchargement ou ajout de plugin n’est autorisé dans cette version.
- Les consoles nécessitent parfois un relais d’accès; les mods Java côté client et les packs Java ne deviennent pas universels sur Bedrock.
- Une compilation sur Mac ne valide pas les installateurs Windows ou Raspberry Pi. Il faut encore compiler et vérifier ces cibles sur leurs machines correspondantes.

Ces limites sont affichées dans l’application afin de ne pas présenter les versions cataloguées comme une vérification complète du monde existant.

## Compatibilité du profil neuf

Le profil utilise un serveur Java Paper avec les versions officielles de Geyser et Floodgate consultées au moment de la préparation. Il permet le jeu partagé Java/Bedrock pour les versions indiquées par le catalogue Geyser. Les versions Bedrock peuvent différer des versions Java. L’application ne prétend pas que les mods, packs, extensions ou consoles fonctionnent tous sans configuration.

Pour les clients Bedrock 26.60 et plus récents, Geyser doit utiliser NetherNet : l’installation configure ce transport et vérifie que le port Bedrock répond en TCP pour le signalement et en UDP pour le jeu. Le contrôle à domicile ne peut pas confirmer une redirection de routeur, un pare-feu externe, un filtrage opérateur ou un accès Internet.

## Raspberry Pi

Les serveurs Pi créés par cette version disposent aussi d’une sauvegarde complète vers l’ordinateur et d’une liste d’accès Java/Bedrock gérée depuis le tableau de bord. Le contrôle RCON reste limité à la boucle locale du Pi et chaque action passe par un tunnel SSH temporaire. Les installations Pi plus anciennes affichent un message dédié au lieu de modifier leur configuration en silence.

Sur un Pi déjà démarré, choisis « Sur un Raspberry Pi », saisis le nom de son compte, puis sélectionne « Trouver mon Pi ». L’application cherche sur le réseau local; si plusieurs Pi répondent, choisis celui qui convient. Elle utilise uniquement une clé SSH déjà approuvée et refuse les connexions qui demandent un mot de passe ou une clé inconnue. Ensuite, confirme l’EULA et choisis « Installer et démarrer le serveur ». La confirmation indique l’adresse et les ports. Les clients du même réseau local utilisent cette adresse; l’accès depuis l’extérieur demande aussi un réglage du routeur.

Pour un Pi neuf, branche sa carte SD ou son SSD à l’ordinateur qui exécute l’application. Détecte le support, relis son nom et sa capacité, puis confirme son effacement. L’application télécharge Raspberry Pi Imager depuis la publication officielle dont l’empreinte est vérifiée, écrit Raspberry Pi OS Lite 64 bits et prépare Ethernet ou le Wi-Fi. Le mot de passe Wi-Fi est effacé du support après le premier démarrage ; préfère le câble réseau si tu ne veux pas l’écrire temporairement sur la carte. Insère ensuite le support, branche le Pi au même routeur et allume-le. Le bouton de fin recherche le Pi pendant son premier démarrage, puis installe et contrôle le serveur. La mémoire, le modèle, les ports et l’espace libre sont vérifiés avant l’installation de Minecraft. L’interface reste sur Mac/PC ; le Pi héberge le serveur.

Sur Windows, l’installation de l’outil officiel peut déclencher la confirmation administrateur de Windows ; sur Mac/Linux, l’écriture du support demande l’autorisation du système. Cette étape est imposée par l’écriture d’un disque. L’application conserve la protection de Raspberry Pi Imager contre l’écriture sur le disque système.

## Builds par plateforme

Le workflow GitHub Actions produit séparément un DMG et une archive macOS Apple Silicon, un installateur Windows x64, puis des paquets Debian Linux x64 et ARM64. Les sorties restent des artefacts de compilation, pas des versions certifiées : elles doivent encore être ouvertes et vérifiées sur chaque système cible. Le paquet ARM64 est destiné à un Linux graphique compatible avec les dépendances Tauri ; Raspberry Pi OS Lite est sans interface de bureau et ne lance pas cette application graphique.

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
- mise à jour Pi uniquement après contrôle de compatibilité et sauvegarde complète vérifiée ; en cas d’échec de démarrage, retour arrière à la sauvegarde précédente.
- aucun remplacement d’un dossier existant lors d’une nouvelle installation ou d’une restauration.
