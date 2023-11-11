use log::Level;
use plonky2x::backend::circuit::Circuit;
use plonky2x::frontend::mapreduce::generator::MapReduceGenerator;
use plonky2x::frontend::uint::uint64::U64Variable;
use plonky2x::frontend::vars::U32Variable;
use plonky2x::prelude::{Bytes32Variable, CircuitBuilder, PlonkParameters};

use crate::builder::header::HeaderRangeFetcherHint;
use crate::builder::justification::{GrandpaJustificationVerifier, HintSimpleJustification};
use crate::consts::HEADERS_PER_MAP;
use crate::subchain_verification::{
    MapReduceSubchainVariable, SubChainVerifier, SubchainVerificationCtx,
};

#[derive(Clone, Debug)]
pub struct StepCircuit<
    const MAX_AUTHORITY_SET_SIZE: usize,
    const MAX_HEADER_LENGTH: usize,
    const MAX_NUM_HEADERS: usize,
> {}

impl<
        const MAX_AUTHORITY_SET_SIZE: usize,
        const MAX_HEADER_LENGTH: usize,
        const MAX_NUM_HEADERS: usize,
    > Circuit for StepCircuit<MAX_AUTHORITY_SET_SIZE, MAX_HEADER_LENGTH, MAX_NUM_HEADERS>
{
    fn define<L: PlonkParameters<D>, const D: usize>(builder: &mut CircuitBuilder<L, D>)
    where
        <<L as PlonkParameters<D>>::Config as plonky2::plonk::config::GenericConfig<D>>::Hasher:
            plonky2::plonk::config::AlgebraicHasher<L::Field>,
    {
        // Read the on-chain inputs.
        let trusted_block = builder.evm_read::<U32Variable>();
        builder.watch_with_level(
            &trusted_block,
            "step circuit input - trusted block",
            Level::Debug,
        );

        let trusted_header_hash = builder.evm_read::<Bytes32Variable>();
        builder.watch_with_level(
            &trusted_header_hash,
            "step circuit input - trusted header hash",
            Level::Debug,
        );

        let authority_set_id = builder.evm_read::<U64Variable>();
        builder.watch_with_level(
            &authority_set_id,
            "step circuit input - authority set id",
            Level::Debug,
        );

        let authority_set_hash = builder.evm_read::<Bytes32Variable>();
        builder.watch_with_level(
            &authority_set_hash,
            "step circuit input - authority set hash",
            Level::Debug,
        );

        let target_block = builder.evm_read::<U32Variable>();
        builder.watch_with_level(
            &target_block,
            "step circuit input - target block",
            Level::Debug,
        );

        let (target_header_hash, state_root_merkle_root, data_root_merkle_root) = builder
            .verify_subchain::<StepCircuit<
            MAX_AUTHORITY_SET_SIZE,
            MAX_HEADER_LENGTH,
            MAX_NUM_HEADERS,
        >, MAX_NUM_HEADERS>(
            trusted_block,
            trusted_header_hash,
            target_block,
        );

        builder.watch_with_level(
            &target_header_hash,
            "step circuit verify_subchain target header hash",
            Level::Debug,
        );

        builder.watch_with_level(
            &state_root_merkle_root,
            "step circuit verify_subchain state root merkle root",
            Level::Debug,
        );
        builder.watch_with_level(
            &data_root_merkle_root,
            "step circuit verify_subchain merkle root",
            Level::Debug,
        );

        builder.verify_simple_justification::<MAX_AUTHORITY_SET_SIZE>(
            target_block,
            target_header_hash,
            authority_set_id,
            authority_set_hash,
        );

        builder.evm_write::<Bytes32Variable>(target_header_hash);
        builder.evm_write::<Bytes32Variable>(state_root_merkle_root);
        builder.evm_write::<Bytes32Variable>(data_root_merkle_root);
    }

    fn register_generators<L: PlonkParameters<D>, const D: usize>(
        generator_registry: &mut plonky2x::prelude::HintRegistry<L, D>,
    ) where
        <<L as PlonkParameters<D>>::Config as plonky2::plonk::config::GenericConfig<D>>::Hasher:
            plonky2::plonk::config::AlgebraicHasher<L::Field>,
    {
        generator_registry
            .register_async_hint::<HeaderRangeFetcherHint<MAX_HEADER_LENGTH, HEADERS_PER_MAP>>();
        generator_registry.register_async_hint::<HintSimpleJustification<MAX_AUTHORITY_SET_SIZE>>();

        let mr_id = MapReduceGenerator::<
            L,
            SubchainVerificationCtx,
            U32Variable,
            MapReduceSubchainVariable,
            Self,
            HEADERS_PER_MAP,
            D,
        >::id();
        generator_registry.register_simple::<MapReduceGenerator<
            L,
            SubchainVerificationCtx,
            U32Variable,
            MapReduceSubchainVariable,
            Self,
            HEADERS_PER_MAP,
            D,
        >>(mr_id);
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use ethers::utils::hex;
    use plonky2x::backend::circuit::PublicInput;
    use plonky2x::prelude::{DefaultBuilder, GateRegistry, HintRegistry};

    use super::*;
    use crate::consts::MAX_HEADER_SIZE;

    #[test]
    #[cfg_attr(feature = "ci", ignore)]
    fn test_circuit_function_step() {
        env::set_var("RUST_LOG", "debug");
        env_logger::try_init().unwrap_or_default();

        const NUM_AUTHORITIES: usize = 4;
        const MAX_HEADER_LENGTH: usize = MAX_HEADER_SIZE;
        const NUM_HEADERS: usize = 36;

        let mut builder = DefaultBuilder::new();

        log::debug!("Defining circuit");
        StepCircuit::<NUM_AUTHORITIES, MAX_HEADER_LENGTH, NUM_HEADERS>::define(&mut builder);
        let circuit = builder.build();
        log::debug!("Done building circuit");

        let mut hint_registry = HintRegistry::new();
        let mut gate_registry = GateRegistry::new();
        StepCircuit::<NUM_AUTHORITIES, MAX_HEADER_LENGTH, NUM_HEADERS>::register_generators(
            &mut hint_registry,
        );
        StepCircuit::<NUM_AUTHORITIES, MAX_HEADER_LENGTH, NUM_HEADERS>::register_gates(
            &mut gate_registry,
        );

        circuit.test_serializers(&gate_registry, &hint_registry);
    }

    #[test]
    #[cfg_attr(feature = "ci", ignore)]
    fn test_circuit_with_input_bytes() {
        env::set_var("RUST_LOG", "debug");
        env_logger::try_init().unwrap_or_default();

        // This is from block 3000 with requested block 3100
        let input_bytes = hex::decode(
            "a8512f18c34b70e1533cfd5aa04f251fcb0d7be56ec570051fbad9bdb9435e6a0000000000000bb80000000000000c1c",
        )
        .unwrap();

        const NUM_AUTHORITIES: usize = 4;
        const MAX_HEADER_LENGTH: usize = 1024;
        const NUM_HEADERS: usize = 4;

        let mut builder = DefaultBuilder::new();

        log::debug!("Defining circuit");
        StepCircuit::<NUM_AUTHORITIES, MAX_HEADER_LENGTH, NUM_HEADERS>::define(&mut builder);

        log::debug!("Building circuit");
        let circuit = builder.build();
        log::debug!("Done building circuit");

        let input = PublicInput::Bytes(input_bytes);
        let (_proof, mut output) = circuit.prove(&input);
        let next_header = output.evm_read::<Bytes32Variable>();
        println!("next_header {:?}", next_header);
    }

    #[test]
    #[cfg_attr(feature = "ci", ignore)]
    fn test_step_small() {
        // Only 10 authorities in the authority set for this authority set id.
        env::set_var("RUST_LOG", "debug");
        env_logger::try_init().unwrap_or_default();

        const NUM_AUTHORITIES: usize = 5;
        const MAX_HEADER_LENGTH: usize = MAX_HEADER_SIZE;
        const NUM_HEADERS: usize = 32;
        let mut builder = DefaultBuilder::new();

        log::debug!("Defining circuit");
        StepCircuit::<NUM_AUTHORITIES, MAX_HEADER_LENGTH, NUM_HEADERS>::define(&mut builder);

        log::debug!("Building circuit");
        let circuit = builder.build();
        log::debug!("Done building circuit");

        let mut input = circuit.input();

        let trusted_header = "087ee7c739e35c46b2ac422cf683ecf6d4cb4571610efe6a5dff6f5b3d5818c9"
            .parse()
            .unwrap();
        let trusted_block = 4310u32;
        // Step to an epoch end block, so it's not reliant on a stored justification.
        let target_block = 4321u32;
        let authority_set_id = 0u64;
        let authority_set_hash = "54eb3049b763a6a84c391d53ffb5e93515a171b2dbaaa6a900ec09e3b6bb8dfb"
            .parse()
            .unwrap();

        input.evm_write::<U32Variable>(trusted_block);
        input.evm_write::<Bytes32Variable>(trusted_header);
        input.evm_write::<U64Variable>(authority_set_id);
        input.evm_write::<Bytes32Variable>(authority_set_hash);
        input.evm_write::<U32Variable>(target_block);

        log::debug!("Generating proof");
        let (proof, mut output) = circuit.prove(&input);
        log::debug!("Done generating proof");

        circuit.verify(&proof, &input, &output);
        let target_header = output.evm_read::<Bytes32Variable>();
        let state_root_merkle_root = output.evm_read::<Bytes32Variable>();
        let data_root_merkle_root = output.evm_read::<Bytes32Variable>();
        println!("target_header {:?}", target_header);
        println!("state root merkle root {:?}", state_root_merkle_root);
        println!("data root merkle root {:?}", data_root_merkle_root);
    }

    #[test]
    #[cfg_attr(feature = "ci", ignore)]
    fn test_step() {
        env::set_var("RUST_LOG", "debug");
        env_logger::try_init().unwrap_or_default();

        const NUM_AUTHORITIES: usize = 76;
        const MAX_HEADER_LENGTH: usize = MAX_HEADER_SIZE;
        const NUM_HEADERS: usize = 100;
        let mut builder = DefaultBuilder::new();

        log::debug!("Defining circuit");
        StepCircuit::<NUM_AUTHORITIES, MAX_HEADER_LENGTH, NUM_HEADERS>::define(&mut builder);

        log::debug!("Building circuit");
        let circuit = builder.build();
        log::debug!("Done building circuit");

        let mut input = circuit.input();

        let trusted_header = "087ee7c739e35c46b2ac422cf683ecf6d4cb4571610efe6a5dff6f5b3d5818c9"
            .parse()
            .unwrap();
        let trusted_block = 4310u32;
        // Step to an epoch end block, so it's not reliant on a stored justification.
        let target_block = 4321u32;
        let authority_set_id = 0u64;
        let authority_set_hash = "54eb3049b763a6a84c391d53ffb5e93515a171b2dbaaa6a900ec09e3b6bb8dfb"
            .parse()
            .unwrap();

        input.evm_write::<U32Variable>(trusted_block);
        input.evm_write::<Bytes32Variable>(trusted_header);
        input.evm_write::<U64Variable>(authority_set_id);
        input.evm_write::<Bytes32Variable>(authority_set_hash);
        input.evm_write::<U32Variable>(target_block);

        log::debug!("Generating proof");
        let (proof, mut output) = circuit.prove(&input);
        log::debug!("Done generating proof");

        circuit.verify(&proof, &input, &output);
        let target_header = output.evm_read::<Bytes32Variable>();
        let state_root_merkle_root = output.evm_read::<Bytes32Variable>();
        let data_root_merkle_root = output.evm_read::<Bytes32Variable>();
        println!("target_header {:?}", target_header);
        println!("state root merkle root {:?}", state_root_merkle_root);
        println!("data root merkle root {:?}", data_root_merkle_root);
    }
}