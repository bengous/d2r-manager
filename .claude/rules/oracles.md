---
paths:
  - "tools/**"
  - "goldens/**"
---

# Oracles et goldens

- d2rsavegameparser (LGPL 2.1) : jar execute et `txt/` seulement ; ne jamais
  ouvrir ni porter `reference/d2rsavegameparser/src/` (PLAN §2, §9.3).
- Jar telecharge par script avec verification sha256 obligatoire (PLAN §4),
  jamais commite.
- JSON canonique : cles triees, indente (PLAN §6.1).
- Axes du parseur Java inverses : normaliser toutes les sorties sur la
  convention D2SSharp, `InvX` = colonne (PLAN §6).
- Desaccords C#/Java listes dans `goldens/REPORT.md`, jamais masques.
- Corpus = 60 fichiers ; `v105/windows/` en section a part du REPORT, hors
  criteres 55/55 ; `Settings.json` hors corpus (PLAN §6.3).
