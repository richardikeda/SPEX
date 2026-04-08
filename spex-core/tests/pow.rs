// SPDX-License-Identifier: MPL-2.0
use spex_core::error::SpexError;
use spex_core::pow::{
    generate_puzzle_output, validate_pow_nonce, verify_puzzle_output, PowNonceParams, PowParams,
};

// Builds a PowParams instance that is intentionally below the Argon2 minimums.
fn build_invalid_pow_params() -> PowParams {
    PowParams {
        memory_kib: 1,
        iterations: 0,
        parallelism: 0,
        output_len: 2,
    }
}

// Builds a PowParams instance that is below the Spex minimum PoW requirements.
fn build_weak_pow_params() -> PowParams {
    PowParams {
        memory_kib: 32 * 1024,
        iterations: 2,
        parallelism: 1,
        output_len: 32,
    }
}

#[test]
// Ensures invalid Argon2 parameter minima are rejected during output generation.
fn rejects_pow_params_below_minimum() {
    let params = build_invalid_pow_params();
    let result = generate_puzzle_output(b"recipient", b"input", params);
    assert!(result.is_err());
}

#[test]
// Ensures the verifier rejects puzzles that use weaker-than-allowed PoW parameters.
fn rejects_puzzle_below_minimum_requirements() {
    let params = build_weak_pow_params();
    let input = b"puzzle-input";
    let output = generate_puzzle_output(b"recipient", input, params).unwrap();

    let result = verify_puzzle_output(b"recipient", input, &output, params);

    assert!(
        matches!(result, Err(SpexError::InvalidInput(message)) if message.contains("below minimum"))
    );
}

#[test]
// Ensures puzzles at the minimum requirements verify successfully.
fn accepts_puzzle_at_minimum_requirements() {
    let params = PowParams::minimum();
    let input = b"puzzle-input";
    let output = generate_puzzle_output(b"recipient", input, params).unwrap();

    let result = verify_puzzle_output(b"recipient", input, &output, params);

    assert!(matches!(result, Ok(true)));
}

#[test]
// Confirms that distinct recipient salts produce different outputs for the same input.
fn different_salt_changes_output() {
    let params = PowParams::default();
    let input = b"puzzle-input";

    let output_a = generate_puzzle_output(b"recipient-a", input, params).unwrap();
    let output_b = generate_puzzle_output(b"recipient-b", input, params).unwrap();

    assert_ne!(output_a, output_b);
}

#[test]
// Ensures verification fails when the recipient salt does not match the output.
fn rejects_puzzle_with_incorrect_salt() {
    let params = PowParams::default();
    let input = b"puzzle-input";
    let output = generate_puzzle_output(b"recipient-a", input, params).unwrap();

    let result = verify_puzzle_output(b"recipient-b", input, &output, params).unwrap();

    assert!(!result);
}

#[test]
// Verifies invalid nonce lengths are rejected against the configured parameters.
fn rejects_invalid_nonce_length() {
    let params = PowNonceParams::default();
    let nonce = vec![0u8; params.nonce_len + 1];

    let result = validate_pow_nonce(&nonce, params);
    assert!(matches!(result, Err(SpexError::InvalidLength("pow nonce"))));
}
