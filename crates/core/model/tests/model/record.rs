//! A build's record: plugin ids are checked, and a record reads back as it was written.

use model::record::{Against, Cause, Changes, PluginId, Touched};
use model::{Ability, DeployId, Hash};

#[test]
fn a_plugin_id_is_a_scope_and_a_slot_variant() {
    let id: PluginId = "ink:format-markdown".parse().unwrap();
    assert!(id.is_format());
    assert!(!"ink:theme-quiet".parse::<PluginId>().unwrap().is_format());
    for refused in ["markdown", "ink:", "Ink:format-x", "ink:format--x", "ink:markdown"] {
        assert!(refused.parse::<PluginId>().is_err(), "{refused}");
    }
}

#[test]
fn a_change_set_reads_back_and_causes_read_as_words() {
    let deploy: DeployId = serde_json::from_value(serde_json::json!(Hash::of(b"x"))).unwrap();
    let changes = Changes::new(Against::Deploy { deploy });
    let json = serde_json::to_value(&changes).unwrap();
    assert_eq!(serde_json::from_value::<Changes>(json).unwrap(), changes);
    assert!(changes.is_empty());
    assert_eq!(Cause::Content(Touched::Edited).to_string(), "content: the file was edited");
    assert_eq!(Cause::Ability { ability: Ability::History }.to_string(), "source: history is gained or lost");
}
