namespace D2RManager.Goldens;

internal sealed record Difference(int Row, string Field, string Csharp, string Java);

internal sealed record ConcordanceResult(
    int CsharpRows,
    int JavaRows,
    int Matched,
    int Strict,
    int CompactQualityEquivalences,
    int CursorTransitEquivalences,
    IReadOnlyList<Difference> Differences);

internal static class Concordance
{
    private const string CursorContainer = "ONCURSOR";
    private const string TransitContainer = "TRANSIT";

    public static ConcordanceResult Compare(IReadOnlyList<OracleRow> csharp, IReadOnlyList<OracleRow> java)
    {
        var differences = new List<Difference>();
        int matched = 0;
        int strict = 0;
        int compactQuality = 0;
        int cursorTransit = 0;

        int shared = Math.Min(csharp.Count, java.Count);
        for (int index = 0; index < shared; index++)
        {
            OracleRow left = csharp[index];
            OracleRow right = java[index];
            int before = differences.Count;
            bool equivalent = false;

            if (left.Container != right.Container)
            {
                if (NormalizeContainer(left.Container) == right.Container)
                {
                    cursorTransit++;
                    equivalent = true;
                }
                else
                {
                    differences.Add(new Difference(index, "container", left.Container, right.Container));
                }
            }

            if (left.Code != right.Code)
            {
                differences.Add(new Difference(index, "code", left.Code, right.Code));
            }

            if (left.InvX != right.InvX || left.InvY != right.InvY)
            {
                differences.Add(new Difference(index, "position",
                    $"{left.InvX},{left.InvY}", $"{right.InvX},{right.InvY}"));
            }

            if (left.Stack != right.Stack)
            {
                differences.Add(new Difference(index, "stack",
                    left.Stack.ToString(), right.Stack.ToString()));
            }

            if (left.Quality != right.Quality)
            {
                if (left.Compact && left.Quality == "NORMAL" && right.Quality == "NONE")
                {
                    compactQuality++;
                    equivalent = true;
                }
                else
                {
                    differences.Add(new Difference(index, "quality", left.Quality, right.Quality));
                }
            }

            if (differences.Count != before)
            {
                continue;
            }

            matched++;
            if (!equivalent)
            {
                strict++;
            }
        }

        if (csharp.Count != java.Count)
        {
            differences.Add(new Difference(-1, "rowCount",
                csharp.Count.ToString(), java.Count.ToString()));
        }

        return new ConcordanceResult(csharp.Count, java.Count, matched, strict, compactQuality, cursorTransit, differences);
    }

    private static string NormalizeContainer(string container) =>
        container.EndsWith(CursorContainer, StringComparison.Ordinal)
            ? string.Concat(container.AsSpan(0, container.Length - CursorContainer.Length), TransitContainer)
            : container;
}
