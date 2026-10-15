# Feuille de route — GTM Stock Manager

Document de suivi des phases à venir. Il décrit des besoins et des questions
ouvertes ; il ne fixe aucune décision technique tant que la phase concernée
n'a pas été conçue et validée.

Le produit n'est pas terminé. Des tests automatisés qui passent ne suffisent
pas à le déclarer sans défaut ni prêt pour la vente.

## Ordre des phases

1. **Stabilisation** (en cours) : corrections vérifiées, tests, dépendances,
   performances. Pas de nouvelle fonction.
2. **Recherche fonctionnelle** : étude des logiciels concurrents et des
   améliorations utiles à GTM Bâtiment.
3. **Conditionnements, unités et prix d'achat** : conception puis
   réalisation (voir ci-dessous).
4. **Sécurité et fiabilité du backend** : renforcement complémentaire.
5. **Interface** : finitions professionnelles et ergonomie.
6. **Version Windows** : production et essai de l'installateur.

Hors périmètre actuel : synchronisation cloud, application mobile,
multi-entrepôts, service backend distant.

## Phase 3 — Conditionnements, unités et prix d'achat par produit

Besoin exprimé par l'utilisateur prévu du logiciel chez GTM Bâtiment.
**À ne pas réaliser avant la fin de la stabilisation et de la recherche
fonctionnelle.**

### Besoin

Chaque produit a une unité de stock de base, et l'utilisateur crée ses
propres conditionnements pour ce produit, chacun avec la quantité qu'il
contient dans l'unité de base et un prix d'achat.

Exemple (valeurs indicatives) :

- Référence XX1, désignation « Level Uni », fournisseur STO, unité de base : sac
- 1 sac = 1 sac
- 1 palette = 1 000 sacs
- 1 demi-palette = 500 sacs

Un autre produit peut utiliser la cartouche, le carton de 25, la palette, le
kilogramme, le litre, le mètre, ou toute autre unité définie par
l'utilisateur. Aucune liste imposée n'est commune à tous les produits.

Parcours attendu :

1. Créer ou modifier un produit.
2. Définir son unité de stock de base.
3. Créer un ou plusieurs conditionnements.
4. Indiquer la quantité contenue dans chacun.
5. Indiquer le prix d'achat de chacun.
6. Choisir un conditionnement lors d'une entrée de stock.
7. Convertir automatiquement la quantité dans l'unité de base.
8. Enregistrer le mouvement correspondant.
9. Conserver les prix d'achat et les mouvements passés.

Exemple : 1 palette = 1 000 sacs ; une entrée de 3 palettes augmente le
stock de 3 000 sacs. La conversion doit être exacte et validée.

Le prix d'achat ne doit être confondu ni avec la valorisation du stock ni
avec un prix de vente. Un changement de prix ne doit pas réécrire les
mouvements déjà enregistrés.

### État actuel du modèle (schéma, migration 4)

- `products` : nom, référence, type, fournisseur, description, actif. Aucune
  unité, aucun prix.
- `stock_movements` : sens, `quantity_milli` (entier, millièmes), motif,
  référence documentaire, commentaire, date. Aucune trace d'un
  conditionnement ou d'un prix.
- `stock_balances` : solde par produit en millièmes.
- Les quantités ont donc trois décimales au plus, et
  `MAX_MILLI` = 10^12 millièmes.

### Questions à trancher lors de la conception

À étudier, sans rien supposer d'avance :

- Faut-il une nouvelle migration (table de conditionnements, informations
  ajoutées aux mouvements) ? Comment migrer les produits existants sans
  perte, et quelle unité de base leur attribuer par défaut ?
- Précision : un facteur de conversion ou une quantité convertie qui dépasse
  trois décimales (par exemple 1 carton = 1/3 d'une unité) doit-il être
  refusé, arrondi, ou faut-il changer d'échelle ?
- Historique : quelles informations du conditionnement (nom, quantité
  contenue, prix d'achat) faut-il figer dans chaque mouvement pour qu'une
  modification ultérieure ne change pas le passé ?
- Prix d'achat : monnaie, hors taxe ou toutes taxes, précision, historique
  des prix. Aucune règle comptable (TVA, valorisation, coût moyen) ne doit
  être ajoutée sans confirmation de l'utilisateur.
- Effets sur les sorties, l'inventaire, les seuils d'alerte, les imports et
  les exports.

## Points ouverts issus de la stabilisation

Voir le rapport de stabilisation du 15/10/2026 : comportements non vérifiés
sur Windows et sur le bureau Wayland de l'utilisateur, avertissements de la
pile GTK de Tauri sous Linux, et autres risques restants.
