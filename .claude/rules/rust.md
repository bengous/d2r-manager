---
paths:
  - "crates/**"
---

# Qualite Rust

- Concevoir les types et structures de donnees avant le code : viser le
  meilleur Rust possible, pas le premier qui compile.
- Etats invalides irrepresentables partout ou une donnee a plus d'une forme
  legitime : enums a la place de bools correles, newtypes pour les primitives
  porteuses de regles, typestate pour les operations ordonnees.
- Le skill `unrepresentable` est disponible via l'outil Skill : l'invoquer a
  la conception d'un nouveau type ou d'une signature publique a formes
  multiples. Usage selon la tache.
- Propriete et emprunts precis : les API prennent ce qu'elles consomment ;
  pas de clone de confort.
- `thiserror` a variantes precises dans les bibliotheques ; `anyhow` seulement
  dans les binaires `cli` et `server` (PLAN §3.4).
- `format`, `domain` et `app` restent purs : zero I/O, horloge, processus.
  Chaque crate pure porte son propre `clippy.toml` `disallowed-types` ; une
  nouvelle crate pure sans ce fichier n'est gardee par rien.
- `dead_code` en deny : une API publique sans appelant se supprime.
