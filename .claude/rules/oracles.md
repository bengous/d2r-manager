---
paths:
  - "tools/**"
  - "goldens/**"
---

# Oracles et goldens

- d2rsavegameparser (LGPL 2.1) : jar execute et `txt/` seulement ; ne jamais
  ouvrir ni porter `reference/d2rsavegameparser/src/`. D2SSharp et halbu sont
  MIT, le portage y est permis.
- Jar telecharge par `tools/goldens-java/fetch-jars.sh` avec verification
  sha256 obligatoire, jamais commite.
- JSON canonique : cles triees, indente. `just goldens` doit etre idempotent.
- Le dumper Java echange x et y par rapport a D2SSharp : normaliser toutes les
  sorties sur la convention du modele, decrite dans
  `.claude/rules/bitstream.md`.
- Desaccords C#/Java listes dans `goldens/REPORT.md`, jamais masques.
- Le corpus est fige : `fixtures/sha256sums.txt` en donne la liste et le
  contenu exact. Les fichiers `v105/windows/` vont dans une section a part du
  REPORT, hors des criteres de concordance de `v105/linux/`.
