use format::bits::{BitReader, BitWriter, Bits};
use format::code::ItemCode;
use format::item::{
    AffixSlot, BodyLocation, CharacterIdentity, ChronicleEntries, CompactBody, CompactContent,
    CompleteBody, Durability, EarData, Gold, GoldAmount, GridCell, GroundMode, Item, ItemBody,
    ItemChronicle, ItemFlags, ItemName, ItemPosition, ItemQuality, ItemVersion, MagicAffix,
    NormalExtra, RareNames, Runeword, SetBonuses, SlotMode, Sockets, StorePage, TypeSpecific,
};
use format::save::ItemList;
use format::stats::{ItemStat, ItemStats, StatId};
use format::text::SaveString;

fn bits<const N: u32>(value: u32) -> Bits<N> {
    Bits::<N>::new(value).expect("value fits its field")
}

fn code(text: &str) -> ItemCode {
    ItemCode::from_text(text).expect("item code")
}

fn slot(page: StorePage) -> ItemPosition {
    ItemPosition::Slot {
        mode: SlotMode::Stored,
        body: BodyLocation::None,
        cell: GridCell::new(3, 2).expect("grid cell"),
        page,
    }
}

fn version() -> ItemVersion {
    ItemVersion::from_number(101).expect("item version 101")
}

fn compact(flags: u32, content: CompactContent) -> Item {
    Item {
        flags: ItemFlags::from_bits(flags | ItemFlags::COMPACT_SAVE),
        version: version(),
        position: slot(StorePage::Stash),
        body: ItemBody::Compact(CompactBody {
            content,
            quest_difficulty: None,
            realm: None,
            advanced_stash_stack: None,
        }),
    }
}

fn complete(flags: u32, body: CompleteBody) -> Item {
    Item {
        flags: ItemFlags::from_bits(flags),
        version: version(),
        position: slot(StorePage::Inventory),
        body: ItemBody::Complete(Box::new(body)),
    }
}

fn plain_body(text: &str, quality: ItemQuality) -> CompleteBody {
    CompleteBody {
        code: code(text),
        seed: 0x1234_5678,
        level: bits::<7>(42),
        quality,
        variable_gfx: None,
        auto_affix: None,
        runeword: None,
        name: ItemName::Absent,
        realm: None,
        type_specific: TypeSpecific::Absent,
        quantity: None,
        sockets: Sockets {
            filled: Vec::new(),
            capacity: None,
        },
        stats: ItemStats::default(),
        chronicle: None,
        advanced_stash_stack: None,
    }
}

fn ear() -> EarData {
    EarData {
        file_index: bits::<3>(4),
        level: bits::<7>(80),
        name: SaveString::new(b"Zealer".to_vec()).expect("ear name"),
    }
}

fn rare_names() -> RareNames {
    RareNames {
        prefix: bits::<8>(195),
        suffix: bits::<8>(128),
        affixes: [
            AffixSlot {
                prefix: Some(bits::<11>(239)),
                suffix: Some(bits::<11>(115)),
            },
            AffixSlot {
                prefix: None,
                suffix: Some(bits::<11>(7)),
            },
            AffixSlot {
                prefix: Some(bits::<11>(0)),
                suffix: None,
            },
        ],
    }
}

fn stats() -> ItemStats {
    ItemStats(vec![
        ItemStat {
            id: StatId::new(39).expect("stat 39"),
            param: 0,
            value: 30,
            paired: Vec::new(),
        },
        ItemStat {
            id: StatId::COLD_MIN_DAMAGE,
            param: 0,
            value: 12,
            paired: vec![24, 100],
        },
    ])
}

fn compact_branches() -> Vec<Item> {
    let mut quest = compact(
        0,
        CompactContent::Code {
            code: code("xyz"),
            gold: None,
        },
    );
    if let ItemBody::Compact(body) = &mut quest.body {
        body.quest_difficulty = Some(2);
        body.realm = Some([1, 2, 3, 4]);
        body.advanced_stash_stack = Some(bits::<8>(99));
    }

    vec![
        compact(ItemFlags::IS_EAR, CompactContent::Ear(ear())),
        compact(
            0,
            CompactContent::Code {
                code: code("gld"),
                gold: Some(Gold {
                    amount: GoldAmount::Small(bits::<12>(4095)),
                    player_gold: true,
                }),
            },
        ),
        compact(
            0,
            CompactContent::Code {
                code: code("gld"),
                gold: Some(Gold {
                    amount: GoldAmount::Large(1_075_411),
                    player_gold: false,
                }),
            },
        ),
        quest,
    ]
}

fn quality_branches() -> Vec<Item> {
    vec![
        complete(0, plain_body("ua1", ItemQuality::Inferior(bits::<3>(3)))),
        complete(
            0,
            plain_body(
                "ua1",
                ItemQuality::Tempered {
                    prefix: bits::<8>(11),
                    suffix: bits::<8>(22),
                },
            ),
        ),
        complete(
            0,
            plain_body(
                "cm1",
                ItemQuality::Normal(NormalExtra::Charm(MagicAffix::Prefix(bits::<11>(340)))),
            ),
        ),
        complete(
            0,
            plain_body(
                "cm2",
                ItemQuality::Normal(NormalExtra::Charm(MagicAffix::Suffix(bits::<11>(41)))),
            ),
        ),
        complete(
            0,
            plain_body(
                "spe",
                ItemQuality::Normal(NormalExtra::BodyPart(bits::<10>(517))),
            ),
        ),
        complete(
            0,
            plain_body(
                "tbk",
                ItemQuality::Normal(NormalExtra::ScrollOrBook(bits::<5>(17))),
            ),
        ),
        complete(0, plain_body("ua1", ItemQuality::Rare(rare_names()))),
        complete(0, plain_body("ua1", ItemQuality::Craft(rare_names()))),
    ]
}

fn payload_branches() -> Vec<Item> {
    let mut armor = plain_body("ci0", ItemQuality::Superior(bits::<3>(2)));
    armor.type_specific = TypeSpecific::Armor {
        defense: -10,
        durability: Durability::Limited {
            max: 48,
            current: 31,
        },
    };
    armor.variable_gfx = Some(bits::<3>(5));
    armor.auto_affix = Some(bits::<11>(1234));
    armor.quantity = Some(bits::<9>(511));
    armor.advanced_stash_stack = Some(bits::<8>(7));
    armor.stats = stats();

    let mut weapon = plain_body("ob1", ItemQuality::Unique(bits::<12>(4095)));
    weapon.type_specific = TypeSpecific::Weapon(Durability::Indestructible);
    weapon.name =
        ItemName::Personalized(SaveString::new(b"Crameuse".to_vec()).expect("personal name"));
    weapon.realm = Some([0, 0, 0, 0]);

    let mut runeword = plain_body("ob1", ItemQuality::Superior(bits::<3>(0)));
    runeword.type_specific = TypeSpecific::Weapon(Durability::Limited {
        max: 30,
        current: 9,
    });
    runeword.runeword = Some(Runeword {
        id: bits::<16>(20586),
        stats: stats(),
    });
    runeword.sockets = Sockets {
        filled: vec![compact(
            0,
            CompactContent::Code {
                code: code("r01"),
                gold: None,
            },
        )],
        capacity: Some(3),
    };

    let mut set = plain_body(
        "ua1",
        ItemQuality::Set {
            file_index: bits::<12>(5),
            bonuses: SetBonuses {
                mask: bits::<5>(0b1_0101),
                lists: vec![stats(), ItemStats::default(), stats()],
            },
        },
    );
    set.stats = stats();

    vec![
        complete(0, armor),
        complete(ItemFlags::PERSONALIZED, weapon),
        complete(ItemFlags::RUNEWORD | ItemFlags::SOCKETED, runeword),
        complete(0, set),
    ]
}

fn chronicle_branches() -> Vec<Item> {
    let mut dated = plain_body(
        "ua1",
        ItemQuality::Magic {
            prefix: bits::<11>(340),
            suffix: bits::<11>(2047),
        },
    );
    dated.chronicle = Some(ItemChronicle {
        id: bits::<16>(4242),
        entries: ChronicleEntries::Dated {
            timestamp: 29_781_687,
            recipients: vec![
                CharacterIdentity {
                    game_account_id: 7,
                    character_id: 8,
                },
                CharacterIdentity {
                    game_account_id: 9,
                    character_id: 10,
                },
            ],
        },
    });

    let mut compact_form = plain_body("ua1", ItemQuality::Normal(NormalExtra::Absent));
    compact_form.chronicle = Some(ItemChronicle {
        id: bits::<16>(1),
        entries: ChronicleEntries::Compact(CharacterIdentity {
            game_account_id: 11,
            character_id: 12,
        }),
    });

    vec![
        complete(ItemFlags::CHRONICLE, dated),
        complete(
            ItemFlags::CHRONICLE | ItemFlags::CHRONICLE_COMPACT,
            compact_form,
        ),
    ]
}

fn position_branches() -> Vec<Item> {
    let plain = || {
        complete(
            0,
            plain_body("ua1", ItemQuality::Normal(NormalExtra::Absent)),
        )
    };

    let mut items = Vec::new();
    for mode in [GroundMode::OnGround, GroundMode::Dropping] {
        let mut item = plain();
        item.position = ItemPosition::Ground {
            mode,
            x: 0xBEEF,
            y: 0xF00D,
        };
        items.push(item);
    }
    for mode in [
        SlotMode::Equipped,
        SlotMode::InBelt,
        SlotMode::OnCursor,
        SlotMode::Socketed,
    ] {
        let mut item = plain();
        item.position = ItemPosition::Slot {
            mode,
            body: BodyLocation::LeftHand,
            cell: GridCell::new(15, 15).expect("grid cell"),
            page: StorePage::Belt,
        };
        items.push(item);
    }
    items
}

fn branches() -> Vec<Item> {
    let mut items = compact_branches();
    items.extend(quality_branches());
    items.extend(payload_branches());
    items.extend(chronicle_branches());
    items.extend(position_branches());
    items
}

#[test]
fn the_branches_the_corpus_never_exercises_survive_a_write_then_read() {
    let list = ItemList(branches());

    let mut writer = BitWriter::new();
    list.write(&mut writer).expect("write item list");
    let bytes = writer.finish();

    let mut reader = BitReader::new(&bytes);
    let read_back = ItemList::read(&mut reader).expect("read item list");

    assert_eq!(read_back.0.len(), list.0.len(), "item count");
    for (index, (mine, theirs)) in read_back.0.iter().zip(&list.0).enumerate() {
        assert_eq!(mine, theirs, "item {index}");
    }
}

#[test]
fn a_second_write_of_the_read_model_gives_the_same_bytes() {
    let list = ItemList(branches());

    let mut writer = BitWriter::new();
    list.write(&mut writer).expect("write item list");
    let first = writer.finish();

    let read_back = ItemList::read(&mut BitReader::new(&first)).expect("read item list");
    let mut writer = BitWriter::new();
    read_back.write(&mut writer).expect("rewrite item list");

    assert_eq!(writer.finish(), first, "byte for byte");
}
