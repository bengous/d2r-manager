import io.github.paladijn.d2rsavegameparser.model.ChronicleStashTab;
import io.github.paladijn.d2rsavegameparser.model.D2Character;
import io.github.paladijn.d2rsavegameparser.model.Item;
import io.github.paladijn.d2rsavegameparser.model.ItemContainer;
import io.github.paladijn.d2rsavegameparser.model.ItemLocation;
import io.github.paladijn.d2rsavegameparser.model.SharedStashTab;
import io.github.paladijn.d2rsavegameparser.parser.CharacterParser;
import io.github.paladijn.d2rsavegameparser.parser.SharedStashParser;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

public final class JavaOracleDumper {
    private static final String COLUMNS = "container\tcode\tinvX\tinvY\tstack\tquality\tfingerprint";

    public static void main(String[] args) throws Exception {
        if (args.length < 3) {
            System.err.println("usage: JavaOracleDumper <fixtures-root> <output-dir> <relative-path>...");
            System.exit(2);
        }

        Path fixturesRoot = Path.of(args[0]);
        Path outputDirectory = Path.of(args[1]);

        for (int index = 2; index < args.length; index++) {
            String relativePath = args[index];
            byte[] bytes = Files.readAllBytes(fixturesRoot.resolve(relativePath));
            String document = dump(relativePath, bytes);

            Path target = outputDirectory.resolve(relativePath + ".tsv");
            Files.createDirectories(target.getParent());
            Files.writeString(target, document, StandardCharsets.UTF_8);
        }
    }

    private static String dump(String relativePath, byte[] bytes) {
        boolean character = relativePath.endsWith(".d2s");
        StringBuilder metadata = new StringBuilder();
        metadata.append(meta("file", relativePath));
        metadata.append(meta("kind", character ? "character" : "stash"));

        List<String> rows = new ArrayList<>();
        try {
            if (character) {
                metadata.append(dumpCharacter(bytes, rows));
            } else {
                metadata.append(dumpStash(bytes, rows));
            }
            metadata.insert(0, meta("status", "ok"));
        } catch (Throwable failure) {
            rows.clear();
            metadata.setLength(0);
            metadata.append(meta("status", "refused"));
            metadata.append(meta("file", relativePath));
            metadata.append(meta("kind", character ? "character" : "stash"));
            metadata.append(meta("error", failure.getClass().getName() + ": " + failure.getMessage()));
        }

        StringBuilder document = new StringBuilder();
        document.append(metadata);
        document.append(meta("rows", String.valueOf(rows.size())));
        document.append(COLUMNS).append('\n');
        for (String row : rows) {
            document.append(row).append('\n');
        }

        return document.toString();
    }

    private static String dumpCharacter(byte[] bytes, List<String> rows) {
        D2Character parsed = new CharacterParser(false).parse(ByteBuffer.wrap(bytes));

        List<Item> mercenaryItems = parsed.mercenary() == null ? List.of() : parsed.mercenary().items();

        StringBuilder metadata = new StringBuilder();
        metadata.append(meta("formatVersion", String.valueOf(parsed.fileData().version())));
        metadata.append(meta("character", parsed.name() + "\t" + parsed.level() + "\t" + parsed.characterType()
                + "\t" + parsed.hardcore()));
        metadata.append(meta("itemGroups", "items\t" + parsed.items().size()
                + "\tdeadBody\t" + parsed.deadBodyItems().size()
                + "\tmercenary\t" + mercenaryItems.size()
                + "\tgolem\t" + (parsed.golemItem() == null ? 0 : 1)));

        appendItems(rows, parsed.items(), "");
        appendItems(rows, parsed.deadBodyItems(), "CORPSE:");
        appendItems(rows, mercenaryItems, "MERC:");
        if (parsed.golemItem() != null) {
            appendItems(rows, List.of(parsed.golemItem()), "GOLEM:");
        }

        return metadata.toString();
    }

    private static String dumpStash(byte[] bytes, List<String> rows) {
        SharedStashParser parser = new SharedStashParser(false);
        List<SharedStashTab> tabs = parser.parse(ByteBuffer.wrap(bytes));

        StringBuilder metadata = new StringBuilder();
        metadata.append(meta("formatVersion", tabs.isEmpty() ? "0" : String.valueOf(tabs.getFirst().version())));
        metadata.append(meta("tabCount", String.valueOf(tabs.size())));
        for (int index = 0; index < tabs.size(); index++) {
            SharedStashTab tab = tabs.get(index);
            metadata.append(meta("tab", index + "\t" + tab.version() + "\t" + tab.gold() + "\t"
                    + tab.lengthInBytes() + "\t" + tab.items().size()));
            appendItems(rows, tab.items(), "TAB" + index + ":");
        }

        try {
            ChronicleStashTab chronicle = parser.getChronicleStashTab(ByteBuffer.wrap(bytes));
            metadata.append(meta("chronicle", chronicle.setItemsDiscovered() + "\t"
                    + chronicle.uniqueItemsDiscovered() + "\t" + chronicle.runewordsDiscovered()));
        } catch (Throwable absent) {
            metadata.append(meta("chronicle", "none\t" + absent.getClass().getName() + ": " + absent.getMessage()));
        }

        return metadata.toString();
    }

    private static void appendItems(List<String> rows, List<Item> items, String prefix) {
        for (Item item : items) {
            rows.add(row(item, prefix));
            appendItems(rows, item.socketedItems(), prefix);
        }
    }

    private static String row(Item item, String prefix) {
        return prefix + container(item) + '\t' + item.code() + '\t' + item.y() + '\t' + item.x() + '\t'
                + item.stacks() + '\t' + item.quality() + '\t' + item.fingerPrint();
    }

    private static String container(Item item) {
        if (item.location() == ItemLocation.STORED) {
            return item.container() == ItemContainer.HORADRIC_CUBE ? "CUBE" : item.container().name();
        }

        return item.location().name();
    }

    private static String meta(String key, String value) {
        return "#" + key + '\t' + value + '\n';
    }
}
