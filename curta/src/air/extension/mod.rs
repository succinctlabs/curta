use core::fmt::Debug;

use super::parser::AirParser;
use crate::math::prelude::*;

pub mod cubic;

/// Operations for an extension field
pub trait ExtensionParser<F: ExtensionField<Self::Field>>: AirParser {
    type ExtensionVar: Debug + Copy + 'static;

    fn from_base_field(&mut self, value: Self::Var) -> Self::ExtensionVar;

    fn one_extension(&mut self) -> Self::ExtensionVar {
        let one = self.one();
        self.from_base_field(one)
    }

    fn zero_extension(&mut self) -> Self::ExtensionVar {
        let zero = self.zero();
        self.from_base_field(zero)
    }

    fn constant_extension(&mut self, value: F) -> Self::ExtensionVar;

    fn add_extension(&mut self, a: Self::ExtensionVar, b: Self::ExtensionVar)
        -> Self::ExtensionVar;

    fn sub_extension(&mut self, a: Self::ExtensionVar, b: Self::ExtensionVar)
        -> Self::ExtensionVar;

    fn neg_extension(&mut self, a: Self::ExtensionVar) -> Self::ExtensionVar;

    fn mul_extension(&mut self, a: Self::ExtensionVar, b: Self::ExtensionVar)
        -> Self::ExtensionVar;

    fn mul_base_field(&mut self, a: Self::ExtensionVar, b: Self::Var) -> Self::ExtensionVar {
        let b = self.from_base_field(b);
        self.mul_extension(a, b)
    }
}

// default impl<AP: AirParser> ExtensionParser<AP::Field> for AP {
//     default type ExtensionVar = AP::Var;

//     default fn from_base_field(&mut self, value: Self::Var) -> Self::ExtensionVar {
//         value
//     }

//     default fn constant_extension(&mut self, value: AP::Field) -> Self::ExtensionVar {
//         self.constant(value)
//     }

//     default fn add_extension(
//         &mut self,
//         a: Self::ExtensionVar,
//         b: Self::ExtensionVar,
//     ) -> Self::ExtensionVar {
//         self.add(a, b)
//     }

//     default fn sub_extension(
//         &mut self,
//         a: Self::ExtensionVar,
//         b: Self::ExtensionVar,
//     ) -> Self::ExtensionVar {
//         self.sub(a, b)
//     }

//     default fn neg_extension(&mut self, a: Self::ExtensionVar) -> Self::ExtensionVar {
//         self.neg(a)
//     }

//     default fn mul_extension(
//         &mut self,
//         a: Self::ExtensionVar,
//         b: Self::ExtensionVar,
//     ) -> Self::ExtensionVar {
//         self.mul(a, b)
//     }

//     default fn mul_base_field(&mut self, a: Self::ExtensionVar, b: Self::Var) -> Self::ExtensionVar {
//         self.mul(a, b)
//     }

//     default fn one_extension(&mut self) -> Self::ExtensionVar {
//         let one = self.one();
//         self.from_base_field(one)
//     }

//     default fn zero_extension(&mut self) -> Self::ExtensionVar {
//         let zero = self.zero();
//         self.from_base_field(zero)
//     }
// }
