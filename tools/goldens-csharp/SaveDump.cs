using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text.Json.Nodes;
using D2SSharp.Enums;
using D2SSharp.Model;

namespace D2RManager.Goldens;

internal static class SaveDump
{
    public const string CharacterKind = "character";
    public const string StashKind = "stash";

    public static string KindOf(string relativePath) =>
        relativePath.EndsWith(".d2s", StringComparison.Ordinal) ? CharacterKind : StashKind;

    public static JsonObject Build(string relativePath, byte[] bytes)
    {
        string kind = KindOf(relativePath);
        var document = new JsonObject
        {
            ["file"] = relativePath,
            ["fileBytes"] = bytes.Length,
            ["sha256"] = Convert.ToHexStringLower(SHA256.HashData(bytes)),
            ["kind"] = kind,
        };

        try
        {
            if (kind == CharacterKind)
            {
                D2Save save = D2Save.Read(bytes);
                document["status"] = "ok";
                document["formatVersion"] = save.Version;
                document["itemCounts"] = CharacterItemCounts(save);
                document["model"] = ModelDumper.Dump(save);
            }
            else
            {
                D2StashSave stash = D2StashSave.Read(bytes);
                document["status"] = "ok";
                document["formatVersion"] = stash.Count == 0 ? 0 : stash[0].ItemFormat;
                document["tabs"] = TabSummaries(bytes, stash);
                document["itemCounts"] = StashItemCounts(stash);
                document["model"] = ModelDumper.Dump(stash);
            }
        }
        catch (Exception failure)
        {
            document["status"] = "error";
            document["error"] = $"{failure.GetType().Name}: {failure.Message}";
            return document;
        }

        try
        {
            document["roundTrip"] = RoundTrip(kind, bytes);
        }
        catch (Exception failure)
        {
            document["roundTrip"] = new JsonObject
            {
                ["identical"] = false,
                ["error"] = $"{failure.GetType().Name}: {failure.Message}",
            };
        }

        return document;
    }

    private static JsonObject RoundTrip(string kind, byte[] original)
    {
        byte[] written = kind == CharacterKind
            ? D2Save.Read(original).ToBytes()
            : D2StashSave.Read(original).ToBytes();

        bool identical = written.AsSpan().SequenceEqual(original);
        var verdict = new JsonObject
        {
            ["identical"] = identical,
            ["writtenBytes"] = written.Length,
        };

        if (identical)
        {
            return verdict;
        }

        int shared = Math.Min(written.Length, original.Length);
        int firstDifference = -1;
        int differingBytes = 0;
        for (int offset = 0; offset < shared; offset++)
        {
            if (written[offset] == original[offset])
            {
                continue;
            }

            differingBytes++;
            if (firstDifference < 0)
            {
                firstDifference = offset;
            }
        }

        if (firstDifference < 0 && written.Length != original.Length)
        {
            firstDifference = shared;
        }

        verdict["firstDifferenceOffset"] = firstDifference;
        verdict["differingBytes"] = differingBytes + Math.Abs(written.Length - original.Length);
        return verdict;
    }

    private static JsonObject CharacterItemCounts(D2Save save)
    {
        int corpseItems = save.Corpses.Sum(corpse => corpse.Items.Count);
        int mercItems = save.MercItems?.Items.Count ?? 0;
        int golemItems = save.IronGolem?.GolemItem is null ? 0 : 1;

        var counts = new JsonObject
        {
            ["items"] = save.Items.Count,
            ["corpseItems"] = corpseItems,
            ["mercItems"] = mercItems,
            ["ironGolemItems"] = golemItems,
            ["total"] = save.Items.Count + corpseItems + mercItems + golemItems,
        };

        int recursive = CountWithSockets(save.Items)
            + save.Corpses.Sum(corpse => CountWithSockets(corpse.Items))
            + CountWithSockets(save.MercItems?.Items)
            + (save.IronGolem?.GolemItem is { } golem ? CountWithSockets([golem]) : 0);
        counts["totalWithSockets"] = recursive;
        return counts;
    }

    private static JsonObject StashItemCounts(D2StashSave stash)
    {
        int total = stash.Sum(tab => tab.Items.Count);
        int advancedStacks = stash.Sum(AdvancedStackTotal);
        return new JsonObject
        {
            ["items"] = total,
            ["total"] = total,
            ["totalWithSockets"] = stash.Sum(tab => CountWithSockets(tab.Items)),
            ["advancedStashStacks"] = advancedStacks,
            ["itemsPlusAdvancedStashStacks"] = total + advancedStacks,
        };
    }

    private static int AdvancedStackTotal(D2StashTab tab) =>
        tab.TabType != StashTabType.AdvancedStash
            ? 0
            : tab.Items.Sum(item => item.AdvancedStashStackSize ?? 0);

    private static int CountWithSockets(IEnumerable<Item?>? items)
    {
        if (items is null)
        {
            return 0;
        }

        int count = 0;
        foreach (Item? item in items)
        {
            if (item is null)
            {
                continue;
            }

            count++;
            count += CountWithSockets(item.Sockets);
        }

        return count;
    }

    private static JsonArray TabSummaries(byte[] bytes, D2StashSave stash)
    {
        List<int> sizes = RawTabSizes(bytes);
        var summaries = new JsonArray();
        for (int index = 0; index < stash.Count; index++)
        {
            D2StashTab tab = stash[index];
            var summary = new JsonObject
            {
                ["index"] = index,
                ["tabType"] = tab.TabType.ToString(),
                ["stashFormat"] = tab.StashFormat,
                ["itemFormat"] = tab.ItemFormat,
                ["gold"] = tab.Gold,
                ["items"] = tab.Items.Count,
                ["itemsWithSockets"] = CountWithSockets(tab.Items),
                ["fileTabBytes"] = index < sizes.Count ? sizes[index] : -1,
                ["advancedStashStacks"] = AdvancedStackTotal(tab),
            };

            if (tab.TabType == StashTabType.Chronicle && tab.Chronicle is { } chronicle)
            {
                summary["chronicleSetEntries"] = chronicle.SetEntries.Count;
                summary["chronicleUniqueEntries"] = chronicle.UniqueEntries.Count;
                summary["chronicleRunewordEntries"] = chronicle.RunewordEntries.Count;
                summary["chronicleTrailingBytes"] = chronicle.TrailingData.Length;
            }

            summaries.Add(summary);
        }

        return summaries;
    }

    private static List<int> RawTabSizes(byte[] bytes)
    {
        var sizes = new List<int>();
        int offset = 0;
        while (offset + (int)D2StashTab.HeaderSize <= bytes.Length)
        {
            uint magic = BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(offset, 4));
            if (magic != D2StashTab.Magic)
            {
                break;
            }

            int size = BinaryPrimitives.ReadUInt16LittleEndian(bytes.AsSpan(offset + 16, 2));
            if (size <= 0)
            {
                break;
            }

            sizes.Add(size);
            offset += size;
        }

        return sizes;
    }
}
