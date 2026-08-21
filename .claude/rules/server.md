---
paths:
  - "crates/server/**"
  - "docs/openapi.json"
---

# API HTTP locale

- `docs/openapi.json` est le contrat gele : il fige l'interface pour une GUI
  future. Toute route, propriete, parametre de query ou statut ajoute au
  serveur se declare dans le contrat, et l'inverse.
- `tests/contract.rs` oppose le contrat au serveur : noms, types, requiredness,
  schemas imbriques, parametres de query, mots-cles inline et statuts. Quand il
  echoue, on rapproche le garde du contrat, jamais le contrat du garde.
- Les noms de parametres vivent dans la table `ROUTES` : un nom ajoute cote
  serveur sans le contrat fait echouer la suite. C'est voulu.
- Ecoute loopback seulement. Un `Host` etranger rend 403 : c'est une mesure
  anti-reliaison DNS, pas une authentification.
- Aucune operation que la CLI n'expose pas ; la parite est verrouillee dans les
  deux sens par `x-cli` et `cli/tests/commands.rs`. Pas de creation d'objet,
  rien qui contourne `domain` : ce projet n'est pas un editeur de personnage.
- Erreurs typees par famille metier ; jamais de 500 pour un refus de regle.
- Le serveur traite une requete a la fois : deux `apply` simultanes sont
  serialises. Aucun verrou n'est pris, voir `docs/DECISIONS.md`.
