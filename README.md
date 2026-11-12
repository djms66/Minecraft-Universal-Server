# Monde Ensemble 0.6.5

Application de bureau francophone qui prépare un nouveau serveur Minecraft Paper pour joueurs Java et Bedrock, et protège les dossiers de serveur par sauvegarde et restauration dans une copie neuve.

## Ce que fait cette version

- détecte le système, l’architecture, la mémoire et l’espace libre local ;
- consulte les versions annoncées par GeyserMC et PaperMC, et bloque la préparation si leur catalogue est inaccessible ;
- prépare un nouveau dossier Paper + Geyser + Floodgate avec un runtime Java inclus et vérifié par SHA-256 ;
- démarre ensuite ce nouveau serveur local, attend le message Minecraft « Done » et vérifie les ports Java TCP et Bedrock UDP avant d’annoncer qu’il est prêt ; les dossiers de serveur précédents ne sont pas remplacés ;
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

Toutes les installations neuves et restaurations sont faites à côté des dossiers existants. Sur le Pi, l’application vérifie que les ports choisis sont libres et ne touche pas aux autres dossiers. L’acceptation de l’EULA est requise. La connexion SSH doit être déjà configurée sur cet ordinateur; aucune clé inconnue n’est approuvée automatiquement. Le compte choisi doit pouvoir écrire dans son dossier personnel.

## Limites à connaître

- La préparation d’un Pi neuf est intégrée, mais l’écriture réelle sur carte/SSD et le premier démarrage doivent encore être validés sur chaque cible Mac, Windows et Linux, avec leurs appareils et supports physiques. Le paquet Mac local ne certifie pas les autres plateformes.
- Le démarrage automatique après redémarrage dépend du service de planification déjà actif sur le Pi. L’application en vérifie la présence; sinon elle affiche que cette fonction n’est pas configurée.
- Elle n’a pas encore configuré une réservation DHCP dans la Freebox : l’adresse affichée est celle observée sur le réseau et peut changer. Pour une adresse durable, réserve l’adresse du Pi dans le routeur ; l’application ne prétend pas l’avoir fixée.
- Elle ne met pas à jour un serveur existant. Elle consulte les versions officielles compatibles pour une nouvelle installation, mais ne remplace ni ne met à niveau le monde actif. L’installation locale est démarrée par l’application ; l’arrêt et la gestion de son processus restent à améliorer.
- Elle ne gère pas encore l’arrêt, les joueurs, les mods, ni les packs de ressources. Le redémarrage manuel est disponible seulement pour le serveur enregistré par l’application.
- Le message Bedrock `InitialConnection-149` n’est pas diagnostiqué depuis les logs du serveur par cette version.
- Les consoles nécessitent parfois un relais d’accès; les mods Java côté client et les packs Java ne deviennent pas universels sur Bedrock.
- Une compilation sur Mac ne valide pas les installateurs Windows ou Raspberry Pi. Il faut encore compiler et vérifier ces cibles sur leurs machines correspondantes.

Ces limites sont affichées dans l’application afin de ne pas présenter les versions cataloguées comme une vérification complète du monde existant.

## Compatibilité du profil neuf

Le profil utilise un serveur Java Paper avec les versions officielles de Geyser et Floodgate consultées au moment de la préparation. Il permet le jeu partagé Java/Bedrock pour les versions indiquées par le catalogue Geyser. Les versions Bedrock peuvent différer des versions Java. L’application ne prétend pas que les mods, packs, extensions ou consoles fonctionnent tous sans configuration.

## Raspberry Pi

Sur un Pi déjà démarré, choisis « Sur un Raspberry Pi », saisis le nom de son compte, puis sélectionne « Trouver mon Pi ». L’application cherche sur le réseau local; si plusieurs Pi répondent, choisis celui qui convient. Elle utilise uniquement une clé SSH déjà approuvée et refuse les connexions qui demandent un mot de passe ou une clé inconnue. Ensuite, confirme l’EULA et choisis « Installer et démarrer le serveur ». La confirmation indique l’adresse et les ports. Les clients du même réseau local utilisent cette adresse; l’accès depuis l’extérieur demande aussi un réglage du routeur.

Pour un Pi neuf, branche sa carte SD ou son SSD à l’ordinateur qui exécute l’application. Détecte le support, relis son nom et sa capacité, puis confirme son effacement. L’application télécharge Raspberry Pi Imager depuis la publication officielle dont l’empreinte est vérifiée, écrit Raspberry Pi OS Lite 64 bits et prépare Ethernet ou le Wi-Fi. Le mot de passe Wi-Fi est effacé du support après le premier démarrage ; préfère le câble réseau si tu ne veux pas l’écrire temporairement sur la carte. Insère ensuite le support, branche le Pi au même routeur et allume-le. Le bouton de fin recherche le Pi pendant son premier démarrage, puis installe et contrôle le serveur. La mémoire, le modèle, les ports et l’espace libre sont vérifiés avant l’installation de Minecraft. L’interface reste sur Mac/PC ; le Pi héberge le serveur.

Sur Windows, l’installation de l’outil officiel peut déclencher la confirmation administrateur de Windows ; sur Mac/Linux, l’écriture du support demande l’autorisation du système. Cette étape est imposée par l’écriture d’un disque. L’application conserve la protection de Raspberry Pi Imager contre l’écriture sur le disque système.

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
