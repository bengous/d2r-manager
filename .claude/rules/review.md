# Protocole de review de lot

- L'implementeur ne se declare fini qu'apres : `just check` et `just test`
  verts, criteres du lot verifies un a un, rebase sur main.
- Review par un agent Opus a contexte frais (outil Agent,
  `subagent_type: "claude"`, `model: "opus"`), spawne par l'implementeur.
- Entrees du reviewer : brief du lot verbatim, § de PLAN.md concernes,
  `git diff main...feature/<lot>`, liste des criteres d'acceptation. Jamais
  l'auto-evaluation de l'implementeur.
- Mandat du reviewer : verifier en executant (relancer les commandes,
  recompter, sonder la spec contre le code et l'oracle), pas en lisant des
  affirmations.
- Sortie : verdict par critere ; findings bloquant / majeur / mineur, chacun
  avec preuve (commande + sortie, ou fichier:ligne).
- Bloquants et majeurs corriges puis gates re-passes. Mineurs corriges ou
  assumes explicitement dans le rapport. Review sans finding : dire ce qui a
  ete verifie empiriquement, sinon la refaire.
