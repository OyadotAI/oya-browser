/**
 * A width-type length whose percentage waits for its containing block. `min()`, `max()` and
 * `clamp()` over mixed percentages and pixels are not linear in the percentage, so they cannot be
 * reduced to one px-plus-percent pair as `calc()` is. The style keeps the expression and layout
 * evaluates it once the containing block's width is known (CSS Values 4, section 10).
 */
use super::visual_model::Extent;
use serde::{Deserialize, Serialize};

/** A length expression, resolved to px by `resolve` once its percentage basis is known. */
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum LengthExpr {
    /** A plain px-plus-percent leaf. */
    Leaf(Extent),
    /** The sum of both operands; subtraction is a sum with a negated operand. */
    Sum(Box<LengthExpr>, Box<LengthExpr>),
    /** An operand scaled by a number, as `calc()` multiplies and divides. */
    Scale(Box<LengthExpr>, f64),
    /** The smallest operand once every operand is resolved. */
    Min(Vec<LengthExpr>),
    /** The largest operand once every operand is resolved. */
    Max(Vec<LengthExpr>),
}

impl LengthExpr {
    /** The px length this expression is at a containing block `basis` px wide. Each leaf takes
     * its percentage of `basis`, so `min()` and `max()` compare resolved lengths, as CSS requires. */
    pub fn resolve(&self, basis: f64) -> f64 {
        match self {
            Self::Leaf(extent) => extent.px + basis * extent.percent / 100.0,
            Self::Sum(left, right) => left.resolve(basis) + right.resolve(basis),
            Self::Scale(operand, factor) => operand.resolve(basis) * factor,
            Self::Min(operands) => fold(operands, basis, f64::min),
            Self::Max(operands) => fold(operands, basis, f64::max),
        }
    }
}

/** Resolves every operand at `basis` and combines them with `pick`, starting from the first. */
fn fold(operands: &[LengthExpr], basis: f64, pick: fn(f64, f64) -> f64) -> f64 {
    let mut values = operands.iter().map(|operand| operand.resolve(basis));
    let first = values.next().unwrap_or(0.0);
    values.fold(first, pick)
}
