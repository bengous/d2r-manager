using System.Text.Json.Nodes;

namespace D2RManager.Goldens;

internal sealed record OracleRow(string Container, string Code, int InvX, int InvY, int Stack, string Quality, bool Compact);

internal sealed record JavaDump(string Status, string Error, IReadOnlyList<string> Metadata, IReadOnlyList<OracleRow> Rows);

internal static class OracleRows
{
    private const uint CompactSaveFlag = 0x00200000;

    private static readonly Dictionary<long, string> StorePages = new()
    {
        [0] = "INVENTORY",
        [1] = "EQUIP",
        [2] = "TRADE",
        [3] = "CUBE",
        [4] = "STASH",
        [5] = "BELT",
        [255] = "NULL",
    };

    private static readonly Dictionary<long, string> Modes = new()
    {
        [0] = "STORED",
        [1] = "EQUIPPED",
        [2] = "BELT",
        [3] = "ONGROUND",
        [4] = "ONCURSOR",
        [5] = "DROPPING",
        [6] = "SOCKET",
    };

    public static List<OracleRow> FromCsharp(JsonObject document)
    {
        var rows = new List<OracleRow>();
        if ((string?)document["status"] != "ok")
        {
            return rows;
        }

        JsonNode model = document["model"]!;
        if ((string?)document["kind"] == SaveDump.CharacterKind)
        {
            AppendAll(rows, model["Items"], "");
            foreach (JsonNode? corpse in model["Corpses"]!.AsArray())
            {
                AppendAll(rows, corpse!["Items"], "CORPSE:");
            }

            AppendAll(rows, model["MercItems"]?["Items"], "MERC:");
            Append(rows, model["IronGolem"]?["GolemItem"], "GOLEM:");
            return rows;
        }

        JsonArray tabs = model.AsArray();
        for (int index = 0; index < tabs.Count; index++)
        {
            AppendAll(rows, tabs[index]!["Items"], $"TAB{index}:");
        }

        return rows;
    }

    private static void AppendAll(List<OracleRow> rows, JsonNode? items, string prefix)
    {
        if (items is null)
        {
            return;
        }

        foreach (JsonNode? item in items.AsArray())
        {
            Append(rows, item, prefix);
        }
    }

    private static void Append(List<OracleRow> rows, JsonNode? item, string prefix)
    {
        if (item is null)
        {
            return;
        }

        JsonNode position = item["Position"]!;
        long mode = (long)position["Mode"]!["value"]!;
        string container = mode == 0
            ? StorePages[(long)position["StorePage"]!["value"]!]
            : Modes[mode];

        int stack = (int?)item["Quantity"] ?? (int?)item["AdvancedStashStackSize"] ?? 0;
        bool compact = ((uint)(long)item["Flags"]!["value"]! & CompactSaveFlag) != 0;

        rows.Add(new OracleRow(
            prefix + container,
            (string)item["ItemCodeString"]!,
            (int)position["InvX"]!,
            (int)position["InvY"]!,
            stack,
            ((string)item["Quality"]!["name"]!).ToUpperInvariant(),
            compact));

        AppendAll(rows, item["Sockets"], prefix);
    }

    public static JavaDump ReadJava(string path)
    {
        var metadata = new List<string>();
        var rows = new List<OracleRow>();
        string status = "missing";
        string error = "";

        foreach (string line in File.ReadAllLines(path))
        {
            if (line.Length == 0)
            {
                continue;
            }

            if (line[0] == '#')
            {
                metadata.Add(line);
                string[] fields = line[1..].Split('\t');
                if (fields[0] == "status")
                {
                    status = fields[1];
                }
                else if (fields[0] == "error")
                {
                    error = fields[1];
                }

                continue;
            }

            if (line.StartsWith("container\t", StringComparison.Ordinal))
            {
                continue;
            }

            string[] columns = line.Split('\t');
            rows.Add(new OracleRow(
                columns[0],
                columns[1],
                int.Parse(columns[2]),
                int.Parse(columns[3]),
                int.Parse(columns[4]),
                columns[5],
                false));
        }

        return new JavaDump(status, error, metadata, rows);
    }
}
