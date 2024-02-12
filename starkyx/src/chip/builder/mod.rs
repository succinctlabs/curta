pub mod arithmetic;
pub mod memory;
pub mod range_check;
pub mod shared_memory;

use core::cmp::Ordering;

use self::shared_memory::SharedMemory;
use super::arithmetic::expression::ArithmeticExpression;
use super::constraint::Constraint;
use super::instruction::clock::ClockInstruction;
use super::instruction::set::AirInstruction;
use super::memory::pointer::accumulate::PointerAccumulator;
use super::register::array::ArrayRegister;
use super::register::cubic::CubicRegister;
use super::register::element::ElementRegister;
use super::register::Register;
use super::table::accumulator::Accumulator;
use super::table::bus::channel::BusChannel;
use super::table::bus::global::Bus;
use super::table::lookup::table::LookupTable;
use super::table::lookup::values::LookupValues;
use super::table::powers::Powers;
use super::trace::data::AirTraceData;
use super::{AirParameters, Chip};
use crate::chip::register::RegisterSerializable;

#[derive(Debug, Clone)]
#[allow(clippy::type_complexity)]
pub struct AirBuilder<L: AirParameters> {
    local_index: usize,
    local_arithmetic_index: usize,
    extended_index: usize,
    pub(crate) internal_range_check: bool,
    pub(crate) shared_memory: SharedMemory,
    pub(crate) global_arithmetic: Vec<ElementRegister>,
    pub(crate) instructions: Vec<AirInstruction<L::Field, L::Instruction>>,
    pub(crate) global_instructions: Vec<AirInstruction<L::Field, L::Instruction>>,
    pub(crate) constraints: Vec<Constraint<L>>,
    pub(crate) global_constraints: Vec<Constraint<L>>,
    pub(crate) powers: Vec<Powers<L::Field, L::CubicParams>>,
    pub(crate) accumulators: Vec<Accumulator<L::Field, L::CubicParams>>,
    pub(crate) pointer_row_accumulators: Vec<PointerAccumulator<L::Field, L::CubicParams>>,
    pub(crate) pointer_global_accumulators: Vec<PointerAccumulator<L::Field, L::CubicParams>>,
    pub(crate) bus_channels: Vec<BusChannel<CubicRegister, L::CubicParams>>,
    pub(crate) buses: Vec<Bus<CubicRegister, L::CubicParams>>,
    pub(crate) lookup_values: Vec<LookupValues<L::Field, L::CubicParams>>,
    pub(crate) lookup_tables: Vec<LookupTable<L::Field, L::CubicParams>>,
    range_data: Option<(
        LookupTable<L::Field, L::CubicParams>,
        LookupValues<L::Field, L::CubicParams>,
    )>,
}

impl<L: AirParameters> AirBuilder<L> {
    pub fn new() -> Self {
        Self::new_with_shared_memory(SharedMemory::new())
    }

    pub fn init(shared_memory: SharedMemory) -> Self {
        Self::new_with_shared_memory(shared_memory)
    }

    pub fn new_with_shared_memory(shared_memory: SharedMemory) -> Self {
        Self {
            local_index: L::NUM_ARITHMETIC_COLUMNS,
            local_arithmetic_index: 0,
            extended_index: L::NUM_ARITHMETIC_COLUMNS + L::NUM_FREE_COLUMNS,
            global_arithmetic: Vec::new(),
            shared_memory,
            internal_range_check: true,
            instructions: Vec::new(),
            global_instructions: Vec::new(),
            constraints: Vec::new(),
            global_constraints: Vec::new(),
            powers: Vec::new(),
            accumulators: Vec::new(),
            pointer_row_accumulators: Vec::new(),
            pointer_global_accumulators: Vec::new(),
            bus_channels: Vec::new(),
            buses: Vec::new(),
            lookup_values: Vec::new(),
            lookup_tables: Vec::new(),
            range_data: None,
        }
    }

    pub fn constant<T: Register>(&mut self, value: &T::Value<L::Field>) -> T {
        let register = self.alloc_public::<T>();
        self.set_to_expression_public(
            &register,
            ArithmeticExpression::from_constant_vec(T::align(value).to_vec()),
        );
        register
    }

    pub(crate) fn constant_array<T: Register>(
        &mut self,
        values: &[T::Value<L::Field>],
    ) -> ArrayRegister<T> {
        let array = self.alloc_array_public::<T>(values.len());

        for (register, value) in array.iter().zip(values.iter()) {
            self.set_to_expression_public(
                &register,
                ArithmeticExpression::from_constant_vec(T::align(value).to_vec()),
            );
        }

        array
    }

    /// Prints out a log message (using the log::debug! macro) with the value of the register.
    ///
    /// The message will be presented with `RUST_LOG=debug` or `RUST_LOG=trace`.
    pub fn watch(&mut self, data: &impl Register, name: &str) {
        let register = ArrayRegister::from_register_unsafe(*data.register());
        let instruction = AirInstruction::Watch(name.to_string(), register);
        if data.is_trace() {
            self.register_air_instruction_internal(instruction);
        } else {
            self.register_global_air_instruction_internal(instruction);
        }
    }

    /// Registers an custom instruction with the builder.
    pub fn register_instruction<I>(&mut self, instruction: I)
    where
        L::Instruction: From<I>,
    {
        let instr = L::Instruction::from(instruction);
        self.register_air_instruction_internal(AirInstruction::from(instr))
    }

    /// Registers an custom instruction with the builder.
    pub fn register_global_instruction<I>(&mut self, instruction: I)
    where
        L::Instruction: From<I>,
    {
        let instr = L::Instruction::from(instruction);
        self.register_global_air_instruction_internal(AirInstruction::from(instr))
    }

    /// Registers an custom instruction with the builder.
    pub fn register_instruction_with_filter<I>(
        &mut self,
        instruction: I,
        filter: ArithmeticExpression<L::Field>,
    ) where
        L::Instruction: From<I>,
    {
        let instr = AirInstruction::from(L::Instruction::from(instruction));
        let filtered_instr = instr.as_filtered(filter);

        self.register_air_instruction_internal(filtered_instr)
    }

    /// Register an instruction into the builder.
    pub(crate) fn register_air_instruction_internal(
        &mut self,
        instruction: AirInstruction<L::Field, L::Instruction>,
    ) {
        // Add the instruction to the list
        self.instructions.push(instruction.clone());
        // Add the constraints
        self.constraints
            .push(Constraint::from_instruction_set(instruction));
    }

    /// Register a global instruction into the builder.
    pub(crate) fn register_global_air_instruction_internal(
        &mut self,
        instruction: AirInstruction<L::Field, L::Instruction>,
    ) {
        // Add the instruction to the list
        self.global_instructions.push(instruction.clone());
        // Add the constraints
        self.global_constraints
            .push(Constraint::from_instruction_set(instruction));
    }

    pub(crate) fn register_constraint<I>(&mut self, constraint: I)
    where
        Constraint<L>: From<I>,
    {
        self.constraints.push(constraint.into());
    }

    pub(crate) fn register_global_constraint<I>(&mut self, constraint: I)
    where
        Constraint<L>: From<I>,
    {
        self.global_constraints.push(constraint.into());
    }

    pub fn clock(&mut self) -> ElementRegister {
        let clk = self.alloc::<ElementRegister>();

        let instruction = AirInstruction::clock(ClockInstruction { clk });
        self.register_air_instruction_internal(instruction);
        clk
    }

    pub fn build(mut self) -> (Chip<L>, AirTraceData<L>) {
        // Register all bus constraints.
        for i in 0..self.buses.len() {
            self.register_bus_constraint(i);
        }
        // constrain all bus channels
        for channel in self.bus_channels.iter() {
            self.constraints.push(channel.clone().into());
        }

        // Add the range checks
        if (L::NUM_ARITHMETIC_COLUMNS > 0 || !self.global_arithmetic.is_empty())
            && self.internal_range_check
        {
            self.arithmetic_range_checks();
        }

        // Check the number of columns in comparison to config
        let num_free_columns = self.local_index - L::NUM_ARITHMETIC_COLUMNS;

        match num_free_columns.cmp(&L::NUM_FREE_COLUMNS) {
            Ordering::Greater => panic!(
                "Not enough free columns. Expected {} free columns, got {}.",
                num_free_columns,
                L::NUM_FREE_COLUMNS
            ),
            Ordering::Less => {
                println!(
                    "Warning: {} free columns unused",
                    L::NUM_FREE_COLUMNS - num_free_columns
                );
            }
            Ordering::Equal => {}
        }

        let num_arithmetic_columns = self.local_arithmetic_index;

        match num_arithmetic_columns.cmp(&L::NUM_ARITHMETIC_COLUMNS) {
            Ordering::Greater => panic!(
                "Not enough arithmetic columns. Expected {} arithmetic columns, got {}.",
                num_arithmetic_columns,
                L::NUM_ARITHMETIC_COLUMNS
            ),
            Ordering::Less => {
                println!(
                    "Warning: {} arithmetic columns unused",
                    L::NUM_ARITHMETIC_COLUMNS - num_arithmetic_columns
                );
            }
            Ordering::Equal => {}
        }

        let num_extended_columns =
            self.extended_index - L::NUM_ARITHMETIC_COLUMNS - L::NUM_FREE_COLUMNS;

        match num_extended_columns.cmp(&L::EXTENDED_COLUMNS) {
            Ordering::Greater => panic!(
                "Not enough extended columns. Expected {} extended columns, got {}.",
                num_extended_columns,
                L::EXTENDED_COLUMNS
            ),
            Ordering::Less => {
                println!(
                    "Warning: {} extended columns unused",
                    L::EXTENDED_COLUMNS - num_extended_columns
                );
            }
            Ordering::Equal => {}
        }

        let execution_trace_length = self.local_index;
        (
            Chip {
                constraints: self.constraints,
                global_constraints: self.global_constraints,
                num_challenges: self.shared_memory.challenge_index(),
                execution_trace_length,
                num_public_values: self.shared_memory.public_index(),
                num_global_values: self.shared_memory.global_index(),
            },
            AirTraceData {
                num_challenges: self.shared_memory.challenge_index(),
                num_public_inputs: self.shared_memory.public_index(),
                num_global_values: self.shared_memory.global_index(),
                execution_trace_length,
                instructions: self.instructions,
                global_instructions: self.global_instructions,
                powers: self.powers,
                accumulators: self.accumulators,
                pointer_row_accumulators: self.pointer_row_accumulators,
                pointer_global_accumulators: self.pointer_global_accumulators,
                bus_channels: self.bus_channels,
                buses: self.buses,
                lookup_values: self.lookup_values,
                lookup_tables: self.lookup_tables,
                range_data: self.range_data,
            },
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    pub use std::sync::mpsc::channel;

    pub use plonky2::field::goldilocks_field::GoldilocksField;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::air::fibonacci::FibonacciAir;
    pub use crate::air::parser::AirParser;
    pub use crate::air::RAir;
    pub use crate::chip::instruction::empty::EmptyInstruction;
    use crate::chip::register::element::ElementRegister;
    pub use crate::chip::register::u16::U16Register;
    pub use crate::chip::register::RegisterSerializable;
    pub use crate::chip::trace::generator::ArithmeticGenerator;
    pub use crate::math::goldilocks::cubic::GoldilocksCubicParameters;
    use crate::math::prelude::*;
    pub use crate::maybe_rayon::*;
    pub use crate::plonky2::stark::config::PoseidonGoldilocksStarkConfig;
    pub(crate) use crate::plonky2::stark::tests::{test_recursive_starky, test_starky};
    pub use crate::plonky2::stark::Starky;
    pub use crate::trace::window_parser::TraceWindowParser;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FibonacciParameters;

    impl AirParameters for FibonacciParameters {
        type Field = GoldilocksField;
        type CubicParams = GoldilocksCubicParameters;
        type Instruction = EmptyInstruction<GoldilocksField>;
        const NUM_ARITHMETIC_COLUMNS: usize = 0;
        const NUM_FREE_COLUMNS: usize = 2;
        const EXTENDED_COLUMNS: usize = 0;
    }

    #[test]
    fn test_builder_fibonacci_air() {
        type F = GoldilocksField;
        type L = FibonacciParameters;

        let mut builder = AirBuilder::<L>::new();
        let x_0 = builder.alloc::<ElementRegister>();
        let x_1 = builder.alloc::<ElementRegister>();

        // x0' <- x1
        let constr_1 = builder.set_to_expression_transition(&x_0.next(), x_1.expr());
        // x1' <- x0 + x1
        let constr_2 = builder.set_to_expression_transition(&x_1.next(), x_0.expr() + x_1.expr());

        let (mut air, mut air_data) = builder.build();
        air.num_public_values = 3;
        air_data.num_public_inputs = 3;

        let num_rows = 1 << 10;
        let public_inputs = [
            F::ZERO,
            F::ONE,
            FibonacciAir::fibonacci(num_rows - 1, F::ZERO, F::ONE),
        ];

        let generator = ArithmeticGenerator::<L>::new(air_data, num_rows);

        let writer = generator.new_writer();

        writer.write(&x_0, &F::ZERO, 0);
        writer.write(&x_1, &F::ONE, 0);

        for i in 0..num_rows {
            writer.write_instruction(&constr_1, i);
            writer.write_instruction(&constr_2, i);
        }
        let trace = generator.trace_clone();

        for window in trace.windows() {
            assert_eq!(window.local_slice.len(), 2);
            let mut window_parser = TraceWindowParser::new(window, &[], &[], &public_inputs);
            assert_eq!(window_parser.local_slice().len(), 2);
            air.eval(&mut window_parser);
        }
    }

    #[test]
    fn test_builder_fibonacci_stark() {
        type F = GoldilocksField;
        type L = FibonacciParameters;
        type SC = PoseidonGoldilocksStarkConfig;

        let _ = env_logger::builder().is_test(true).try_init();

        let mut builder = AirBuilder::<L>::new();
        let x_0 = builder.alloc::<ElementRegister>();
        let x_1 = builder.alloc::<ElementRegister>();

        // x0' <- x1
        builder.set_to_expression_transition(&x_0.next(), x_1.expr());
        // x1' <- x0 + x1
        builder.set_to_expression_transition(&x_1.next(), x_0.expr() + x_1.expr());

        builder.watch(&x_1, "x_1 fib");

        let num_rows = 1 << 10;
        let public_inputs = [
            F::ZERO,
            F::ONE,
            FibonacciAir::fibonacci(num_rows - 1, F::ZERO, F::ONE),
        ];

        let (mut air, mut air_data) = builder.build();
        air.num_public_values = 3;
        air_data.num_public_inputs = 3;

        let generator = ArithmeticGenerator::<L>::new(air_data, num_rows);

        let writer = generator.new_writer();

        writer.write(&x_0, &F::ZERO, 0);
        writer.write(&x_1, &F::ONE, 0);

        for i in 0..num_rows {
            writer.write_row_instructions(&generator.air_data, i);
        }
        let stark = Starky::new(air);
        let config = SC::standard_fast_config(num_rows);

        // Generate proof and verify as a stark
        test_starky(&stark, &config, &generator, &public_inputs);

        // Test the recursive proof.
        test_recursive_starky(stark, config, generator, &public_inputs);
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SimpleTestParameters;

    impl AirParameters for SimpleTestParameters {
        type Field = GoldilocksField;
        type CubicParams = GoldilocksCubicParameters;
        type Instruction = EmptyInstruction<GoldilocksField>;
        const NUM_ARITHMETIC_COLUMNS: usize = 3;
        const NUM_FREE_COLUMNS: usize = 4;
        const EXTENDED_COLUMNS: usize = 12;
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SimpleTestPublicParameters;

    impl AirParameters for SimpleTestPublicParameters {
        type Field = GoldilocksField;
        type CubicParams = GoldilocksCubicParameters;
        type Instruction = EmptyInstruction<GoldilocksField>;
        const NUM_ARITHMETIC_COLUMNS: usize = 0;
        const NUM_FREE_COLUMNS: usize = 4;
        const EXTENDED_COLUMNS: usize = 13;
    }

    #[test]
    fn test_builder_simple_range_check() {
        type F = GoldilocksField;
        type L = SimpleTestParameters;
        type SC = PoseidonGoldilocksStarkConfig;

        let mut builder = AirBuilder::<L>::new();
        let x_0 = builder.alloc::<U16Register>();
        let x_1 = builder.alloc::<U16Register>();
        let x_3 = builder.alloc::<U16Register>();

        let clk = builder.clock();
        let clk_expected = builder.alloc::<ElementRegister>();

        builder.assert_equal(&clk, &clk_expected);

        let (air, trace_data) = builder.build();
        let num_rows = 1 << 16;
        let generator = ArithmeticGenerator::<L>::new(trace_data, num_rows);

        let writer = generator.new_writer();
        for i in 0..num_rows {
            writer.write(&x_0, &F::ZERO, i);
            writer.write(&x_1, &F::from_canonical_usize(0), i);
            writer.write(&x_3, &F::from_canonical_usize(23), i);
            writer.write(&clk_expected, &F::from_canonical_usize(i), i);
            writer.write_row_instructions(&generator.air_data, i);
        }
        writer.write_global_instructions(&generator.air_data);
        let stark = Starky::new(air);
        let config = SC::standard_fast_config(num_rows);

        let public_inputs = writer.public().unwrap().clone();

        // Generate proof and verify as a stark
        test_starky(&stark, &config, &generator, &public_inputs);

        // Test the recursive proof.
        test_recursive_starky(stark, config, generator, &public_inputs);
    }

    #[test]
    fn test_builder_public_range_check() {
        type F = GoldilocksField;
        type L = SimpleTestPublicParameters;
        type SC = PoseidonGoldilocksStarkConfig;

        let mut builder = AirBuilder::<L>::new();
        let y_1 = builder.alloc_public::<U16Register>();

        let clk = builder.clock();
        let clk_expected = builder.alloc::<ElementRegister>();

        builder.assert_equal(&clk, &clk_expected);

        let num_rows = 1 << 14;

        let (air, trace_data) = builder.build();
        let generator = ArithmeticGenerator::<L>::new(trace_data, num_rows);

        let writer = generator.new_writer();
        writer.write(&y_1, &F::from_canonical_u32(45), 0);
        for i in 0..num_rows {
            writer.write(&clk_expected, &F::from_canonical_usize(i), i);
            writer.write_row_instructions(&generator.air_data, i);
        }

        let stark = Starky::new(air);
        let config = SC::standard_fast_config(num_rows);

        let public_inputs = writer.public.read().unwrap().clone();
        // Generate proof and verify as a stark
        test_starky(&stark, &config, &generator, &public_inputs);

        // Test the recursive proof.
        test_recursive_starky(stark, config, generator, &public_inputs);
    }
}
