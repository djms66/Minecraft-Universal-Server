import { cloneElement, isValidElement, type ReactNode } from "react";

export const localeOptions = [
  ["fr", "Français"],
  ["en", "English"],
  ["zh", "中文（简体）"],
  ["hi", "हिन्दी"],
  ["es", "Español"],
  ["ar", "العربية"],
  ["bn", "বাংলা"],
  ["pt", "Português"],
  ["ru", "Русский"],
  ["ur", "اردو"],
  ["id", "Bahasa Indonesia"],
] as const;

export type Locale = (typeof localeOptions)[number][0];

const catalogs: Record<Exclude<Locale, "fr">, Record<string, string>> = {
  en: {
    "Monde Ensemble": "World Together",
    "Mon serveur": "My server",
    "Préparer le serveur": "Set up server",
    "Préparer mon serveur": "Set up my server",
    "Préparer un serveur": "Set up a server",
    Compatibilité: "Compatibility",
    "Ce qui peut jouer ensemble": "Who can play together",
    Sauvegardes: "Backups",
    "Sauvegarder et déplacer": "Back up and move",
    "Réglages avancés": "Advanced settings",
    "Mode débutant": "Beginner mode",
    "Mode avancé": "Advanced mode",
    "TON SERVEUR": "YOUR SERVER",
    "PREMIÈRE INSTALLATION": "FIRST SETUP",
    "TON SERVEUR MINECRAFT": "YOUR MINECRAFT SERVER",
    Version: "Version",
    Langue: "Language",
    Installer: "Install",
    "Aucun serveur installé": "No server installed",
    "Le guide va partir de zéro": "The guide starts from scratch",
    "Serveur enregistré": "Server saved",
    "L’installation existante est conservée":
      "Your existing installation is preserved",
    "Joueurs autorisés": "Allowed players",
    Édition: "Edition",
    "Java · PC et Mac": "Java · PC and Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · mobile, console, Windows",
    "Nom Java": "Java username",
    "Gamertag Xbox": "Xbox gamertag",
    "Ex. Alex_12": "e.g. Alex_12",
    "Ex. Alex 12": "e.g. Alex 12",
    Ajouter: "Add",
    Retirer: "Remove",
    "Liste active": "Allowlist on",
    "Liste inactive": "Allowlist off",
    "Serveur à démarrer": "Start the server first",
    "Seuls les joueurs listés peuvent rejoindre.":
      "Only listed players can join.",
    "La liste est inactive : toute personne connaissant l’adresse peut tenter de rejoindre.":
      "The allowlist is off. Anyone who knows the address can try to join.",
    "La liste est inactive : toute personne connaissant l’adresse du serveur peut tenter de rejoindre.":
      "The allowlist is off. Anyone who knows the server address can try to join.",
    "Voir la réponse du serveur": "Show server response",
    "Désactiver la liste d’accès": "Turn off allowlist",
    "Activer la liste d’accès": "Turn on allowlist",
    "Démarre le serveur pour ajouter ou retirer un joueur. La liste d’accès protège ton monde lorsque tu l’actives.":
      "Start the server to add or remove players. The allowlist protects your world when enabled.",
    "Un gamertag Bedrock doit avoir rejoint une première fois un serveur Geyser. La protection ne sera jamais désactivée pour contourner cette règle.":
      "A Bedrock gamertag may need to join a Geyser server once before Floodgate can identify it. The app will not disable protection automatically.",
    "Floodgate peut ajouter un gamertag après sa première connexion à un serveur Geyser. Si la liste est déjà active, il faut d’abord l’autoriser autrement; l’application ne désactive jamais la protection pour le faire.":
      "Floodgate can add a gamertag after its first connection to a Geyser server. If the allowlist is on, the player must be let in another way first; the app never disables protection automatically.",
    "Liste d’accès active": "Allowlist on",
    "Liste d’accès inactive": "Allowlist off",
    "Ajouter ou retirer un joueur": "Add or remove a player",
    "L’adresse à saisir dans Minecraft": "Address to enter in Minecraft",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Choose Bedrock for Xbox, PlayStation, Switch, phone, tablet, or Minecraft Bedrock on Windows.",
    "Le gamertag Xbox n’est pas un nom Java.":
      "An Xbox gamertag is not a Java username.",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "If Floodgate says the player does not exist, they must join this Geyser server once first. The app will not unexpectedly turn off protection.",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = the Minecraft launcher on PC/Mac. Bedrock = Xbox, PlayStation, Switch, phone, tablet, or Minecraft for Windows. Enter the name for the selected edition.",
  },
  zh: {
    "Monde Ensemble": "一起玩世界",
    "Mon serveur": "我的服务器",
    "Préparer le serveur": "设置服务器",
    "Préparer mon serveur": "设置我的服务器",
    "Préparer un serveur": "设置服务器",
    Compatibilité: "兼容性",
    "Ce qui peut jouer ensemble": "哪些玩家可以一起玩",
    Sauvegardes: "备份",
    "Sauvegarder et déplacer": "备份并迁移",
    "Réglages avancés": "高级设置",
    "Mode débutant": "新手模式",
    "Mode avancé": "高级模式",
    "TON SERVEUR": "你的服务器",
    "PREMIÈRE INSTALLATION": "首次安装",
    "TON SERVEUR MINECRAFT": "你的 Minecraft 服务器",
    Version: "版本",
    Langue: "语言",
    Installer: "安装",
    "Aucun serveur installé": "尚未安装服务器",
    "Le guide va partir de zéro": "向导将从零开始",
    "Serveur enregistré": "已保存服务器",
    "L’installation existante est conservée": "现有安装已保留",
    "Joueurs autorisés": "允许的玩家",
    Édition: "版本",
    "Java · PC et Mac": "Java · PC 和 Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · 手机、主机、Windows",
    "Nom Java": "Java 用户名",
    "Gamertag Xbox": "Xbox 玩家代号",
    "Ex. Alex_12": "例如 Alex_12",
    "Ex. Alex 12": "例如 Alex 12",
    Ajouter: "添加",
    Retirer: "移除",
    "Liste active": "允许列表已开启",
    "Liste inactive": "允许列表已关闭",
    "Serveur à démarrer": "请先启动服务器",
    "Seuls les joueurs listés peuvent rejoindre.": "只有列表中的玩家可以加入。",
    "Voir la réponse du serveur": "查看服务器回复",
    "Désactiver la liste d’accès": "关闭允许列表",
    "Activer la liste d’accès": "开启允许列表",
    "Ajouter ou retirer un joueur": "添加或移除玩家",
    "L’adresse à saisir dans Minecraft": "在 Minecraft 中输入的地址",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Xbox、PlayStation、Switch、手机、平板或 Windows 版 Minecraft Bedrock 请选择 Bedrock。",
    "Le gamertag Xbox n’est pas un nom Java.":
      "Xbox 玩家代号不是 Java 用户名。",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "如果 Floodgate 提示玩家不存在，该玩家需要先加入此 Geyser 服务器一次。应用不会擅自关闭保护。",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = PC/Mac 上的 Minecraft 启动器。Bedrock = Xbox、PlayStation、Switch、手机、平板或 Windows 版 Minecraft。请填写所选版本的玩家名称。",
  },
  hi: {
    "Monde Ensemble": "साथ खेलने की दुनिया",
    "Mon serveur": "मेरा सर्वर",
    "Préparer le serveur": "सर्वर सेट अप करें",
    "Préparer mon serveur": "मेरा सर्वर सेट अप करें",
    "Préparer un serveur": "सर्वर सेट अप करें",
    Compatibilité: "अनुकूलता",
    "Ce qui peut jouer ensemble": "कौन साथ खेल सकता है",
    Sauvegardes: "बैकअप",
    "Sauvegarder et déplacer": "बैकअप लें और स्थानांतरित करें",
    "Réglages avancés": "उन्नत सेटिंग",
    "Mode débutant": "शुरुआती मोड",
    "Mode avancé": "उन्नत मोड",
    "TON SERVEUR": "आपका सर्वर",
    "PREMIÈRE INSTALLATION": "पहला सेटअप",
    "TON SERVEUR MINECRAFT": "आपका Minecraft सर्वर",
    Version: "संस्करण",
    Langue: "भाषा",
    Installer: "इंस्टॉल करें",
    "Aucun serveur installé": "कोई सर्वर इंस्टॉल नहीं है",
    "Le guide va partir de zéro": "गाइड शून्य से शुरू होगी",
    "Serveur enregistré": "सर्वर सहेजा गया",
    "L’installation existante est conservée": "मौजूदा इंस्टॉलेशन सुरक्षित है",
    "Joueurs autorisés": "अनुमत खिलाड़ी",
    Édition: "संस्करण",
    "Java · PC et Mac": "Java · PC और Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · मोबाइल, कंसोल, Windows",
    "Nom Java": "Java उपयोगकर्ता नाम",
    "Gamertag Xbox": "Xbox गेमरटैग",
    "Ex. Alex_12": "जैसे Alex_12",
    "Ex. Alex 12": "जैसे Alex 12",
    Ajouter: "जोड़ें",
    Retirer: "हटाएँ",
    "Liste active": "अनुमति सूची चालू",
    "Liste inactive": "अनुमति सूची बंद",
    "Serveur à démarrer": "पहले सर्वर शुरू करें",
    "Seuls les joueurs listés peuvent rejoindre.":
      "केवल सूचीबद्ध खिलाड़ी जुड़ सकते हैं।",
    "Voir la réponse du serveur": "सर्वर का जवाब देखें",
    "Désactiver la liste d’accès": "अनुमति सूची बंद करें",
    "Activer la liste d’accès": "अनुमति सूची चालू करें",
    "Ajouter ou retirer un joueur": "खिलाड़ी जोड़ें या हटाएँ",
    "L’adresse à saisir dans Minecraft": "Minecraft में दर्ज करने का पता",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Xbox, PlayStation, Switch, फ़ोन, टैबलेट या Windows पर Minecraft Bedrock के लिए Bedrock चुनें।",
    "Le gamertag Xbox n’est pas un nom Java.":
      "Xbox गेमरटैग, Java उपयोगकर्ता नाम नहीं है।",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "अगर Floodgate कहता है कि खिलाड़ी मौजूद नहीं है, तो उसे पहले इस Geyser सर्वर से एक बार जुड़ना होगा। ऐप सुरक्षा को अचानक बंद नहीं करेगा।",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = PC/Mac पर Minecraft लॉन्चर। Bedrock = Xbox, PlayStation, Switch, फ़ोन, टैबलेट या Windows का Minecraft। चुने गए संस्करण का नाम दर्ज करें।",
  },
  es: {
    "Monde Ensemble": "Mundo en Compañía",
    "Mon serveur": "Mi servidor",
    "Préparer le serveur": "Preparar el servidor",
    "Préparer mon serveur": "Preparar mi servidor",
    "Préparer un serveur": "Preparar un servidor",
    Compatibilité: "Compatibilidad",
    "Ce qui peut jouer ensemble": "Quién puede jugar en conjunto",
    Sauvegardes: "Copias de seguridad",
    "Sauvegarder et déplacer": "Guardar y trasladar",
    "Réglages avancés": "Ajustes avanzados",
    "Mode débutant": "Modo principiante",
    "Mode avancé": "Modo avanzado",
    "TON SERVEUR": "TU SERVIDOR",
    "PREMIÈRE INSTALLATION": "PRIMERA INSTALACIÓN",
    "TON SERVEUR MINECRAFT": "TU SERVIDOR DE MINECRAFT",
    Version: "Versión",
    Langue: "Idioma",
    Installer: "Instalar",
    "Aucun serveur installé": "No hay ningún servidor instalado",
    "Le guide va partir de zéro": "La guía empezará desde cero",
    "Serveur enregistré": "Servidor guardado",
    "L’installation existante est conservée":
      "Se conserva la instalación existente",
    "Joueurs autorisés": "Jugadores autorizados",
    Édition: "Edición",
    "Java · PC et Mac": "Java · PC y Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · móvil, consola, Windows",
    "Nom Java": "Nombre de Java",
    "Gamertag Xbox": "Gamertag de Xbox",
    "Ex. Alex_12": "Ej. Alex_12",
    "Ex. Alex 12": "Ej. Alex 12",
    Ajouter: "Añadir",
    Retirer: "Quitar",
    "Liste active": "Lista activa",
    "Liste inactive": "Lista inactiva",
    "Serveur à démarrer": "Inicia el servidor primero",
    "Seuls les joueurs listés peuvent rejoindre.":
      "Solo pueden entrar los jugadores de la lista.",
    "Voir la réponse du serveur": "Ver respuesta del servidor",
    "Désactiver la liste d’accès": "Desactivar la lista de acceso",
    "Activer la liste d’accès": "Activar la lista de acceso",
    "Ajouter ou retirer un joueur": "Añadir o quitar jugadores",
    "L’adresse à saisir dans Minecraft":
      "Dirección que debes introducir en Minecraft",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Elige Bedrock para Xbox, PlayStation, Switch, móvil, tableta o Minecraft Bedrock en Windows.",
    "Le gamertag Xbox n’est pas un nom Java.":
      "El gamertag de Xbox no es un nombre de Java.",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "Si Floodgate dice que el jugador no existe, primero debe unirse una vez a este servidor Geyser. La app no desactivará la protección sin avisar.",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = el lanzador de Minecraft en PC/Mac. Bedrock = Xbox, PlayStation, Switch, móvil, tableta o Minecraft para Windows. Escribe el nombre de la edición elegida.",
  },
  ar: {
    "Monde Ensemble": "عالم للعب معًا",
    "Mon serveur": "سيرفري",
    "Préparer le serveur": "إعداد الخادم",
    "Préparer mon serveur": "إعداد خادمي",
    "Préparer un serveur": "إعداد خادم",
    Compatibilité: "التوافق",
    "Ce qui peut jouer ensemble": "من يمكنه اللعب معًا",
    Sauvegardes: "النسخ الاحتياطية",
    "Sauvegarder et déplacer": "نسخ احتياطي ونقل",
    "Réglages avancés": "إعدادات متقدمة",
    "Mode débutant": "وضع المبتدئ",
    "Mode avancé": "الوضع المتقدم",
    "TON SERVEUR": "خادمك",
    "PREMIÈRE INSTALLATION": "الإعداد الأول",
    "TON SERVEUR MINECRAFT": "خادم Minecraft الخاص بك",
    Version: "الإصدار",
    Langue: "اللغة",
    Installer: "تثبيت",
    "Aucun serveur installé": "لا يوجد خادم مثبت",
    "Le guide va partir de zéro": "سيبدأ الدليل من الصفر",
    "Serveur enregistré": "الخادم محفوظ",
    "L’installation existante est conservée": "تم الاحتفاظ بالتثبيت الحالي",
    "Joueurs autorisés": "اللاعبون المسموح لهم",
    Édition: "الإصدار",
    "Java · PC et Mac": "Java · الكمبيوتر وMac",
    "Bedrock · mobile, console, Windows": "Bedrock · الهاتف والمنصة وWindows",
    "Nom Java": "اسم Java",
    "Gamertag Xbox": "اسم لاعب Xbox",
    "Ex. Alex_12": "مثال Alex_12",
    "Ex. Alex 12": "مثال Alex 12",
    Ajouter: "إضافة",
    Retirer: "إزالة",
    "Liste active": "القائمة مفعلة",
    "Liste inactive": "القائمة متوقفة",
    "Serveur à démarrer": "ابدأ الخادم أولًا",
    "Seuls les joueurs listés peuvent rejoindre.":
      "يمكن للاعبين المدرجين فقط الانضمام.",
    "Voir la réponse du serveur": "عرض رد الخادم",
    "Désactiver la liste d’accès": "إيقاف قائمة السماح",
    "Activer la liste d’accès": "تفعيل قائمة السماح",
    "Ajouter ou retirer un joueur": "إضافة لاعب أو إزالته",
    "L’adresse à saisir dans Minecraft": "العنوان المطلوب إدخاله في Minecraft",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "اختر Bedrock لأجهزة Xbox وPlayStation وSwitch والهاتف واللوحي وMinecraft Bedrock على Windows.",
    "Le gamertag Xbox n’est pas un nom Java.": "اسم لاعب Xbox ليس اسم Java.",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "إذا أشار Floodgate إلى أن اللاعب غير موجود، فعليه الانضمام إلى خادم Geyser هذا مرة أولًا. لن يتم إيقاف الحماية دون تنبيه.",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = مشغّل Minecraft على الكمبيوتر أو Mac. Bedrock = Xbox وPlayStation وSwitch والهاتف واللوحي وMinecraft على Windows. أدخل اسم الإصدار المختار.",
  },
  bn: {
    "Monde Ensemble": "একসঙ্গে খেলার জগৎ",
    "Mon serveur": "আমার সার্ভার",
    "Préparer le serveur": "সার্ভার সেট আপ করুন",
    "Préparer mon serveur": "আমার সার্ভার সেট আপ করুন",
    "Préparer un serveur": "সার্ভার সেট আপ করুন",
    Compatibilité: "সামঞ্জস্য",
    "Ce qui peut jouer ensemble": "কারা একসঙ্গে খেলতে পারে",
    Sauvegardes: "ব্যাকআপ",
    "Sauvegarder et déplacer": "ব্যাকআপ ও স্থানান্তর",
    "Réglages avancés": "উন্নত সেটিংস",
    "Mode débutant": "শুরুর মোড",
    "Mode avancé": "উন্নত মোড",
    "TON SERVEUR": "আপনার সার্ভার",
    "PREMIÈRE INSTALLATION": "প্রথম সেটআপ",
    "TON SERVEUR MINECRAFT": "আপনার Minecraft সার্ভার",
    Version: "সংস্করণ",
    Langue: "ভাষা",
    Installer: "ইনস্টল করুন",
    "Aucun serveur installé": "কোনো সার্ভার ইনস্টল নেই",
    "Le guide va partir de zéro": "গাইডটি শূন্য থেকে শুরু হবে",
    "Serveur enregistré": "সার্ভার সংরক্ষিত",
    "L’installation existante est conservée": "আগের ইনস্টলেশন অক্ষত থাকবে",
    "Joueurs autorisés": "অনুমোদিত খেলোয়াড়",
    Édition: "সংস্করণ",
    "Java · PC et Mac": "Java · PC ও Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · মোবাইল, কনসোল, Windows",
    "Nom Java": "Java ব্যবহারকারীর নাম",
    "Gamertag Xbox": "Xbox গেমারট্যাগ",
    "Ex. Alex_12": "যেমন Alex_12",
    "Ex. Alex 12": "যেমন Alex 12",
    Ajouter: "যোগ করুন",
    Retirer: "সরান",
    "Liste active": "অনুমতি তালিকা চালু",
    "Liste inactive": "অনুমতি তালিকা বন্ধ",
    "Serveur à démarrer": "আগে সার্ভার চালু করুন",
    "Seuls les joueurs listés peuvent rejoindre.":
      "শুধু তালিকাভুক্ত খেলোয়াড়রা যোগ দিতে পারবেন।",
    "Voir la réponse du serveur": "সার্ভারের উত্তর দেখুন",
    "Désactiver la liste d’accès": "অনুমতি তালিকা বন্ধ করুন",
    "Activer la liste d’accès": "অনুমতি তালিকা চালু করুন",
    "Ajouter ou retirer un joueur": "খেলোয়াড় যোগ বা সরান",
    "L’adresse à saisir dans Minecraft": "Minecraft-এ দেওয়ার ঠিকানা",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Xbox, PlayStation, Switch, ফোন, ট্যাবলেট বা Windows-এর Minecraft Bedrock-এর জন্য Bedrock বেছে নিন।",
    "Le gamertag Xbox n’est pas un nom Java.":
      "Xbox গেমারট্যাগ Java ব্যবহারকারীর নাম নয়।",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "Floodgate খেলোয়াড়কে না চিনলে, তাকে আগে একবার এই Geyser সার্ভারে যোগ দিতে হবে। অ্যাপ হঠাৎ সুরক্ষা বন্ধ করবে না।",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = PC/Mac-এর Minecraft লঞ্চার। Bedrock = Xbox, PlayStation, Switch, ফোন, ট্যাবলেট বা Windows-এর Minecraft। নির্বাচিত সংস্করণের নাম লিখুন।",
  },
  pt: {
    "Monde Ensemble": "Mundo em Conjunto",
    "Mon serveur": "Meu servidor",
    "Préparer le serveur": "Configurar servidor",
    "Préparer mon serveur": "Configurar meu servidor",
    "Préparer un serveur": "Configurar um servidor",
    Compatibilité: "Compatibilidade",
    "Ce qui peut jouer ensemble": "Quem pode jogar junto",
    Sauvegardes: "Cópias de segurança",
    "Sauvegarder et déplacer": "Fazer backup e transferir",
    "Réglages avancés": "Configurações avançadas",
    "Mode débutant": "Modo iniciante",
    "Mode avancé": "Modo avançado",
    "TON SERVEUR": "SEU SERVIDOR",
    "PREMIÈRE INSTALLATION": "PRIMEIRA INSTALAÇÃO",
    "TON SERVEUR MINECRAFT": "SEU SERVIDOR MINECRAFT",
    Version: "Versão",
    Langue: "Idioma",
    Installer: "Instalar",
    "Aucun serveur installé": "Nenhum servidor instalado",
    "Le guide va partir de zéro": "O guia começa do zero",
    "Serveur enregistré": "Servidor salvo",
    "L’installation existante est conservée":
      "A instalação existente será preservada",
    "Joueurs autorisés": "Jogadores autorizados",
    Édition: "Edição",
    "Java · PC et Mac": "Java · PC e Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · celular, console, Windows",
    "Nom Java": "Nome Java",
    "Gamertag Xbox": "Gamertag Xbox",
    "Ex. Alex_12": "Ex.: Alex_12",
    "Ex. Alex 12": "Ex.: Alex 12",
    Ajouter: "Adicionar",
    Retirer: "Remover",
    "Liste active": "Lista ativa",
    "Liste inactive": "Lista inativa",
    "Serveur à démarrer": "Inicie o servidor primeiro",
    "Seuls les joueurs listés peuvent rejoindre.":
      "Somente jogadores listados podem entrar.",
    "Voir la réponse du serveur": "Ver resposta do servidor",
    "Désactiver la liste d’accès": "Desativar lista de acesso",
    "Activer la liste d’accès": "Ativar lista de acesso",
    "Ajouter ou retirer un joueur": "Adicionar ou remover jogador",
    "L’adresse à saisir dans Minecraft": "Endereço para inserir no Minecraft",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Escolha Bedrock para Xbox, PlayStation, Switch, celular, tablet ou Minecraft Bedrock no Windows.",
    "Le gamertag Xbox n’est pas un nom Java.":
      "O gamertag do Xbox não é um nome Java.",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "Se o Floodgate disser que o jogador não existe, ele precisa entrar neste servidor Geyser uma vez antes. O app não desativará a proteção sem avisar.",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = inicializador do Minecraft no PC/Mac. Bedrock = Xbox, PlayStation, Switch, celular, tablet ou Minecraft para Windows. Digite o nome da edição escolhida.",
  },
  ru: {
    "Monde Ensemble": "Мир Вместе",
    "Mon serveur": "Мой сервер",
    "Préparer le serveur": "Настроить сервер",
    "Préparer mon serveur": "Настроить мой сервер",
    "Préparer un serveur": "Настроить сервер",
    Compatibilité: "Совместимость",
    "Ce qui peut jouer ensemble": "Кто может играть вместе",
    Sauvegardes: "Резервные копии",
    "Sauvegarder et déplacer": "Создать копию и перенести",
    "Réglages avancés": "Расширенные настройки",
    "Mode débutant": "Режим новичка",
    "Mode avancé": "Расширенный режим",
    "TON SERVEUR": "ВАШ СЕРВЕР",
    "PREMIÈRE INSTALLATION": "ПЕРВАЯ УСТАНОВКА",
    "TON SERVEUR MINECRAFT": "ВАШ СЕРВЕР MINECRAFT",
    Version: "Версия",
    Langue: "Язык",
    Installer: "Установить",
    "Aucun serveur installé": "Сервер не установлен",
    "Le guide va partir de zéro": "Настройка начнётся с нуля",
    "Serveur enregistré": "Сервер сохранён",
    "L’installation existante est conservée": "Текущая установка сохранена",
    "Joueurs autorisés": "Разрешённые игроки",
    Édition: "Издание",
    "Java · PC et Mac": "Java · ПК и Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · телефон, консоль, Windows",
    "Nom Java": "Имя Java",
    "Gamertag Xbox": "Тег игрока Xbox",
    "Ex. Alex_12": "Например Alex_12",
    "Ex. Alex 12": "Например Alex 12",
    Ajouter: "Добавить",
    Retirer: "Удалить",
    "Liste active": "Список включён",
    "Liste inactive": "Список выключен",
    "Serveur à démarrer": "Сначала запустите сервер",
    "Seuls les joueurs listés peuvent rejoindre.":
      "Подключаться могут только игроки из списка.",
    "Voir la réponse du serveur": "Показать ответ сервера",
    "Désactiver la liste d’accès": "Выключить список доступа",
    "Activer la liste d’accès": "Включить список доступа",
    "Ajouter ou retirer un joueur": "Добавить или удалить игрока",
    "L’adresse à saisir dans Minecraft": "Адрес для Minecraft",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Выберите Bedrock для Xbox, PlayStation, Switch, телефона, планшета или Minecraft Bedrock в Windows.",
    "Le gamertag Xbox n’est pas un nom Java.": "Тег Xbox — это не имя Java.",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "Если Floodgate не находит игрока, ему нужно один раз подключиться к этому серверу Geyser. Защита не будет отключена без предупреждения.",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = программа запуска Minecraft на ПК/Mac. Bedrock = Xbox, PlayStation, Switch, телефон, планшет или Minecraft для Windows. Укажите имя выбранной версии.",
  },
  ur: {
    "Monde Ensemble": "ساتھ کھیلنے کی دنیا",
    "Mon serveur": "میرا سرور",
    "Préparer le serveur": "سرور سیٹ اپ کریں",
    "Préparer mon serveur": "میرا سرور سیٹ اپ کریں",
    "Préparer un serveur": "سرور سیٹ اپ کریں",
    Compatibilité: "مطابقت",
    "Ce qui peut jouer ensemble": "کون ساتھ کھیل سکتا ہے",
    Sauvegardes: "بیک اپ",
    "Sauvegarder et déplacer": "بیک اپ اور منتقل کریں",
    "Réglages avancés": "اعلیٰ ترتیبات",
    "Mode débutant": "ابتدائی موڈ",
    "Mode avancé": "اعلیٰ موڈ",
    "TON SERVEUR": "آپ کا سرور",
    "PREMIÈRE INSTALLATION": "پہلی تنصیب",
    "TON SERVEUR MINECRAFT": "آپ کا Minecraft سرور",
    Version: "ورژن",
    Langue: "زبان",
    Installer: "انسٹال کریں",
    "Aucun serveur installé": "کوئی سرور انسٹال نہیں",
    "Le guide va partir de zéro": "رہنما صفر سے شروع ہوگا",
    "Serveur enregistré": "سرور محفوظ ہے",
    "L’installation existante est conservée": "موجودہ تنصیب محفوظ ہے",
    "Joueurs autorisés": "اجازت یافتہ کھلاڑی",
    Édition: "ایڈیشن",
    "Java · PC et Mac": "Java · PC اور Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · موبائل، کنسول، Windows",
    "Nom Java": "Java نام",
    "Gamertag Xbox": "Xbox گیمر ٹیگ",
    "Ex. Alex_12": "مثلاً Alex_12",
    "Ex. Alex 12": "مثلاً Alex 12",
    Ajouter: "شامل کریں",
    Retirer: "ہٹائیں",
    "Liste active": "اجازت فہرست فعال",
    "Liste inactive": "اجازت فہرست غیر فعال",
    "Serveur à démarrer": "پہلے سرور شروع کریں",
    "Seuls les joueurs listés peuvent rejoindre.":
      "صرف فہرست کے کھلاڑی شامل ہو سکتے ہیں۔",
    "Voir la réponse du serveur": "سرور کا جواب دیکھیں",
    "Désactiver la liste d’accès": "اجازت فہرست بند کریں",
    "Activer la liste d’accès": "اجازت فہرست چالو کریں",
    "Ajouter ou retirer un joueur": "کھلاڑی شامل یا ہٹائیں",
    "L’adresse à saisir dans Minecraft": "Minecraft میں درج کرنے کا پتہ",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Xbox، PlayStation، Switch، فون، ٹیبلٹ یا Windows کے Minecraft Bedrock کے لیے Bedrock منتخب کریں۔",
    "Le gamertag Xbox n’est pas un nom Java.":
      "Xbox گیمر ٹیگ Java نام نہیں ہے۔",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "اگر Floodgate کھلاڑی کو نہ پہچانے تو اسے پہلے ایک بار اس Geyser سرور میں شامل ہونا ہوگا۔ تحفظ اچانک بند نہیں کیا جائے گا۔",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = PC/Mac پر Minecraft لانچر۔ Bedrock = Xbox، PlayStation، Switch، فون، ٹیبلٹ یا Windows کے لیے Minecraft۔ منتخب ایڈیشن کا نام درج کریں۔",
  },
  id: {
    "Monde Ensemble": "Dunia Bermain Bersama",
    "Mon serveur": "Server saya",
    "Préparer le serveur": "Siapkan server",
    "Préparer mon serveur": "Siapkan server saya",
    "Préparer un serveur": "Siapkan server",
    Compatibilité: "Kompatibilitas",
    "Ce qui peut jouer ensemble": "Siapa yang bisa bermain bersama",
    Sauvegardes: "Cadangan",
    "Sauvegarder et déplacer": "Cadangkan dan pindahkan",
    "Réglages avancés": "Pengaturan lanjutan",
    "Mode débutant": "Mode pemula",
    "Mode avancé": "Mode lanjutan",
    "TON SERVEUR": "SERVER ANDA",
    "PREMIÈRE INSTALLATION": "PENYIAPAN PERTAMA",
    "TON SERVEUR MINECRAFT": "SERVER MINECRAFT ANDA",
    Version: "Versi",
    Langue: "Bahasa",
    Installer: "Instal",
    "Aucun serveur installé": "Belum ada server yang terpasang",
    "Le guide va partir de zéro": "Panduan dimulai dari awal",
    "Serveur enregistré": "Server tersimpan",
    "L’installation existante est conservée": "Instalasi yang ada tetap aman",
    "Joueurs autorisés": "Pemain yang diizinkan",
    Édition: "Edisi",
    "Java · PC et Mac": "Java · PC dan Mac",
    "Bedrock · mobile, console, Windows": "Bedrock · ponsel, konsol, Windows",
    "Nom Java": "Nama Java",
    "Gamertag Xbox": "Gamertag Xbox",
    "Ex. Alex_12": "Contoh Alex_12",
    "Ex. Alex 12": "Contoh Alex 12",
    Ajouter: "Tambah",
    Retirer: "Hapus",
    "Liste active": "Daftar aktif",
    "Liste inactive": "Daftar nonaktif",
    "Serveur à démarrer": "Mulai server terlebih dahulu",
    "Seuls les joueurs listés peuvent rejoindre.":
      "Hanya pemain dalam daftar yang dapat bergabung.",
    "Voir la réponse du serveur": "Lihat jawaban server",
    "Désactiver la liste d’accès": "Nonaktifkan daftar akses",
    "Activer la liste d’accès": "Aktifkan daftar akses",
    "Ajouter ou retirer un joueur": "Tambah atau hapus pemain",
    "L’adresse à saisir dans Minecraft": "Alamat untuk dimasukkan di Minecraft",
    "Choisis Bedrock pour Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft Bedrock sur Windows.":
      "Pilih Bedrock untuk Xbox, PlayStation, Switch, ponsel, tablet, atau Minecraft Bedrock di Windows.",
    "Le gamertag Xbox n’est pas un nom Java.":
      "Gamertag Xbox bukan nama pengguna Java.",
    "Si Floodgate indique que le joueur n’existe pas, il doit d’abord avoir rejoint une fois ce serveur Geyser; la protection ne sera pas désactivée par surprise.":
      "Jika Floodgate tidak menemukan pemain, pemain harus bergabung ke server Geyser ini sekali terlebih dahulu. Aplikasi tidak akan mematikan perlindungan secara tiba-tiba.",
    "Java = launcher Minecraft sur PC/Mac. Bedrock = Xbox, PlayStation, Switch, téléphone, tablette ou Minecraft pour Windows. Utilise le nom de l’édition choisie.":
      "Java = peluncur Minecraft di PC/Mac. Bedrock = Xbox, PlayStation, Switch, ponsel, tablet, atau Minecraft untuk Windows. Masukkan nama edisi yang dipilih.",
  },
};

const rtlLocales = new Set<Locale>(["ar", "ur"]);
const serverStateCatalogs: Record<
  Exclude<Locale, "fr">,
  Record<string, string>
> = {
  en: {
    "Serveur prêt": "Server ready",
    "Connexion enregistrée": "Saved connection",
    "État confirmé en ligne": "Server confirmed online",
    "Serveur non confirmé en ligne": "Server not confirmed online",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "SERVER ONLINE · WORLD PROTECTED",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER":
      "SAVED CONNECTION · STATUS NEEDS CHECKING",
  },
  zh: {
    "Serveur prêt": "服务器已就绪",
    "Connexion enregistrée": "连接信息已保存",
    "État confirmé en ligne": "已确认服务器在线",
    "Serveur non confirmé en ligne": "尚未确认服务器在线",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "服务器在线 · 世界受保护",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER": "连接信息已保存 · 需检查状态",
  },
  hi: {
    "Serveur prêt": "सर्वर तैयार",
    "Connexion enregistrée": "कनेक्शन सहेजा गया",
    "État confirmé en ligne": "सर्वर ऑनलाइन होने की पुष्टि हुई",
    "Serveur non confirmé en ligne": "सर्वर ऑनलाइन होने की पुष्टि नहीं हुई",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "सर्वर ऑनलाइन · दुनिया सुरक्षित",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER": "कनेक्शन सहेजा गया · स्थिति जाँचें",
  },
  es: {
    "Serveur prêt": "Servidor listo",
    "Connexion enregistrée": "Conexión guardada",
    "État confirmé en ligne": "Servidor confirmado en línea",
    "Serveur non confirmé en ligne": "Servidor no confirmado en línea",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "SERVIDOR EN LÍNEA · MUNDO PROTEGIDO",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER":
      "CONEXIÓN GUARDADA · HAY QUE COMPROBAR EL ESTADO",
  },
  ar: {
    "Serveur prêt": "الخادم جاهز",
    "Connexion enregistrée": "تم حفظ الاتصال",
    "État confirmé en ligne": "تم تأكيد أن الخادم متصل",
    "Serveur non confirmé en ligne": "لم يتم تأكيد اتصال الخادم",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "الخادم متصل · العالم محمي",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER":
      "تم حفظ الاتصال · تحقق من الحالة",
  },
  bn: {
    "Serveur prêt": "সার্ভার প্রস্তুত",
    "Connexion enregistrée": "সংযোগ সংরক্ষিত",
    "État confirmé en ligne": "সার্ভার অনলাইন নিশ্চিত হয়েছে",
    "Serveur non confirmé en ligne": "সার্ভার অনলাইন নিশ্চিত নয়",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "সার্ভার অনলাইন · জগৎ সুরক্ষিত",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER":
      "সংযোগ সংরক্ষিত · অবস্থা যাচাই করুন",
  },
  pt: {
    "Serveur prêt": "Servidor pronto",
    "Connexion enregistrée": "Conexão salva",
    "État confirmé en ligne": "Servidor confirmado online",
    "Serveur non confirmé en ligne": "Servidor ainda não confirmado online",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "SERVIDOR ONLINE · MUNDO PROTEGIDO",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER":
      "CONEXÃO SALVA · VERIFIQUE O ESTADO",
  },
  ru: {
    "Serveur prêt": "Сервер готов",
    "Connexion enregistrée": "Подключение сохранено",
    "État confirmé en ligne": "Работа сервера подтверждена",
    "Serveur non confirmé en ligne": "Работа сервера не подтверждена",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "СЕРВЕР В СЕТИ · МИР ЗАЩИЩЁН",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER":
      "ПОДКЛЮЧЕНИЕ СОХРАНЕНО · ПРОВЕРЬТЕ СОСТОЯНИЕ",
  },
  ur: {
    "Serveur prêt": "سرور تیار ہے",
    "Connexion enregistrée": "کنکشن محفوظ ہے",
    "État confirmé en ligne": "سرور کے آن لائن ہونے کی تصدیق ہوگئی",
    "Serveur non confirmé en ligne": "سرور کے آن لائن ہونے کی تصدیق نہیں ہوئی",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "سرور آن لائن · دنیا محفوظ",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER": "کنکشن محفوظ · حالت چیک کریں",
  },
  id: {
    "Serveur prêt": "Server siap",
    "Connexion enregistrée": "Koneksi tersimpan",
    "État confirmé en ligne": "Server dipastikan online",
    "Serveur non confirmé en ligne": "Server belum dipastikan online",
    "SERVEUR EN LIGNE · MONDE PROTÉGÉ": "SERVER ONLINE · DUNIA TERLINDUNGI",
    "CONNEXION ENREGISTRÉE · ÉTAT À VÉRIFIER":
      "KONEKSI TERSIMPAN · PERIKSA STATUS",
  },
};

function keyOf(text: string) {
  return text.trim().replace(/\s+/g, " ");
}

export function translate(text: string, locale: Locale): string {
  if (locale === "fr") return text;
  const match = text.match(/^(\s*)([\s\S]*?)(\s*)$/);
  if (!match) return text;
  const [, leading, content, trailing] = match;
  const key = keyOf(content);
  const translated = catalogs[locale][key] ?? serverStateCatalogs[locale][key];
  return translated ? `${leading}${translated}${trailing}` : text;
}

function localizeNode(node: ReactNode, locale: Locale): ReactNode {
  if (typeof node === "string") return translate(node, locale);
  if (Array.isArray(node))
    return node.map((child) => localizeNode(child, locale));
  if (!isValidElement<{ children?: ReactNode }>(node)) return node;

  const props: Record<string, unknown> = {};
  if (node.props.children !== undefined) {
    props.children = localizeNode(node.props.children, locale);
  }
  for (const attribute of ["placeholder", "title", "aria-label"] as const) {
    const value = (node.props as Record<string, unknown>)[attribute];
    if (typeof value === "string") props[attribute] = translate(value, locale);
  }
  return cloneElement(node, props);
}

export function LocalizedTree({
  locale,
  children,
}: {
  locale: Locale;
  children: ReactNode;
}) {
  return <>{localizeNode(children, locale)}</>;
}

export function localeDirection(locale: Locale): "ltr" | "rtl" {
  return rtlLocales.has(locale) ? "rtl" : "ltr";
}
