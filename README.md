# Monde Ensemble 0.18.45

Application de bureau francophone qui prépare un nouveau serveur Minecraft Paper pour joueurs Java et Bedrock, et protège les dossiers de serveur par sauvegarde et restauration dans une copie neuve.

## Ce que fait cette version

- démarre les nouveaux utilisateurs dans un guide « installer mon serveur » plutôt que dans un tableau de bord vide ; pour un Pi neuf, montre d’abord le matériel nécessaire et la préparation depuis la carte, puis présente uniquement l’adresse et les versions après installation ; les outils de réglage apparaissent ensuite dans le tableau de bord ;
- détecte le système, l’architecture, la mémoire et l’espace libre local ;
- consulte les versions annoncées par GeyserMC et PaperMC, et bloque la préparation si leur catalogue est inaccessible ;
- prépare un nouveau dossier Paper + Geyser + Floodgate avec un runtime Java inclus et vérifié par SHA-256 ;
- démarre ensuite ce nouveau serveur local, attend le message Minecraft « Done » et vérifie les ports Java TCP, Bedrock TCP (signalement NetherNet) et Bedrock UDP avant d’annoncer qu’il est prêt ; les dossiers de serveur précédents ne sont pas remplacés ;
- mémorise le chemin des serveurs locaux créés par l’application, affiche l’adresse, les versions, les joueurs connectés et le journal, et permet de copier l’adresse Java complète ou l’IP Bedrock depuis le tableau de bord ; les boutons démarrent ou arrêtent le serveur proprement avec le contrôle RCON limité à la boucle locale ;
- permet d’ajouter ou de retirer des comptes Java et des gamertags Bedrock de la liste d’accès d’un serveur local, puis de l’activer après avoir vérifié qu’elle contient au moins un joueur ; l’application n’ouvre pas temporairement la liste pour laisser entrer un Bedrock inconnu ;
- compare à la demande les versions locales aux catalogues officiels et bloque la validation d’une mise à jour si des mods, extensions, packs, liens, configuration Geyser modifiée ou runtime incompatible sont détectés ; ce contrôle est en lecture seule ; enregistre aussi les empreintes SHA-256 du Paper, de Geyser et de Floodgate ; pour un ancien profil sans empreinte, vérifie le build installé auprès des catalogues officiels avant d’autoriser la mise à jour ;
- en mode avancé, recherche dans Modrinth les plugins Paper déclarés pour la version Java exacte ; sur un serveur local ou Pi arrêté, crée une sauvegarde et une nouvelle copie avec un plugin publié sans dépendance obligatoire, vérifie son SHA-512 et permet de revenir à l’ancienne copie ;
- en mode avancé, ajoute un pack de ressources Bedrock `.mcpack` ou `.zip` au dossier Geyser du serveur local ou Pi après validation du manifeste, du type, du nom, de la taille et de l’empreinte SHA-256 ; le serveur doit être arrêté et redémarré après l’ajout ;
- en mode avancé, configure pour les joueurs Java un lien HTTPS direct vers un pack ZIP, son empreinte SHA-1 facultative et l’option qui l’exige ; sauvegarde `server.properties` avant l’enregistrement local ou Pi ;
- en mode avancé, règle le nom affiché, le mode de jeu, la difficulté, le nombre maximal de joueurs, les distances d’affichage et de simulation, le PvP et le Nether ; exige l’arrêt du serveur, sauvegarde `server.properties` avant l’écriture et préserve les propriétés non gérées, dont le secret RCON ;
- au démarrage demandé d’un serveur local géré, met à jour Paper, Geyser et Floodgate ensemble uniquement si les catalogues et les contenus sont compatibles ; crée une archive complète, prépare une nouvelle copie du serveur et conserve le dossier précédent ; en cas de blocage ou si la nouvelle copie ne confirme pas son démarrage, relance et vérifie l’ancienne version ; la copie candidate et l’archive restent conservées pour diagnostic ;
- applique la même mise à jour coordonnée aux serveurs locaux gérés sur Mac et Windows, après validation du profil, des contenus, de l’espace libre et des ports ; vérifie la nouvelle copie au démarrage et conserve l’ancienne pour retour arrière ;
- conserve un aperçu lisible des dernières lignes et permet d’exporter le journal complet local ou Pi vers un nouveau fichier sans écraser une copie existante ;
- choisit les ports Minecraft habituels (Java 25565, Bedrock 19132) pour que les joueurs puissent entrer seulement l’adresse du serveur ; si un autre serveur occupe l’un d’eux, utilise l’alternative 25566 ou 19133 et affiche le numéro à saisir ; bloque l’installation si les deux ports d’une édition sont déjà utilisés ;
- adapte automatiquement la mémoire et la portée de jeu aux ressources du Pi ; le profil compact accepte les Pi 3B+ et autres Pi ARM64 avec 1 Go, tandis que l’installation locale sur ordinateur garde son minimum de 2 Go ;
- crée une archive complète du dossier serveur, avec empreinte SHA-256, sans remplacer une archive existante ; vérifie que les ports Minecraft configurés restent libres pendant la copie du monde ;
- vérifie l’intégrité et les chemins d’une archive avant restauration, puis crée une nouvelle copie ; si son profil Monde Ensemble est reconnu et que ses ports sont libres, la charge automatiquement dans le tableau de bord ;
- restaure une archive vers un Raspberry Pi déjà configuré en créant un dossier serveur neuf, en vérifiant l’empreinte, en adaptant Java à Linux ARM64 et à sa mémoire, puis en contrôlant la copie à distance ; l’ancien dossier reste disponible et la copie restaurée n’est pas démarrée automatiquement ;
- recherche les Raspberry Pi qui annoncent SSH sur le réseau local et affiche leur modèle et leur adresse ;
- repère en lecture seule les cartes SD dans les lecteurs intégrés et les supports USB externes sur Mac, Windows ou Linux pour préparer un futur parcours Pi vierge ;
- bloque l’écriture d’un système Pi s’il détecte plusieurs supports externes, affiche l’identifiant du support choisi et demande une confirmation explicite de son effacement ;
- lit le catalogue Raspberry Pi officiel pour proposer l’image Raspberry Pi OS Lite 64 bits avec le format de démarrage requis, sa date de publication et sa taille ;
- prépare une carte SD ou un SSD depuis l’application avec Raspberry Pi Imager officiel, téléchargé à la demande et vérifié par SHA-256 ; choisit Raspberry Pi OS Lite et configure l’accès sécurisé sans demander au débutant d’installer Imager séparément ;
- utilise uniquement les options reconnues par la version vérifiée de Raspberry Pi Imager et masque les données Wi-Fi dans les journaux de diagnostic conservés par l’application ; la vérification de l’image et de l’écriture reste active ;
- prépare Ethernet en automatique ou le Wi-Fi saisi dans l’application, crée une clé SSH dédiée dans l’espace privé de l’application sans lire ni modifier les clés SSH personnelles, puis l’utilise pour communiquer avec le Pi ; attend le premier démarrage et lance ensuite l’installation du serveur ;
- affiche les modèles Pi compatibles avec cette image et compare la capacité des supports externes détectés à la place nécessaire au système et au serveur ;
- choisit un profil compact sur les Pi 3B+ et les appareils disposant de moins de 2 Go : portée de jeu et nombre de joueurs réduits ; un Pi 4 ou 5 avec au moins 4 Go est recommandé pour plus de joueurs ou d’extensions ;
- vérifie l’architecture, la mémoire, l’espace libre et les ports avant d’installer sur un Pi déjà configuré ;
- transfère une version Linux ARM64 du serveur, la décompresse dans un nouveau dossier, démarre Minecraft et attend le signal « serveur prêt » dans le journal ;
- affiche l’adresse et les ports à saisir dans Minecraft, avec le chemin du dossier et des journaux sur le Pi.
- repère sur le Pi l’interface réseau utilisée pour l’adresse locale, affiche son adresse matérielle et guide la réservation DHCP permanente dans l’application Free ; les valeurs IP et matérielle sont copiables depuis le tableau de bord.
- conserve le Pi installé dans l’application, revérifie son état et ses versions Minecraft à l’ouverture, affiche les dernières lignes du journal et peut relancer uniquement cette installation si ses ports sont libres ; l’installation ajoute aussi une tâche de redémarrage automatique si le service de planification du Pi est actif.
- quand un serveur Pi géré est arrêté puis relancé depuis l’application, compare les catalogues Java/Bedrock et les builds Paper/Geyser/Floodgate ; si le profil est propre et que le runtime Java reste compatible, télécharge les trois fichiers contrôlés, archive le serveur complet, met le groupe à jour et vérifie le redémarrage ; en cas d’échec, restaure l’archive précédente ; chaque sauvegarde reste conservée sur le Pi.
- permet d’arrêter proprement ce serveur Pi depuis son tableau de bord ; le processus n’est jamais forcé à quitter s’il ne s’arrête pas correctement.

Toutes les installations neuves et restaurations sont faites à côté des dossiers existants. Sur le Pi, l’application vérifie que les ports choisis sont libres et ne touche pas aux autres dossiers. L’acceptation de l’EULA est requise. La connexion SSH doit être déjà configurée sur cet ordinateur; aucune clé inconnue n’est approuvée automatiquement. Le compte choisi doit pouvoir écrire dans son dossier personnel.

## Limites à connaître

- La préparation d’un Pi neuf est intégrée, mais l’écriture réelle sur carte/SSD et le premier démarrage doivent encore être validés sur chaque cible Mac, Windows et Linux, avec leurs appareils et supports physiques. Le paquet Mac local ne certifie pas les autres plateformes.
- Le démarrage automatique après redémarrage dépend du service de planification déjà actif sur le Pi. L’application en vérifie la présence; sinon elle affiche que cette fonction n’est pas configurée.
- L’application n’écrit pas dans la Freebox et ne peut donc pas confirmer elle-même le bail DHCP. Le tableau de bord fournit l’adresse IP et matérielle ainsi qu’un parcours guidé pour enregistrer le bail permanent dans l’application Free ; l’adresse ne devient durable qu’après cette confirmation dans la box.
- La mise à jour coordonnée automatique est limitée aux serveurs locaux et Pi créés par Monde Ensemble. Le serveur doit être arrêté depuis l’application ; un mod, plugin non géré, pack de données, pack client, lien symbolique, runtime Java incompatible, espace insuffisant ou catalogue indisponible reporte la mise à jour et conserve la version actuelle.
- Une mise à jour de Paper peut modifier les données de monde au premier démarrage. L’application conserve donc une archive complète avant lancement et ne valide la mise à jour qu’après le signal serveur prêt et l’ouverture des ports Java et Bedrock. Les sauvegardes automatiques s’accumulent et exigent de l’espace libre; l’application ne supprime pas les anciennes copies.
- Les profils locaux antérieurs à 0.8.0 n’ont pas le contrôle RCON local nécessaire aux boutons de démarrage, d’arrêt et de mise à jour. Une copie mise à jour locale prend de l’espace supplémentaire : l’ancienne version et l’archive sont conservées, sans suppression automatique.
- La gestion des joueurs est disponible sur les nouveaux serveurs locaux et Pi gérés. Les profils Pi plus anciens peuvent être préparés depuis le tableau de bord après arrêt du serveur; deux copies des réglages sont créées avant toute modification. Sur Bedrock, Floodgate exige que le joueur ait déjà rejoint un serveur Geyser avant que sa commande de liste d’accès puisse l’ajouter.
- En mode avancé, l’ajout de plugin fonctionne sur un serveur local ou Pi géré : la version exacte annoncée pour Paper est revalidée, le fichier est contrôlé par SHA-512 et une archive complète ainsi qu’une nouvelle copie sont créées sans remplacer le serveur d’origine. La copie candidate doit être démarrée manuellement et vérifiée ; l’application permet de revenir à l’ancienne copie. Les dépendances obligatoires, plugins non gérés et mods présents bloquent l’opération.
- L’ajout de pack Bedrock vérifie le format du manifeste et la version minimale déclarée, pas le rendu en jeu ni toutes les versions clientes. Geyser ne convertit pas automatiquement un pack Java en pack Bedrock ; les consoles peuvent avoir des contraintes propres. Un pack personnalisé bloque les mises à jour automatiques jusqu’à un contrôle de compatibilité séparé.
- Le réglage Java ne téléverse pas le ZIP et ne vérifie pas que le lien public fonctionne. Le serveur requiert une URL directe accessible en HTTPS ; si l’option est exigée, les joueurs Java qui refusent le téléchargement sont déconnectés. Un pack Java ne devient pas un pack Bedrock.
- Le paquet macOS est signé ad hoc pour sceller correctement l’application ; il n’est pas signé par un certificat Developer ID ni notarié. macOS peut afficher un avertissement à l’ouverture, ce qui doit encore être résolu pour une installation débutant sans manipulation de sécurité.
- Le tableau de bord guide la redirection des ports Bedrock TCP/UDP et Java TCP dans l’application Freebox, sans écrire dans le routeur. Les voyants confirment les services sur l’hôte, pas leur accessibilité depuis Internet. Le filtrage opérateur, l’authentification Microsoft et la compatibilité de chaque appareil ne sont pas testés ; le diagnostic InitialConnection-149 reste partiel.
- Le catalogue annonce les versions Paper/Minecraft et le côté serveur déclarés par l’éditeur ; cela ne garantit pas le fonctionnement réel ni la compatibilité avec Geyser et Bedrock. Les mises à jour coordonnées sont bloquées en présence de plugins ajoutés ; les sauvegardes d’ajout de plugin et leurs copies candidates sont conservées, sans nettoyage automatique.
- Les consoles nécessitent parfois un relais d’accès; les mods Java côté client et les packs Java ne deviennent pas universels sur Bedrock.
- Une compilation sur Mac ne valide pas les installateurs Windows ou Raspberry Pi. Il faut encore compiler et vérifier ces cibles sur leurs machines correspondantes.

Ces limites sont affichées dans l’application afin de ne pas présenter les versions cataloguées comme une vérification complète du monde existant.

## Compatibilité du profil neuf

Le profil utilise un serveur Java Paper avec les versions officielles de Geyser et Floodgate consultées au moment de la préparation. Il permet le jeu partagé Java/Bedrock pour les versions indiquées par le catalogue Geyser. Les versions Bedrock peuvent différer des versions Java. L’application ne prétend pas que les mods, packs, extensions ou consoles fonctionnent tous sans configuration.

Pour les clients Bedrock 26.60 et plus récents, Geyser doit utiliser NetherNet : l’installation configure ce transport et vérifie que le port Bedrock répond en TCP pour le signalement et en UDP pour le jeu. Le contrôle à domicile ne peut pas confirmer une redirection de routeur, un pare-feu externe, un filtrage opérateur ou un accès Internet.

## Raspberry Pi

Les serveurs Pi créés par cette version disposent d’une sauvegarde complète vers l’ordinateur, d’une restauration de sauvegarde vers un nouveau dossier Pi et d’une liste d’accès Java/Bedrock gérée depuis le tableau de bord. Le contrôle RCON reste limité à la boucle locale du Pi et chaque action passe par un tunnel SSH temporaire. Les installations Pi plus anciennes affichent un message dédié au lieu de modifier leur configuration en silence. La restauration vers Pi exige une archive contenant un profil Monde Ensemble reconnu et Paper, Geyser et Floodgate ; elle refuse les liens, les chemins dangereux, un serveur actif, une architecture non ARM64 ou un espace insuffisant. La copie restaurée reste arrêtée jusqu’à ce que l’utilisateur la démarre depuis le tableau de bord.

Sur un Pi déjà démarré, choisis « Sur un Raspberry Pi », saisis le nom de son compte, puis sélectionne « Trouver mon Pi ». L’application cherche sur le réseau local; si plusieurs Pi répondent, choisis celui qui convient. Elle utilise uniquement une clé SSH déjà approuvée et refuse les connexions qui demandent un mot de passe ou une clé inconnue. Ensuite, confirme l’EULA et choisis « Installer et démarrer le serveur ». La confirmation indique l’adresse et les ports. Les clients du même réseau local utilisent cette adresse; l’accès depuis l’extérieur demande aussi un réglage du routeur. Le tableau du Pi indique son interface active, son IP et son adresse matérielle. Ouvre « Garder cette adresse sur la durée » et utilise les deux boutons de copie pour ajouter un bail statique ou permanent dans l’application Free : Mes équipements Freebox → Server internet → Paramètres réseau avancés → DHCP. Actualise le tableau après l’enregistrement pour relire l’adresse courante.

Pour un Pi neuf sans stockage, la préparation commence sur l’ordinateur qui exécute l’application : branche la carte SD ou le SSD vide à cet ordinateur, détecte le support, relis son nom et sa capacité, puis confirme son effacement. L’application télécharge Raspberry Pi Imager depuis la publication officielle dont l’empreinte est vérifiée, écrit Raspberry Pi OS Lite 64 bits et prépare Ethernet ou le Wi-Fi. Si tu saisis le mot de passe Wi-Fi, l’application le place temporairement dans un fichier de démarrage et programme la suppression de ce fichier au premier démarrage. Préfère le câble réseau si tu ne veux pas écrire le mot de passe sur la carte. Quand l’écriture est vérifiée, éteins le Pi avant d’y insérer la carte, connecte-le au même routeur, puis rallume-le. Le bouton de fin tente d’abord de joindre le nom unique inscrit sur la carte, puis recherche le service SSH sur le réseau local. Il affiche chaque tentative avant d’installer et de contrôler le serveur directement sur le Pi. La mémoire, le modèle, les ports et l’espace libre sont vérifiés avant l’installation de Minecraft. Le Mac prépare la carte et pilote l’installation ; le Pi héberge le serveur.

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

Le paquet Windows x64 est produit par compilation croisée depuis Mac, mais doit encore être lancé sur Windows pour valider son fonctionnement réel. La préparation d’un support Pi doit être vérifiée avec une carte/SSD de test et un Raspberry Pi physique ; le paquet ARM64 Linux ne valide pas Raspberry Pi OS Lite ni son premier démarrage.

## Principes de sécurité

- pas de remplacement de dossier lors d’une installation ou restauration ;
- vérification HTTPS et SHA-256 des fichiers téléchargés ;
- une sauvegarde n’est pas publiée si les ports Minecraft du serveur sont actifs au début ou à la fin de la copie ; sur Pi, l’archive temporaire est aussi relue avant son transfert ;
- pas de mot de passe SSH enregistré ni de validation automatique de clé d’hôte ;
- sauvegarde refusée si l’utilisateur ne confirme pas l’arrêt du serveur ;
- restauration bloquée pour les liens et chemins d’archive dangereux ;
- mise à jour Pi uniquement après contrôle de compatibilité et sauvegarde complète vérifiée ; en cas d’échec de démarrage, retour arrière à la sauvegarde précédente.
- aucun remplacement d’un dossier existant lors d’une nouvelle installation ou d’une restauration.
