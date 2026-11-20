// The shipped tree pays every kind of counted machine work (docs/machine-reports.md).
use path_of_craft_tree_rules::machine_rules::{BOTTLES_PER_RUN, Work};
use path_of_craft_tree_rules::rules::Tree;

const TREE: &[u8] = include_bytes!("../mixins/path_of_craft_tree/data/tree.json");

fn shipped() -> Tree {
    Tree::parse(TREE).expect("data/tree.json")
}

#[test]
fn every_machine_work_is_paid_by_the_shipped_tree() {
    let tree = shipped();
    for (word, work) in Work::ALL {
        assert!(
            tree.xp_for_action(work.action()).is_some(),
            "{word}: no branch pays {}",
            work.action()
        );
    }
}

#[test]
fn a_mixed_bucket_pays_alchemy_half_a_brewing_run() {
    let tree = shipped();
    let (branch, mix) = tree.xp_for_action(Work::Mix.action()).expect("act:mix");
    let (_, brew) = tree.xp_for_action("act:brew").expect("act:brew");
    assert_eq!(branch, "alchemy");
    assert_eq!(mix, brew * f64::from(BOTTLES_PER_RUN) / 2.0);
}
