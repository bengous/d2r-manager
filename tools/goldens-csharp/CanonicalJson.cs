using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace D2RManager.Goldens;

internal static class CanonicalJson
{
    public static string Serialize(JsonNode? node)
    {
        var options = new JsonWriterOptions
        {
            Indented = true,
            IndentCharacter = ' ',
            IndentSize = 2,
            NewLine = "\n",
        };

        using var buffer = new MemoryStream();
        using (var writer = new Utf8JsonWriter(buffer, options))
        {
            Write(writer, node);
        }

        return Encoding.UTF8.GetString(buffer.ToArray()) + "\n";
    }

    private static void Write(Utf8JsonWriter writer, JsonNode? node)
    {
        switch (node)
        {
            case null:
                writer.WriteNullValue();
                return;
            case JsonObject map:
                writer.WriteStartObject();
                foreach (KeyValuePair<string, JsonNode?> entry in map.OrderBy(entry => entry.Key, StringComparer.Ordinal))
                {
                    writer.WritePropertyName(entry.Key);
                    Write(writer, entry.Value);
                }

                writer.WriteEndObject();
                return;
            case JsonArray sequence:
                writer.WriteStartArray();
                foreach (JsonNode? element in sequence)
                {
                    Write(writer, element);
                }

                writer.WriteEndArray();
                return;
            default:
                node.WriteTo(writer);
                return;
        }
    }
}
