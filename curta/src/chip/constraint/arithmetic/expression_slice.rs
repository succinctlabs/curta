use alloc::sync::Arc;

use crate::air::parser::AirParser;
use crate::chip::register::memory::MemorySlice;
use crate::chip::register::Register;
use crate::math::prelude::*;

#[derive(Clone, Debug)]
pub enum ArithmeticExpressionSlice<F> {
    /// A contiguous chunk of elemnt of a trace column.
    Input(MemorySlice),
    /// A constant vector of field values.
    Const(Vec<F>),
    /// The addition of two arithmetic expressions.
    Add(
        Arc<ArithmeticExpressionSlice<F>>,
        Arc<ArithmeticExpressionSlice<F>>,
    ),
    /// The subtraction of two arithmetic expressions
    Sub(
        Arc<ArithmeticExpressionSlice<F>>,
        Arc<ArithmeticExpressionSlice<F>>,
    ),
    /// The scalar multiplication of an arithmetic expression by a field element.
    ScalarMul(F, Arc<ArithmeticExpressionSlice<F>>),
    /// The multiplication of two arithmetic expressions.
    Mul(
        Arc<ArithmeticExpressionSlice<F>>,
        Arc<ArithmeticExpressionSlice<F>>,
    ),
}

impl<F: Field> ArithmeticExpressionSlice<F> {
    pub fn new<T: Register>(input: &T) -> Self {
        ArithmeticExpressionSlice::Input(*input.register())
    }

    pub fn from_raw_register(input: MemorySlice) -> Self {
        ArithmeticExpressionSlice::Input(input)
    }

    pub fn from_constant(constant: F) -> Self {
        ArithmeticExpressionSlice::Const(vec![constant])
    }

    pub fn from_constant_vec(constants: Vec<F>) -> Self {
        ArithmeticExpressionSlice::Const(constants)
    }

    pub fn registers(&self) -> Vec<MemorySlice> {
        match self {
            ArithmeticExpressionSlice::Input(input) => vec![*input],
            ArithmeticExpressionSlice::Const(_) => vec![],
            ArithmeticExpressionSlice::Add(left, right) => {
                let mut left = left.registers();
                let mut right = right.registers();
                left.append(&mut right);
                left
            }
            ArithmeticExpressionSlice::Sub(left, right) => {
                let mut left = left.registers();
                let mut right = right.registers();
                left.append(&mut right);
                left
            }
            ArithmeticExpressionSlice::ScalarMul(_, expr) => expr.registers(),
            ArithmeticExpressionSlice::Mul(left, right) => {
                let mut left = left.registers();
                let mut right = right.registers();
                left.append(&mut right);
                left
            }
        }
    }

    pub fn eval<AP: AirParser<Field = F>>(&self, parser: &mut AP) -> Vec<AP::Var> {
        match self {
            ArithmeticExpressionSlice::Input(input) => input.eval_slice(parser).to_vec(),
            ArithmeticExpressionSlice::Const(constants) => {
                constants.iter().map(|x| parser.constant(*x)).collect()
            }
            ArithmeticExpressionSlice::Add(left, right) => {
                let left = left.eval(parser);
                let right = right.eval(parser);
                left.iter()
                    .zip(right.iter())
                    .map(|(l, r)| parser.add(*l, *r))
                    .collect()
            }
            ArithmeticExpressionSlice::Sub(left, right) => {
                let left = left.eval(parser);
                let right = right.eval(parser);
                left.iter()
                    .zip(right.iter())
                    .map(|(l, r)| parser.sub(*l, *r))
                    .collect()
            }
            ArithmeticExpressionSlice::ScalarMul(scalar, expr) => {
                let expr_val = expr.eval(parser);
                expr_val
                    .iter()
                    .map(|x| parser.mul_const(*x, *scalar))
                    .collect()
            }
            ArithmeticExpressionSlice::Mul(left, right) => {
                let left_vals = left.eval(parser);
                let right_vals = right.eval(parser);
                left_vals
                    .iter()
                    .zip(right_vals.iter())
                    .map(|(l, r)| parser.mul(*l, *r))
                    .collect()
            }
        }
    }
}
