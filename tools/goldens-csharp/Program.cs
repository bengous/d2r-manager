using System.Globalization;
using System.Text.Json.Nodes;

namespace D2RManager.Goldens;

internal static class Program
{
    private static int Main(string[] args)
    {
        CultureInfo.DefaultThreadCurrentCulture = CultureInfo.InvariantCulture;

        if (args.Length >= 4 && args[0] == "dump")
        {
            return Dump(args[1], args[2], args[3..]);
        }

        if (args.Length >= 5 && args[0] == "report")
        {
            return Report.Write(args[1], args[2], args[3], args[4..]);
        }

        Console.Error.WriteLine("usage: goldens-csharp dump <fixtures-root> <output-dir> <relative-path>...");
        Console.Error.WriteLine("       goldens-csharp report <csharp-dir> <java-dir> <output-file> <relative-path>...");
        return 2;
    }

    private static int Dump(string fixturesRoot, string outputDirectory, string[] relativePaths)
    {
        foreach (string relativePath in relativePaths)
        {
            byte[] bytes = File.ReadAllBytes(Path.Combine(fixturesRoot, relativePath));
            JsonObject document = SaveDump.Build(relativePath, bytes);

            string target = Path.Combine(outputDirectory, relativePath + ".json");
            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            File.WriteAllText(target, CanonicalJson.Serialize(document));
        }

        return 0;
    }
}
