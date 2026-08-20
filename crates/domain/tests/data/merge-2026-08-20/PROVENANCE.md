# Fusion de coffres du 2026-08-20

Etat du coffre partage Linux avant et apres la fusion faite en jeu le
2026-08-20, avec un personnage porteur et un echange temporaire du fichier de
coffre (methode decrite dans `d2r-saves:docs/migration-windows-2026-08-20.md`).

La source de la fusion est deja dans le corpus fige :
`fixtures/v105/windows/ModernSharedStashSoftCoreV2.d2i`.

| Fichier | Origine | sha256 |
|---|---|---|
| `before.d2i` | `d2r-saves` `2885c71^` (`7d6a611`), `saves/ModernSharedStashSoftCoreV2.d2i` | `52355bee8e1319dd309c393fd0a36bfed5f590cd103dc4cea67749ecfeec1c21` |
| `after.d2i` | `d2r-saves` `2885c71`, `saves/ModernSharedStashSoftCoreV2.d2i` | `41aabb48b1248796d53178526074b33abd2415fafaf743636b3e3c1d743603ef` |

Extraction, depuis un depot `d2r-saves` en lecture seule :

```sh
git -C ~/Work/d2r-saves show 2885c71^:saves/ModernSharedStashSoftCoreV2.d2i > before.d2i
git -C ~/Work/d2r-saves show  2885c71:saves/ModernSharedStashSoftCoreV2.d2i > after.d2i
```

`after.d2i` differe de `fixtures/v105/linux/ModernSharedStashSoftCoreV2.d2i` :
le corpus a ete gele plus tard, apres d'autres parties.

Ces deux fichiers ne sont pas dans `fixtures/` parce que le corpus y est fige et
en lecture seule (PLAN 9.2). Ils appartiennent au lot 3 et servent au seul test
de rejeu de la fusion (PLAN 8, etape 5).
