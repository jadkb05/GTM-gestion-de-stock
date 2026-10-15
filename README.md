# GTM Stock Manager

Application de bureau pour la gestion de stock d'une entreprise de génie civil.
Cette version couvre le catalogue, les entrées et les sorties de stock, l'historique
des mouvements, la fiche de stock, l'inventaire physique et les alertes de
réapprovisionnement.

L'application fonctionne hors ligne après installation. Chaque poste possède sa
propre base SQLite. Il n'y a pas de serveur, de compte utilisateur, ni de service externe.

## Pile technique

- Tauri 2
- React 19 et TypeScript strict
- Rust
- SQLite, embarqué via `rusqlite` (`bundled`)
- Tailwind CSS

Le frontend n'exécute pas de SQL. Il appelle des commandes Tauri explicites.
Les requêtes SQL utilisent des paramètres liés. Les écritures du catalogue
passent par une transaction.

## Prérequis

Vérifiés sur Ubuntu 24.04 :

- Node.js 24 et npm 11
- Rust stable (testé avec 1.99.0), installé avec [rustup](https://rustup.rs)
- Paquets système Linux, d'après la [documentation Tauri](https://v2.tauri.app/start/prerequisites/) :

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Sur Windows, pour développer ou produire l'installateur :

- Node.js
- Rust via rustup
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) avec la charge de travail « Développement Desktop en C++ »
- WebView2, déjà présent sur Windows 10 et 11 à jour

`cargo` doit être dans le `PATH`. Avec rustup :

```bash
source "$HOME/.cargo/env"
```

## Développement

```bash
npm install
npm run tauri dev
```

`npm run dev` lance seulement l'interface Vite. Sans la coque Tauri, la base
locale ne peut pas s'ouvrir : l'écran affiche alors l'erreur correspondante.

## Tests et vérifications

```bash
npm test
npm run test:db
npm run typecheck
npm run build
```

- `npm test` : interface (Vitest)
- `npm run test:db` : base SQLite, migrations et règles du catalogue
  (crate `gtm-db` seule, sans les bibliothèques GTK/WebKit)
- `npm run typecheck` : TypeScript
- `npm run build` : compilation du frontend

`src-tauri` est un espace de travail Cargo dont `crates/gtm-db` est membre :
les deux crates partagent `src-tauri/Cargo.lock`, celui de l'application
livrée. Depuis `src-tauri`, `cargo test` lance donc aussi les tests de
`gtm-db`. `--locked` fait échouer la commande si ce fichier devait changer.

La coque Tauri se vérifie avec `cargo check --manifest-path src-tauri/Cargo.toml`.
Sur cette machine Linux, la compilation GTK/WebKit utilise les en-têtes extraits
localement ; ils ne font pas partie du projet.

## Base de données

Le fichier n'est pas placé à un chemin absolu fixé dans le code. Au démarrage,
Tauri fournit le répertoire de données de l'installation, puis l'application
crée `gtm-stock.db` dedans.

- Linux : `~/.local/share/com.gtm.stock-manager/gtm-stock.db`
  (`$XDG_DATA_HOME` remplace `~/.local/share` s'il est défini)
- Windows : `%APPDATA%\com.gtm.stock-manager\gtm-stock.db`

À la première ouverture, l'application crée le répertoire si besoin, ouvre la
base et applique les migrations manquantes. Chaque migration est une transaction.
La table `schema_migrations` enregistre la version.

- Migration `1` : `app_metadata`
- Migration `2` : types de produits, fournisseurs et produits
- Migration `3` : soldes et mouvements de stock
- Migration `4` : inventaires physiques et seuils de réapprovisionnement

`app_metadata.schema_phase` vaut `4`. Une base déjà en version 3 reçoit la
migration 4 : les mouvements existants restent des entrées ou des sorties
(`source` vaut `manual`), et les produits n'ont pas de seuil tant qu'il n'est
pas défini. Une base plus récente que l'application
est refusée, sans être modifiée : le mode journal n'est activé qu'après ce contrôle.

### Catalogue

Un produit a un nom obligatoire, une référence facultative, un type, un
fournisseur principal facultatif et une description facultative. Il peut être
désactivé. Il n'est pas supprimé. Cette phase ne stocke ni prix, ni TVA, ni
unité commerciale.

La liste affiche 200 produits, puis 200 de plus à la demande ; la recherche
part après une courte pause de frappe et porte sur tout le catalogue.

Règle d'unicité de la référence : les espaces en trop sont retirés. Une
référence vide ou composée uniquement d'espaces est enregistrée comme absence
de référence, et plusieurs produits peuvent ne pas en avoir. Une référence
renseignée est unique sans tenir compte de la casse. Les noms de produits
peuvent se répéter. Les noms de types et de fournisseurs sont uniques sans
tenir compte de la casse, après suppression des espaces en trop.

Un produit nouveau doit référencer un type actif. Un fournisseur désactivé ne
peut pas être affecté à un nouveau produit. La désactivation d'un type ou d'un
fournisseur conserve les produits déjà liés, y compris leur type et leur
fournisseur. Modifier un produit peut conserver cette affectation historique.

Types préchargés : Silicone, Acrylique, Autre. Fournisseurs préchargés : Selena,
Tremco, Legallais. « Vissable » et « Tenois » ne sont pas insérés : leur
orthographe n'est pas confirmée. L'interface permet de les ajouter.

## Commandes exposées

`get_app_status`, `list_products`, `get_product`, `create_product`,
`update_product`, `set_product_active`, `list_product_types`,
`create_product_type`, `update_product_type`, `set_product_type_active`,
`list_suppliers`, `create_supplier`, `update_supplier`, `set_supplier_active`,
`list_stock_levels`, `get_product_stock`, `record_stock_entry`,
`record_stock_exit`, `list_stock_movements`, `get_stock_movement`, `get_stock_sheet`,
`list_stock_sheet_movements`, `get_stock_overview`, `list_inventories`, `create_inventory`,
`get_inventory`, `add_inventory_products`, `save_inventory_counts`,
`check_inventory_drift`, `refresh_inventory_line`, `remove_inventory_line`,
`validate_inventory`, `list_reorder_alerts`, `set_reorder_threshold`,
`count_reorder_alerts`, `export_catalogue`, `export_history`,
`export_inventory`, `export_reorder_alerts`, `preview_article_import`,
`commit_article_import`, `get_backup_status`, `create_backup`,
`inspect_backup`, `restore_backup`.

Aucune commande n'accepte de SQL arbitraire.

### Stock

Le stock disponible est une table de soldes, `stock_balances`. Chaque mouvement
est une ligne de `stock_movements`. Le solde et le mouvement sont écrits dans
la même transaction immédiate. Un produit créé reçoit un solde à zéro grâce à
un déclencheur. Une entrée augmente le solde. Une sortie le diminue et est
refusée si elle le rendrait négatif. Un produit désactivé ne reçoit plus de
mouvement ; son solde et son historique restent consultables. Un mouvement
validé n'est pas supprimé.

Les quantités sont des entiers de millièmes (`quantity_milli`). Une unité vaut
1 000. La précision maximale est de trois décimales. La quantité maximale est
1 000 000 000. Les valeurs nulles, négatives, non numériques ou à plus de trois
décimales sont refusées. Le calcul ne passe pas par des nombres binaires à
virgule flottante. L'interface accepte le point ou la virgule. Aucune unité
métier (pièce, litre, kilogramme) n'est enregistrée : une colonne d'unité
pourra être ajoutée plus tard sans convertir l'historique.

L'historique général affiche au plus 200 mouvements, du plus récent au plus ancien.
Cette limite ne s'applique pas à la fiche d'un produit.

### Fiche de stock

Chaque produit a une fiche consultable depuis le catalogue et depuis l'historique,
y compris s'il est désactivé. Le stock actuel, le nombre de mouvements, le total
des entrées et le total des sorties sont calculés sur l'historique complet.

Le solde après chaque mouvement est la somme signée, en millièmes, de tous les
mouvements du produit jusqu'à celui-ci. L'ordre est la date puis l'identifiant.
Le stock initial est zéro, comme à la création du produit. Les filtres et la
pagination, par pages de 50, n'entrent pas dans ce calcul : ils ne font que
choisir les lignes affichées. Aucune migration supplémentaire n'est nécessaire.
Un mouvement validé ne peut être ni modifié ni supprimé.

Une correction d'inventaire est un mouvement `in` ou `out` dont `source` vaut
`inventory`. La fiche la désigne comme « Correction d'inventaire ».

### Inventaire physique

Un inventaire est un brouillon, puis un inventaire validé. Il n'est pas
supprimé. Le stock théorique de référence est copié depuis le solde au moment
où le produit est ajouté à l'inventaire. Les mouvements de stock restent
autorisés pendant le comptage. Si le solde change ensuite, la validation est
refusée : il faut réactualiser explicitement la référence, ou reprendre le
comptage. L'écart est `quantité comptée − stock de référence`, en millièmes.
Une quantité comptée nulle est acceptée. Un écart nul ne crée pas de mouvement.
Un surplus crée une entrée, un manque une sortie, liés à l'inventaire. Toutes
les corrections d'une validation sont dans la même transaction : si l'une
échoue, aucune n'est conservée. Un inventaire validé ne peut plus être modifié
ni validé une seconde fois.

### Réapprovisionnement

Le seuil facultatif est `products.reorder_threshold_milli`, en millièmes. `NULL`
signifie que le produit n'est pas surveillé. Zéro est un seuil valide. L'alerte
n'est pas stockée : un produit actif est en alerte lorsque son solde est
inférieur ou égal au seuil. Un produit désactivé conserve son seuil mais
n'apparaît pas dans les alertes actives.

### Tableau de bord

`get_stock_overview` calcule les indicateurs sur la base locale. Un produit
actif dont le solde vaut zéro est en rupture. Un produit actif dont le seuil
est défini et dont le solde est inférieur ou égal à ce seuil est à
réapprovisionner. Les deux comptes sont distincts : un article sous le seuil
peut encore avoir du stock. Les produits désactivés sont exclus des deux.
L'activité récente compte les mouvements des sept derniers jours. La liste
affiche les huit derniers mouvements, du plus récent au plus ancien. Aucune
donnée n'est inventée.

### Exports et import

Les exports lisent la base au moment de l'enregistrement. Ils ne la modifient pas.
Le CSV est en UTF-8, avec une marque d'ordre des octets, le point-virgule comme
séparateur et la virgule comme séparateur décimal. Un texte qui commence par
`=`, `+`, `-` ou `@` est préfixé pour ne pas être lu comme une formule, sauf
s'il s'agit d'une quantité. Le classeur Excel contient une feuille de données
et une feuille « Périmètre ». Les quantités y sont des nombres, calculés
depuis les millièmes entiers (au plus trois décimales, valeur exacte) :
le tableur peut les additionner. Les références restent du texte. Le PDF est
un état paginé, sans valeur de facturation : chaque cellule y est écrite en
entier, avec retour à la ligne dans sa colonne. Au-delà de 2 000 lignes, le PDF est refusé au profit du CSV ou
d'Excel. L'historique exporté n'est pas limité aux 200 lignes de l'écran ni
aux huit mouvements du tableau de bord.

L'import CSV ou Excel ajoute des articles après aperçu et confirmation.
Il n'écrit rien si une ligne est refusée. Il ne crée pas de type ni de
fournisseur, ne remplace pas un article existant et n'enregistre aucun
mouvement de stock. Un stock présent dans le fichier bloque la ligne.
L'aperçu note une empreinte du fichier lu ; si le fichier change avant la
confirmation, l'import est refusé et rien n'est enregistré.

### Sauvegarde

La sauvegarde utilise l'API de sauvegarde SQLite. Elle produit un fichier
autonome et cohérent, même lorsque le journal WAL est actif. Le fichier porte
l'extension `.gtm-backup`. Avant d'être accepté, il passe `PRAGMA integrity_check`
et `PRAGMA foreign_key_check`, puis le contrôle de version du schéma. Une
sauvegarde plus récente que l'application est refusée. L'inspection ne migre
pas le fichier choisi.

La restauration demande une confirmation. Elle enregistre d'abord un point de
retour, `gtm-stock.rollback.db`, à côté de la base locale, puis remplace la
base. Si le remplacement échoue, la base précédente est remise en place. Si
l'application s'arrête après avoir déplacé la base et avant d'installer la
sauvegarde, le démarrage suivant remet le fichier précédent en place. Une
base déjà incohérente n'empêche pas d'installer une sauvegarde qui a passé
les contrôles. Les copies restent sur le même ordinateur : un disque perdu
emporte aussi les sauvegardes qui y sont rangées.

## Compilation

```bash
npm run tauri build
```

Le binaire et les paquets sont produits pour le système sur lequel la commande
est lancée. Sur Windows, avec les Build Tools Microsoft, cette commande produit
l'installateur NSIS `GTM Stock Manager_0.1.0_x64-setup.exe`. L'identifiant
reste `com.gtm.stock-manager`. L'installation se fait pour l'utilisateur
courant, sans élévation. L'installateur est en français. Il ne télécharge pas
WebView2 : ce composant est déjà fourni par Windows 10 et 11 à jour. Il n'est
pas signé : aucun certificat n'est configuré, et Windows SmartScreen peut
afficher un avertissement.

La base n'est pas installée dans le dossier du programme. Au premier lancement,
Tauri ouvre `%APPDATA%\com.gtm.stock-manager\` et l'application y crée
`gtm-stock.db`. Une mise à jour remplace les fichiers du programme et laisse
ce dossier en place. La désinstallation retire le programme, le raccourci et
l'entrée de désinstallation. Elle ne supprime ni la base, ni
`gtm-stock.rollback.db`, ni une sauvegarde `.gtm-backup` enregistrée ailleurs.
Une version plus ancienne ne s'installe pas par-dessus une version plus récente.

Cette compilation Windows n'a pas été exécutée ici. Le poste est Ubuntu, sans
cible Rust Windows, sans MinGW, sans NSIS, sans Wine et sans machine Windows.
`sudo` demande un mot de passe, donc ces outils n'ont pas été installés.

Pour produire et vérifier l'installateur sur Windows :

```bash
npm install
npm test
npm run test:db
npm run typecheck
npm run tauri build
```

Puis lancer l'application installée et confirmer que le tableau de bord
affiche les indicateurs calculés, la base initialisée, la version de schéma 4,
et un fichier sous `%APPDATA%\com.gtm.stock-manager\`. Fermer, rouvrir, créer
une sauvegarde, désinstaller, puis réinstaller : la base et la sauvegarde
doivent être encore là.

## Limites actuelles

- Pas de prix, de TVA ni d'unité commerciale. Les conditionnements, unités et
  prix d'achat par produit sont prévus : voir [ROADMAP.md](ROADMAP.md).
- Pas de commande fournisseur, de multi-entrepôts, ni de facturation.
- Pas de synchronisation, de compte utilisateur, ni d'e-mail.
- La sauvegarde est manuelle et locale : pas de copie distante, pas de planification, et pas de protection si toutes les copies sont sur le même disque.
- Pas de comptes utilisateur.
- L'icône de fenêtre est un aplat neutre, en l'absence de logo GTM.
- La compilation Linux exige les bibliothèques WebKitGTK et GTK de développement.
  Sans elles, `cargo` s'arrête sur `glib-2.0` / `gobject-2.0` introuvables.
- L'installateur Windows est configuré, mais il n'a pas été produit ni essayé : cet environnement Ubuntu ne peut pas le compiler.
