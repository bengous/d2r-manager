---
paths:
  - "crates/**"
---

# Qualite Rust

- Etats invalides irrepresentables partout ou une donnee a plus d'une forme
  legitime : enums a la place de bools correles, newtypes pour les primitives
  porteuses de regles, typestate pour les operations ordonnees.
- Propriete et emprunts precis : les API prennent ce qu'elles consomment ;
  pas de clone de confort.
- `thiserror` a variantes precises dans les bibliotheques ; `anyhow` seulement
  dans les binaires `cli` et `server`.
- `format`, `domain` et `app` restent purs : zero I/O, horloge, processus.
  Chaque crate pure porte son propre `clippy.toml` `disallowed-types` ; une
  nouvelle crate pure sans ce fichier n'est gardee par rien.
- `dead_code` en deny : une API publique sans appelant se supprime.
