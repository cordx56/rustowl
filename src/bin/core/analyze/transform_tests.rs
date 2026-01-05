use super::transform;
use rustc_borrowck::consumers::RichLocation;
use rustc_middle::mir::BasicBlock;
use rustowl::models::{MirBasicBlock, MirStatement, Range, StatementVec};

fn mk_range(from: u32, until: u32) -> Range {
    Range::new(from.into(), until.into()).expect("valid range")
}

#[test]
fn rich_locations_to_ranges_pairs_start_and_mid() {
    let basic_blocks = vec![MirBasicBlock {
        statements: StatementVec::from(vec![
            MirStatement::Other {
                range: mk_range(10, 11),
            },
            MirStatement::Other {
                range: mk_range(20, 21),
            },
        ]),
        terminator: None,
    }];

    let locations = vec![
        RichLocation::Start(rustc_middle::mir::Location {
            block: BasicBlock::from_u32(0),
            statement_index: 0,
        }),
        RichLocation::Mid(rustc_middle::mir::Location {
            block: BasicBlock::from_u32(0),
            statement_index: 1,
        }),
    ];

    let ranges = transform::rich_locations_to_ranges(&basic_blocks, &locations);
    assert_eq!(ranges.len(), 1);
    assert_eq!(u32::from(ranges[0].from()), 10);
    assert_eq!(u32::from(ranges[0].until()), 21);
}

#[test]
fn rich_locations_to_ranges_truncates_mismatched_start_mid_counts() {
    let basic_blocks = vec![MirBasicBlock {
        statements: StatementVec::from(vec![
            MirStatement::Other {
                range: mk_range(1, 2),
            },
            MirStatement::Other {
                range: mk_range(3, 4),
            },
        ]),
        terminator: None,
    }];

    let locations = vec![
        RichLocation::Start(rustc_middle::mir::Location {
            block: BasicBlock::from_u32(0),
            statement_index: 0,
        }),
        RichLocation::Start(rustc_middle::mir::Location {
            block: BasicBlock::from_u32(0),
            statement_index: 1,
        }),
        RichLocation::Mid(rustc_middle::mir::Location {
            block: BasicBlock::from_u32(0),
            statement_index: 0,
        }),
    ];

    let ranges = transform::rich_locations_to_ranges(&basic_blocks, &locations);
    assert_eq!(ranges.len(), 1);
    assert_eq!(u32::from(ranges[0].from()), 1);
    assert_eq!(u32::from(ranges[0].until()), 2);
}

#[test]
fn rich_locations_to_ranges_uses_terminator_range_when_statement_index_out_of_bounds() {
    let basic_blocks = vec![MirBasicBlock {
        statements: StatementVec::from(vec![MirStatement::Other {
            range: mk_range(10, 11),
        }]),
        terminator: Some(rustowl::models::MirTerminator::Other {
            range: mk_range(40, 50),
        }),
    }];

    let locations = vec![
        RichLocation::Start(rustc_middle::mir::Location {
            block: BasicBlock::from_u32(0),
            statement_index: 999,
        }),
        RichLocation::Mid(rustc_middle::mir::Location {
            block: BasicBlock::from_u32(0),
            statement_index: 999,
        }),
    ];

    let ranges = transform::rich_locations_to_ranges(&basic_blocks, &locations);
    assert_eq!(ranges.len(), 1);
    assert_eq!(u32::from(ranges[0].from()), 40);
    assert_eq!(u32::from(ranges[0].until()), 50);
}
