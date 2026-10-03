use nagic::{source, symbols};
use std::{collections::HashMap, fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nagi-module-symbols-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn index(&self, overlays: HashMap<PathBuf, String>) -> serde_json::Value {
        let sources =
            source::load_with_overlays(&self.0.join("main.nagi"), true, &overlays).unwrap();
        symbols::index(&sources, &[&sources.program]).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn at(index: &serde_json::Value, line: u64, column: u64) -> &serde_json::Value {
    index["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["location"]["file"]
                .as_str()
                .unwrap()
                .ends_with("main.nagi")
                && r["location"]["line"] == line
                && r["location"]["column"] == column
        })
        .unwrap()
}

#[test]
fn module_bindings_export_only_own_definitions_and_share_definition_identity() {
    let fixture = Fixture::new();
    fixture.write("dependency.nagi", "class Private:\n    hidden: bool\n");
    fixture.write("orders.nagi", "import \"dependency.nagi\"\nclass Order:\n    value: i32\ndef make() -> Order:\n    return Order(value=7)\n");
    fixture.write("other.nagi", "class Order:\n    other: bool\n");
    fixture.write("main.nagi", "import \"orders.nagi\" as orders\nimport \"orders.nagi\" as again\nfrom \"orders.nagi\" import Order as SavedOrder\nimport \"other.nagi\" as other\ndef inspect(value: orders.Order) -> SavedOrder:\n    return value\ndef main():\n    item = orders.Order(value=7)\n    print(item.value)\n");
    let index = fixture.index(HashMap::new());
    let bindings = index["bindings"].as_array().unwrap();
    let binding = |name: &str| {
        bindings
            .iter()
            .find(|b| b["name"] == name && b["file"].as_str().unwrap().ends_with("main.nagi"))
            .unwrap()
    };
    let orders = binding("orders");
    let members = orders["members"].as_array().unwrap();
    assert_eq!(
        members
            .iter()
            .map(|m| m["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Order", "make"]
    );
    let order = members.iter().find(|m| m["name"] == "Order").unwrap();
    assert_eq!(order["id"], binding("again")["members"][0]["id"]);
    assert_eq!(order["id"], binding("SavedOrder")["definition_id"]);
    assert_ne!(order["id"], binding("other")["members"][0]["id"]);
    assert_eq!(binding("SavedOrder")["definition"]["name"], "SavedOrder");
    assert!(index.to_string().contains("SavedOrder"));
    assert!(!index["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["name"].as_str().unwrap().starts_with("__nagi")));
    assert_eq!(at(&index, 5, 27)["target"]["line"], 2); // orders.Order type member
    assert_eq!(at(&index, 8, 19)["target"]["line"], 2); // orders.Order constructor
    assert_eq!(at(&index, 8, 12)["target"]["line"], 1); // alias use -> import binding
}

#[test]
fn unsaved_imported_buffers_drive_fields_and_shadowed_aliases_use_local_bindings() {
    let fixture = Fixture::new();
    let saved = "class Order:\n    old: i64\n";
    fixture.write("orders.nagi", saved);
    fixture.write("main.nagi", "import \"orders.nagi\" as orders\nclass Holder:\n    local: bool\ndef inspect(orders: Holder):\n    print(orders.local)\ndef main():\n    item = orders.Order(fresh=7)\n    print(item.fresh)\n");
    let mut overlays = HashMap::new();
    overlays.insert(
        fs::canonicalize(fixture.0.join("orders.nagi")).unwrap(),
        "class Order:\n    fresh: i32\n".into(),
    );
    let index = fixture.index(overlays);
    let order = index["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "Order")
        .unwrap();
    assert_eq!(
        order["fields"],
        serde_json::json!([{ "name":"fresh", "type":"i32" }])
    );
    assert_eq!(at(&index, 5, 11)["target"]["line"], 4);
    assert!(index["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["location"]["file"]
            .as_str()
            .unwrap()
            .ends_with("main.nagi")
            && e["location"]["line"] == 8
            && e["fields"] == serde_json::json!([{ "name":"fresh", "type":"i32" }])));
    assert_eq!(
        fs::read_to_string(fixture.0.join("orders.nagi")).unwrap(),
        saved
    );
    assert!(!fixture.0.join("build").exists());
}

#[test]
fn local_names_colliding_with_the_entrypoint_keep_source_names_and_navigation() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        "def inspect(main: i64) -> i64:\n    return main\ndef main():\n    print(inspect(7))\n",
    );
    let index = fixture.index(HashMap::new());
    assert_eq!(at(&index, 2, 12)["target"]["line"], 1);
    assert_eq!(at(&index, 2, 12)["target"]["column"], 13);
    assert!(index["locals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == "main" && v["type"] == "i64" && v["location"]["line"] == 2));
    assert!(!index["locals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"].as_str().unwrap().starts_with("__nagi_local_")));
}

#[test]
fn saved_low_namespaces_use_the_opened_file_and_rebased_definition_locations() {
    let fixture = Fixture::new();
    fixture.write("orders.nagi", "class Order:\n    value: i64\n");
    fixture.write("main.nagi", "import \"orders.nagi\" as orders\nfrom \"orders.nagi\" import Order as SavedOrder\ndef main():\n    item: SavedOrder = orders.Order(value=7)\n    print(item.value)\n");
    let mut sources = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    nagic::check::check(&mut sources.program).unwrap();
    fixture.write("saved.low", &nagic::emit::low(&sources.program));
    let saved = source::load(&fixture.0.join("saved.low"), false).unwrap();
    let index = symbols::index(&saved, &[&saved.program]).unwrap();
    let bindings = index["bindings"].as_array().unwrap();
    let orders = bindings
        .iter()
        .find(|b| b["name"] == "orders" && b["file"].as_str().unwrap().ends_with("saved.low"))
        .unwrap();
    assert_eq!(orders["members"][0]["name"], "Order");
    assert!(orders["members"][0]["location"]["file"]
        .as_str()
        .unwrap()
        .ends_with("saved.low"));
    assert!(bindings
        .iter()
        .any(|b| b["name"] == "SavedOrder" && b["file"].as_str().unwrap().ends_with("saved.low")));
    assert!(index["locals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == "item" && v["type"] == "SavedOrder"));
}

#[test]
fn record_and_import_alias_named_option_remain_record_types_in_editor_output() {
    let fixture = Fixture::new();
    for import in [
        "class Option:\n    value: i64\n",
        "from \"orders.nagi\" import Order as Option\n",
    ] {
        fixture.write("orders.nagi", "class Order:\n    value: i64\n");
        fixture.write("main.nagi", &format!("{import}def keep(value: Option) -> Option:\n    return value\ndef main():\n    item = Option(value=7)\n    print(keep(item).value)\n"));
        let index = fixture.index(HashMap::new());
        let keep = index["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == "keep")
            .unwrap();
        assert_eq!(keep["parameters"][0]["type"], "Option");
        assert_eq!(keep["return_type"], "Option");
        assert!(index["locals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "item" && v["type"] == "Option"));
        assert!(!index.to_string().contains("unit?"));
    }
}
