use super::*;

/// An effect of a statement or a terminator on the value of a place.
///
/// - An `Assign` statement assigns to the place after the moves in its
///   rvalue.
/// - A `Call` terminator assigns to the destination after the moves in
///   its function and arguments.
/// - `SwitchInt`, `Assert` and `TailCall` terminators only move their
///   operands.
/// - A `Drop` terminator drops the place.
/// - `StorageDead` ends the storage of the local.
///
/// An effect on a place also applies to its fragments (see [`Places`]).
///
/// This idea is based on the theory of
/// [`cordx56/FormalRustowl`](https://github.com/cordx56/FormalRustowl).
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Effect {
    Assign,
    Move,
    Drop,
    StorageDead,
}
impl Effect {
    pub fn value_ends(self) -> bool {
        !matches!(self, Self::Assign)
    }
}

pub fn operands_effects<'a>(
    operands: impl IntoIterator<Item = &'a MirOperand>,
) -> impl Iterator<Item = (MirPlace, Effect)> {
    operands.into_iter().filter_map(|operand| match operand {
        MirOperand::Move { place } => Some((place.clone(), Effect::Move)),
        MirOperand::Copy { .. } | MirOperand::Other => None,
    })
}
pub fn rval_operands(rval: &MirRval) -> Vec<&MirOperand> {
    match rval {
        MirRval::Use { operand }
        | MirRval::Repeat { operand }
        | MirRval::Cast { operand }
        | MirRval::UnaryOp { operand } => vec![operand],
        MirRval::BinaryOp { left, right } => vec![left, right],
        MirRval::Aggregate { fields } => fields.iter().collect(),
        MirRval::Ref { .. } | MirRval::Other => Vec::new(),
    }
}
pub fn statement_effects(statement: &MirStatement) -> Vec<(MirPlace, Effect)> {
    match &statement.kind {
        MirStatementKind::Assign { place, rval, .. } => operands_effects(rval_operands(rval))
            .chain([(place.clone(), Effect::Assign)])
            .collect(),
        MirStatementKind::StorageDead { local } => {
            vec![(MirPlace::root(*local), Effect::StorageDead)]
        }
        MirStatementKind::StorageLive { .. } | MirStatementKind::Nop | MirStatementKind::Other => {
            Vec::new()
        }
    }
}
pub fn terminator_effects(terminator: &MirTerminator) -> Vec<(MirPlace, Effect)> {
    match &terminator.kind {
        MirTerminatorKind::SwitchInt { discr: operand, .. }
        | MirTerminatorKind::Assert { cond: operand, .. } => operands_effects([operand]).collect(),
        MirTerminatorKind::Drop { place, .. } => vec![(place.clone(), Effect::Drop)],
        MirTerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } => operands_effects(std::iter::once(func).chain(args))
            .chain([(destination.clone(), Effect::Assign)])
            .collect(),
        MirTerminatorKind::TailCall { func, args, .. } => {
            operands_effects(std::iter::once(func).chain(args)).collect()
        }
        MirTerminatorKind::Goto { .. }
        | MirTerminatorKind::Return
        | MirTerminatorKind::Unreachable
        | MirTerminatorKind::Other { .. } => Vec::new(),
    }
}
/// Effects of the statements of a block followed by those of its
/// terminator, so that the index is the statement index of the location.
pub fn block_effects(bb_data: &MirBasicBlock) -> Vec<Vec<(MirPlace, Effect)>> {
    bb_data
        .statements
        .iter()
        .map(statement_effects)
        .chain([terminator_effects(&bb_data.terminator)])
        .collect()
}
