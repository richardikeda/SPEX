use spex_core::error::SpexError;
use spex_core::pow::{
    generate_puzzle_output, validate_pow_nonce, PowNonceParams, PowParams,
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

#[test]
// Ensures invalid Argon2 parameter minima are rejected during output generation.
fn rejects_pow_params_below_minimum() {
    let params = build_invalid_pow_params();
    let result = generate_puzzle_output(b"recipient", b"input", params);
    assert!(result.is_err());
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
// Verifies invalid nonce lengths are rejected against the configured parameters.
fn rejects_invalid_nonce_length() {
    let params = PowNonceParams::default();
    let nonce = vec![0u8; params.nonce_len + 1];

    let result = validate_pow_nonce(&nonce, params);
    assert!(matches!(result, Err(SpexError::InvalidLength("pow nonce"))));
}
