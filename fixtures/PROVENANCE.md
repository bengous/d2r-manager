# Provenance du corpus

Fige le 2026-08-20. Ne pas modifier ces fichiers : regenerer via un nouveau gel.

## Publication

Ces fichiers sont les anciennes sauvegardes hors-ligne personnelles d'Augustin
Bengolea. Leur publication dans ce depot de test est volontaire. Ils contiennent
des noms de personnages, des inventaires et des metadonnees generes par le jeu.

La licence MIT du depot couvre son code et sa documentation originaux. Diablo II:
Resurrected et les donnees du jeu restent la propriete de Blizzard Entertainment
et de ses ayants droit.

| Dossier | Source | Etat source |
|---|---|---|
| `v105/linux/` | depot prive `d2r-saves`, `git archive 33162cd -- saves`, filtre sur `.d2s` et `.d2i` | commit `33162cd` |
| `v105/windows/` | partition NTFS Windows montee en lecture seule | fichiers du 2026-02 |
| `legacy/` | idem NTFS ; versions 98 et 99, hors perimetre v105 | fixtures du chemin d'erreur |

Empreintes : `sha256sums.txt`. Verification : `cd fixtures && sha256sum -c sha256sums.txt`.
