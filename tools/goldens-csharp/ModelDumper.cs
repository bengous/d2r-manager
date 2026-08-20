using System.Collections;
using System.Globalization;
using System.Reflection;
using System.Text.Json.Nodes;

namespace D2RManager.Goldens;

internal static class ModelDumper
{
    public static JsonNode? Dump(object? value)
    {
        switch (value)
        {
            case null:
                return null;
            case string text:
                return JsonValue.Create(text);
            case bool flag:
                return JsonValue.Create(flag);
            case byte[] bytes:
                return JsonValue.Create(Convert.ToHexStringLower(bytes));
            case char character:
                return JsonValue.Create(character.ToString());
        }

        Type type = value.GetType();

        if (type.IsEnum)
        {
            return new JsonObject
            {
                ["name"] = JsonValue.Create(value.ToString()),
                ["value"] = JsonValue.Create(Convert.ToInt64(value, CultureInfo.InvariantCulture)),
            };
        }

        switch (value)
        {
            case byte or sbyte or short or ushort or int or uint or long:
                return JsonValue.Create(Convert.ToInt64(value, CultureInfo.InvariantCulture));
            case ulong unsigned:
                return JsonValue.Create(unsigned);
            case float or double or decimal:
                return JsonValue.Create(Convert.ToDecimal(value, CultureInfo.InvariantCulture));
        }

        if (value is IDictionary dictionary)
        {
            var mapped = new JsonObject();
            foreach (object key in SortedKeys(dictionary))
            {
                mapped[Convert.ToString(key, CultureInfo.InvariantCulture) ?? ""] = Dump(dictionary[key]);
            }

            return mapped;
        }

        if (value is IEnumerable sequence)
        {
            var elements = new JsonArray();
            foreach (object? element in sequence)
            {
                elements.Add(Dump(element));
            }

            return elements;
        }

        var node = new JsonObject { ["$type"] = JsonValue.Create(type.Name) };
        foreach (PropertyInfo property in ReadableProperties(type))
        {
            node[property.Name] = Dump(property.GetValue(value));
        }

        return node;
    }

    private static List<object> SortedKeys(IDictionary dictionary)
    {
        var keys = new List<object>();
        foreach (object key in dictionary.Keys)
        {
            keys.Add(key);
        }

        keys.Sort((left, right) => string.CompareOrdinal(
            Convert.ToString(left, CultureInfo.InvariantCulture),
            Convert.ToString(right, CultureInfo.InvariantCulture)));
        return keys;
    }

    private static IEnumerable<PropertyInfo> ReadableProperties(Type type) =>
        type.GetProperties(BindingFlags.Public | BindingFlags.Instance)
            .Where(property => property.GetMethod is not null)
            .Where(property => property.GetIndexParameters().Length == 0)
            .OrderBy(property => property.Name, StringComparer.Ordinal);
}
