using System.Globalization;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace D2RManager.Goldens;

internal sealed record FixtureReport(
    string RelativePath,
    string Group,
    string Kind,
    int Bytes,
    long Version,
    string Status,
    string Error,
    int Items,
    int ItemsWithSockets,
    JsonObject Document,
    JavaDump Java,
    ConcordanceResult? Concordance);

internal static class Report
{
    private const string MarkerFixture = "v105/linux/ModernSharedStashSoftCoreV2.d2i";

    public static int Write(string csharpDirectory, string javaDirectory, string outputPath, string[] relativePaths)
    {
        List<FixtureReport> fixtures = [];
        foreach (string relativePath in relativePaths.OrderBy(path => path, StringComparer.Ordinal))
        {
            fixtures.Add(Load(csharpDirectory, javaDirectory, relativePath));
        }

        var text = new StringBuilder();
        WriteHeader(text);
        WriteConventions(text, fixtures);
        WriteGroup(text, fixtures, "v105/linux/", "Corpus v105/linux");
        WriteGroup(text, fixtures, "v105/windows/", "Corpus v105/windows");
        WriteGroup(text, fixtures, "legacy/", "Corpus legacy");
        WriteRoundTrip(text, fixtures);
        WriteDisagreements(text, fixtures);
        WriteMarkers(text, fixtures);
        WriteNotes(text, fixtures);

        Directory.CreateDirectory(Path.GetDirectoryName(outputPath)!);
        File.WriteAllText(outputPath, text.ToString());
        return Regressions(fixtures);
    }

    private static int Regressions(List<FixtureReport> fixtures)
    {
        var failures = new List<string>();
        foreach (FixtureReport fixture in fixtures)
        {
            if (fixture.Concordance is { Differences.Count: > 0 })
            {
                failures.Add($"{fixture.RelativePath}: {fixture.Concordance.Differences.Count} desaccords C#/Java");
            }

            if (fixture.Group == "legacy/")
            {
                continue;
            }

            if (fixture.Status != "ok")
            {
                failures.Add($"{fixture.RelativePath}: lecture C# refusee, {fixture.Error}");
                continue;
            }

            if (!(bool)fixture.Document["roundTrip"]!["identical"]!)
            {
                failures.Add($"{fixture.RelativePath}: round-trip C# {RoundTripCell(fixture)}");
            }

            if (fixture.Java.Status != "ok")
            {
                failures.Add($"{fixture.RelativePath}: lecture Java refusee, {fixture.Java.Error}");
            }
        }

        if (failures.Count == 0)
        {
            return 0;
        }

        Console.Error.WriteLine("regression sur le corpus v105 :");
        foreach (string failure in failures)
        {
            Console.Error.WriteLine($"  {failure}");
        }

        return 1;
    }

    private static FixtureReport Load(string csharpDirectory, string javaDirectory, string relativePath)
    {
        var document = (JsonObject)JsonNode.Parse(
            File.ReadAllText(Path.Combine(csharpDirectory, relativePath + ".json")))!;
        JavaDump java = OracleRows.ReadJava(Path.Combine(javaDirectory, relativePath + ".tsv"));

        string status = (string)document["status"]!;
        JsonNode? counts = document["itemCounts"];
        ConcordanceResult? concordance = status == "ok" && java.Status == "ok"
            ? Concordance.Compare(OracleRows.FromCsharp(document), java.Rows)
            : null;

        return new FixtureReport(
            relativePath,
            Group(relativePath),
            (string)document["kind"]!,
            (int)document["fileBytes"]!,
            status == "ok" ? (long)document["formatVersion"]! : 0,
            status,
            status == "ok" ? "" : (string)document["error"]!,
            counts is null ? 0 : (int)counts["total"]!,
            counts is null ? 0 : (int)counts["totalWithSockets"]!,
            document,
            java,
            concordance);
    }

    private static string Group(string relativePath) => relativePath switch
    {
        _ when relativePath.StartsWith("v105/linux/", StringComparison.Ordinal) => "v105/linux/",
        _ when relativePath.StartsWith("v105/windows/", StringComparison.Ordinal) => "v105/windows/",
        _ => "legacy/",
    };

    private static void WriteHeader(StringBuilder text)
    {
        text.AppendLine("# Harnais differentiel : rapport");
        text.AppendLine();
        text.AppendLine("Fichier genere par `just goldens`. Toute edition manuelle est ecrasee.");
        text.AppendLine();
        text.AppendLine("| Oracle | Source | Role |");
        text.AppendLine("|---|---|---|");
        text.AppendLine("| C# | D2SSharp, submodule `reference/D2SSharp` (MIT) | lecture, ecriture, verdict round-trip |");
        text.AppendLine("| Java | d2rsavegameparser 3.2.0, jar telecharge et verifie par sha256 (LGPL 2.1) | lecture seule, execute, jamais porte |");
        text.AppendLine();
        text.AppendLine("Sorties : `goldens/csharp/<chemin>.json` (modele D2SSharp complet, cles triees,");
        text.AppendLine("indente, sans horodatage) et `goldens/java/<chemin>.tsv` (un objet par ligne).");
        text.AppendLine();
        text.AppendLine("Le jar de l'oracle Java est telecharge par `tools/goldens-java/fetch-jars.sh`,");
        text.AppendLine("verifie par sha256 `0b2164f3c8219e390c42ccba3e2377dce2514561e1c32f5d0c1d4c9622960f89`");
        text.AppendLine("et jamais commite. Il lit les tables du jeu dans");
        text.AppendLine("`reference/d2rsavegameparser/txt/`, en place et en lecture seule.");
        text.AppendLine();
        text.AppendLine("Le parseur declare `org.slf4j:slf4j-api` en portee `provided` et ses classes ne");
        text.AppendLine("se chargent pas sans lui. Le meme script telecharge donc slf4j-api 2.0.17,");
        text.AppendLine("sha256 `7b751d952061954d5abfed7181c1f645d336091b679891591d63329c622eb832`.");
        text.AppendLine("Aucun fournisseur de journalisation n'est ajoute : le parseur reste silencieux.");
        text.AppendLine();
    }

    private static void WriteConventions(StringBuilder text, List<FixtureReport> fixtures)
    {
        int compared = fixtures.Count(fixture => fixture.Concordance is not null);
        int rows = fixtures.Sum(fixture => fixture.Concordance?.CsharpRows ?? 0);
        int matched = fixtures.Sum(fixture => fixture.Concordance?.Matched ?? 0);
        int strict = fixtures.Sum(fixture => fixture.Concordance?.Strict ?? 0);
        int compactQuality = fixtures.Sum(fixture => fixture.Concordance?.CompactQualityEquivalences ?? 0);
        int cursorTransit = fixtures.Sum(fixture => fixture.Concordance?.CursorTransitEquivalences ?? 0);

        text.AppendLine("## Conventions");
        text.AppendLine();
        text.AppendLine("1. Axes. Le parseur Java expose `x()` = ligne et `y()` = colonne, soit l'inverse");
        text.AppendLine("   de D2SSharp. Les TSV sont ecrits dans la convention D2SSharp : colonne");
        text.AppendLine($"   `invX` = `y()` Java = `InvX` C#, ligne `invY` = `x()` Java = `InvY` C#. Sur les {compared}");
        text.AppendLine($"   fichiers lus par les deux oracles, {rows} lignes ont la meme position apres cet echange.");
        FixtureReport? axes = fixtures.FirstOrDefault(candidate =>
            candidate.RelativePath == "v105/windows/ModernSharedStashSoftCoreV2.d2i");
        if (axes?.Concordance is not null)
        {
            text.AppendLine($"   La mesure de PLAN section 6 porte sur `{axes.RelativePath}` :");
            text.AppendLine($"   {axes.Items} objets de premier niveau, {axes.Concordance.Matched} lignes concordantes objets sertis inclus.");
        }

        text.AppendLine("2. Conteneurs. Vocabulaire commun : `INVENTORY`, `CUBE`, `STASH`, `EQUIPPED`,");
        text.AppendLine("   `BELT`, `SOCKET`, `TRANSIT`. Les objets d'un coffre partage portent le prefixe");
        text.AppendLine("   `TAB<n>:`, ceux d'un cadavre `CORPSE:`, ceux du mercenaire `MERC:`, le golem");
        text.AppendLine("   `GOLEM:`. Cote C# le conteneur vient de `ItemPosition.Mode` puis");
        text.AppendLine("   `ItemPosition.StorePage` ; cote Java de `location()` puis `container()`.");
        text.AppendLine("3. Comptage. `objets` compte les objets de premier niveau (hors objets sertis) ;");
        text.AppendLine("   `lignes` compte les objets sertis en plus, et c'est l'unite de comparaison.");
        text.AppendLine("4. Equivalences de vocabulaire appliquees a la comparaison, mesurees :");
        text.AppendLine($"   - objet compact (`ItemFlags.CompactSave`) : C# `NORMAL`, Java `NONE` ({compactQuality} lignes) ;");
        text.AppendLine($"   - C# `ItemMode.OnCursor`, Java `ItemLocation.TRANSIT` ({cursorTransit} lignes).");
        text.AppendLine("   Ces deux regles nomment differemment une meme valeur lue ; tout autre ecart");
        text.AppendLine("   est un desaccord et figure dans la section dediee. Une ligne peut relever");
        text.AppendLine("   des deux regles a la fois, les deux comptes ne s'additionnent donc pas.");
        text.AppendLine("5. Format TSV : lignes de metadonnees `#cle<TAB>valeur`, puis l'entete de");
        text.AppendLine("   colonnes `container code invX invY stack quality fingerprint`, puis un objet");
        text.AppendLine("   par ligne. `fingerprint` est l'empreinte calculee par l'oracle Java ; D2SSharp");
        text.AppendLine("   n'a pas d'equivalent, elle n'entre pas dans la comparaison.");
        text.AppendLine($"6. Concordance globale : {matched}/{rows} lignes concordantes, dont {strict} identiques");
        text.AppendLine($"   champ a champ sans aucune regle, et {matched - strict} ramenees par les equivalences");
        text.AppendLine("   du point 4. La colonne `concordance` des tableaux donne le meme detail.");
        text.AppendLine("7. Garde-fou : `goldens-csharp report` sort en erreur si un fichier `v105/` est");
        text.AppendLine("   refuse par un oracle, si son round-trip C# n'est plus identique, ou si un");
        text.AppendLine("   desaccord apparait. Une regression de lecture ou d'ecriture arrete donc");
        text.AppendLine("   `just goldens` au lieu de passer inapercue dans un diff.");
        text.AppendLine();
    }

    private static void WriteGroup(StringBuilder text, List<FixtureReport> fixtures, string group, string title)
    {
        List<FixtureReport> selected = fixtures.Where(fixture => fixture.Group == group).ToList();
        text.AppendLine($"## {title} ({selected.Count} fichiers)");
        text.AppendLine();
        text.AppendLine("| fixture | octets | version | objets | lignes | round-trip C# | concordance C#/Java |");
        text.AppendLine("|---|---:|---:|---:|---:|---|---|");
        foreach (FixtureReport fixture in selected)
        {
            string version = fixture.Status == "ok" ? fixture.Version.ToString(CultureInfo.InvariantCulture) : "-";
            string items = fixture.Status == "ok" ? fixture.Items.ToString(CultureInfo.InvariantCulture) : "-";
            string rows = fixture.Status == "ok" ? fixture.ItemsWithSockets.ToString(CultureInfo.InvariantCulture) : "-";
            text.AppendLine($"| {Path.GetFileName(fixture.RelativePath)} | {fixture.Bytes} | {version} | {items} "
                + $"| {rows} | {RoundTripCell(fixture)} | {ConcordanceCell(fixture)} |");
        }

        text.AppendLine();
    }

    private static string RoundTripCell(FixtureReport fixture)
    {
        if (fixture.Status != "ok")
        {
            return "lecture refusee";
        }

        JsonNode verdict = fixture.Document["roundTrip"]!;
        if ((bool)verdict["identical"]!)
        {
            return "identique";
        }

        if (verdict["error"] is { } failure)
        {
            return $"ecriture en erreur ({(string)failure!})";
        }

        return $"differe (offset {(int)verdict["firstDifferenceOffset"]!}, {(int)verdict["differingBytes"]!} octets)";
    }

    private static string ConcordanceCell(FixtureReport fixture)
    {
        if (fixture.Status != "ok" && fixture.Java.Status != "ok")
        {
            return "les deux oracles refusent";
        }

        if (fixture.Status != "ok")
        {
            return "C# refuse";
        }

        if (fixture.Java.Status != "ok")
        {
            return "Java refuse";
        }

        ConcordanceResult result = fixture.Concordance!;
        if (result.Differences.Count > 0)
        {
            return $"{result.Matched}/{result.CsharpRows} (desaccords)";
        }

        int equivalences = result.Matched - result.Strict;
        return equivalences == 0
            ? $"{result.Matched}/{result.CsharpRows} strictes"
            : $"{result.Matched}/{result.CsharpRows} dont {equivalences} par equivalence";
    }

    private static void WriteRoundTrip(StringBuilder text, List<FixtureReport> fixtures)
    {
        text.AppendLine("## Round-trip C# (`write(read(f)) == f`, octet a octet)");
        text.AppendLine();
        foreach (string extension in (string[])[".d2s", ".d2i"])
        {
            List<FixtureReport> selected = fixtures
                .Where(fixture => fixture.RelativePath.EndsWith(extension, StringComparison.Ordinal))
                .ToList();
            List<FixtureReport> readable = selected.Where(fixture => fixture.Status == "ok").ToList();
            int identical = readable.Count(fixture => (bool)fixture.Document["roundTrip"]!["identical"]!);
            text.AppendLine($"- `{extension}` : {identical}/{readable.Count} identiques parmi les fichiers lus, "
                + $"sur {selected.Count} fichiers du corpus, avec {selected.Count - readable.Count} refus en lecture.");
        }

        foreach (string group in (string[])["v105/linux/", "v105/windows/", "legacy/"])
        {
            List<FixtureReport> readable = fixtures
                .Where(fixture => fixture.Group == group && fixture.Status == "ok")
                .ToList();
            int identical = readable.Count(fixture => (bool)fixture.Document["roundTrip"]!["identical"]!);
            text.AppendLine($"- `{group}` : {identical}/{readable.Count} identiques sur les fichiers lus.");
        }

        text.AppendLine();
        List<FixtureReport> failures = fixtures
            .Where(fixture => fixture.Status == "ok" && !(bool)fixture.Document["roundTrip"]!["identical"]!)
            .ToList();
        if (failures.Count == 0)
        {
            text.AppendLine("Aucun fichier lu ne differe apres reecriture.");
        }
        else
        {
            foreach (FixtureReport fixture in failures)
            {
                text.AppendLine($"- `{fixture.RelativePath}` : {RoundTripCell(fixture)}.");
            }
        }

        text.AppendLine();
        List<FixtureReport> refused = fixtures.Where(fixture => fixture.Status != "ok").ToList();
        if (refused.Count > 0)
        {
            text.AppendLine("Fichiers refuses par l'oracle C# :");
            text.AppendLine();
            foreach (FixtureReport fixture in refused)
            {
                text.AppendLine($"- `{fixture.RelativePath}` : {fixture.Error}");
            }

            text.AppendLine();
        }
    }

    private static void WriteDisagreements(StringBuilder text, List<FixtureReport> fixtures)
    {
        text.AppendLine("## Desaccords C#/Java");
        text.AppendLine();

        List<FixtureReport> disagreeing = fixtures
            .Where(fixture => fixture.Concordance is { Differences.Count: > 0 })
            .ToList();

        if (disagreeing.Count == 0)
        {
            int compared = fixtures.Count(fixture => fixture.Concordance is not null);
            int rows = fixtures.Sum(fixture => fixture.Concordance?.CsharpRows ?? 0);
            int strict = fixtures.Sum(fixture => fixture.Concordance?.Strict ?? 0);
            int compactQuality = fixtures.Sum(fixture => fixture.Concordance?.CompactQualityEquivalences ?? 0);
            int cursorTransit = fixtures.Sum(fixture => fixture.Concordance?.CursorTransitEquivalences ?? 0);
            text.AppendLine($"Aucun. Les {compared} fichiers lus par les deux oracles donnent {rows} lignes");
            text.AppendLine("concordantes sur conteneur, code, position, pile et qualite. Le detail :");
            text.AppendLine();
            int matched = fixtures.Sum(fixture => fixture.Concordance?.Matched ?? 0);
            text.AppendLine($"- {strict} lignes identiques champ a champ, sans aucune regle ;");
            text.AppendLine($"- {matched - strict} lignes ramenees par au moins une equivalence, dont {compactQuality} par la");
            text.AppendLine($"  regle de qualite des objets compacts et {cursorTransit} par la regle `OnCursor` / `TRANSIT`");
            text.AppendLine("  (une meme ligne peut relever des deux).");
            text.AppendLine();
        }
        else
        {
            foreach (FixtureReport fixture in disagreeing)
            {
                text.AppendLine($"### `{fixture.RelativePath}`");
                text.AppendLine();
                text.AppendLine("| ligne | champ | C# | Java |");
                text.AppendLine("|---:|---|---|---|");
                foreach (Difference difference in fixture.Concordance!.Differences)
                {
                    text.AppendLine($"| {difference.Row} | {difference.Field} | {difference.Csharp} | {difference.Java} |");
                }

                text.AppendLine();
            }
        }

        List<FixtureReport> javaRefused = fixtures.Where(fixture => fixture.Java.Status != "ok").ToList();
        if (javaRefused.Count > 0)
        {
            text.AppendLine("Fichiers refuses par l'oracle Java (v105 exclusif) :");
            text.AppendLine();
            foreach (FixtureReport fixture in javaRefused)
            {
                text.AppendLine($"- `{fixture.RelativePath}` : {fixture.Java.Error}");
            }

            text.AppendLine();
        }
    }

    private static void WriteMarkers(StringBuilder text, List<FixtureReport> fixtures)
    {
        FixtureReport? fixture = fixtures.FirstOrDefault(candidate => candidate.RelativePath == MarkerFixture);
        text.AppendLine("## Reperes anti-regression (PLAN section 6)");
        text.AppendLine();
        if (fixture is null || fixture.Status != "ok")
        {
            text.AppendLine($"Fixture `{MarkerFixture}` absente ou illisible : reperes non mesures.");
            text.AppendLine();
            return;
        }

        JsonArray tabs = fixture.Document["tabs"]!.AsArray();
        JsonNode counts = fixture.Document["itemCounts"]!;
        int normal = tabs.Count(tab => (string?)tab!["tabType"] == "Normal");
        int advanced = tabs.Count(tab => (string?)tab!["tabType"] == "AdvancedStash");
        int chronicle = tabs.Count(tab => (string?)tab!["tabType"] == "Chronicle");
        JsonNode? chronicleTab = tabs.FirstOrDefault(tab => (string?)tab!["tabType"] == "Chronicle");

        text.AppendLine($"Fixture `{MarkerFixture}`.");
        text.AppendLine();
        text.AppendLine("| repere | attendu | mesure |");
        text.AppendLine("|---|---|---|");
        text.AppendLine($"| onglets | 7 (5 normaux, 1 AdvancedStash, 1 Chronicle) | {tabs.Count} ({normal} normaux, {advanced} AdvancedStash, {chronicle} Chronicle) |");
        text.AppendLine($"| objets | 1533 | {(int)counts["items"]!} objets, {(int)counts["advancedStashStacks"]!} unites empilees dans l'onglet AdvancedStash, somme {(int)counts["itemsPlusAdvancedStashStacks"]!} |");
        text.AppendLine($"| or, premier onglet (index 0) | 1075411 | {(long)tabs[0]!["gold"]!} |");
        text.AppendLine($"| onglet Chronicle | 2298 octets | {(chronicleTab is null ? 0 : (int)chronicleTab["fileTabBytes"]!)} octets |");
        text.AppendLine();
        text.AppendLine("Le repere « 1533 objets » ne correspond pas au nombre d'objets du fichier : il");
        text.AppendLine($"vaut {(int)counts["items"]!} objets plus {(int)counts["advancedStashStacks"]!} unites empilees dans l'onglet AdvancedStash. Les objets");
        text.AppendLine($"sertis portent le total a {(int)counts["totalWithSockets"]!} lignes.");
        text.AppendLine();
    }

    private static void WriteNotes(StringBuilder text, List<FixtureReport> fixtures)
    {
        text.AppendLine("## Notes mesurees");
        text.AppendLine();

        var overlaps = new List<string>();
        foreach (FixtureReport fixture in fixtures.Where(candidate => candidate.Kind == SaveDump.StashKind && candidate.Status == "ok"))
        {
            JsonArray tabs = fixture.Document["tabs"]!.AsArray();
            JsonArray model = fixture.Document["model"]!.AsArray();
            for (int index = 0; index < tabs.Count; index++)
            {
                if ((string?)tabs[index]!["tabType"] != "AdvancedStash")
                {
                    continue;
                }

                var positions = new HashSet<(long, long)>();
                int collisions = 0;
                int items = 0;
                foreach (JsonNode? item in model[index]!["Items"]!.AsArray())
                {
                    items++;
                    JsonNode position = item!["Position"]!;
                    if (!positions.Add(((long)position["InvX"]!, (long)position["InvY"]!)))
                    {
                        collisions++;
                    }
                }

                overlaps.Add($"- `{fixture.RelativePath}`, onglet d'index {index} : {items} objets, {collisions} positions deja occupees.");
            }
        }

        text.AppendLine("L'onglet `AdvancedStash` n'est pas une grille : les fichiers ecrits par le jeu");
        text.AppendLine("y contiennent des positions qui se recouvrent. Les deux oracles lisent les memes");
        text.AppendLine("valeurs ; ce n'est pas un desaccord.");
        text.AppendLine();
        foreach (string overlap in overlaps)
        {
            text.AppendLine(overlap);
        }

        text.AppendLine();
        text.AppendLine("Comportement des oracles sur `legacy/` (versions 98 et 99, hors perimetre v105) :");
        text.AppendLine();
        foreach (FixtureReport fixture in fixtures.Where(candidate => candidate.Group == "legacy/"))
        {
            string csharp = fixture.Status == "ok"
                ? $"lu (version {fixture.Version}), round-trip {RoundTripCell(fixture)}"
                : $"refuse ({fixture.Error})";
            string java = fixture.Java.Status == "ok" ? "lu" : $"refuse ({fixture.Java.Error})";
            text.AppendLine($"- `{fixture.RelativePath}` : C# {csharp} ; Java {java}.");
        }

        text.AppendLine();
        List<FixtureReport> legacyRefused = fixtures
            .Where(candidate => candidate.Group == "legacy/" && candidate.Status != "ok")
            .ToList();
        if (legacyRefused.Count > 0)
        {
            text.AppendLine("Ecart avec PLAN section 6, qui attend que D2SSharp lise les fixtures");
            text.AppendLine("`legacy/` (v96 a v105) : la bibliotheque n'embarque pas les tables `txt` de");
            text.AppendLine("toutes les versions, et refuse donc les fichiers suivants.");
            text.AppendLine();
            foreach (FixtureReport fixture in legacyRefused)
            {
                text.AppendLine($"- `{fixture.RelativePath}` : {fixture.Error}");
            }

            text.AppendLine();
        }
    }
}
