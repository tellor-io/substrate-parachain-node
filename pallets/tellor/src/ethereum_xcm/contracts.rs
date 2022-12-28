use super::*;
use sp_core::{bounded::BoundedVec, ConstU32};

const BEGIN_DISPUTE_FUNCTION_SELECTOR: [u8; 4] = [127, 182, 134, 152]; // beginDispute(uint32)

pub(crate) fn begin_dispute(para_id: u32) -> BoundedVec<u8, ConstU32<MAX_ETHEREUM_XCM_INPUT_SIZE>> {
	let mut call = BEGIN_DISPUTE_FUNCTION_SELECTOR.to_vec();
	call.extend(encode_parachain_id(para_id)); // Add paraId parameter
	call.try_into()
		.expect("encoded function call smaller than max ethereum xcm input size")
}

fn encode_parachain_id(para_id: u32) -> [u8; 32] {
	let mut encoded = [0u8; 32];
	encoded[28..].copy_from_slice(&para_id.to_be_bytes());
	encoded
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;
	use sp_core::{bytes::from_hex, keccak_256};

	// https://docs.soliditylang.org/en/v0.8.17/abi-spec.html#function-selector
	fn encode_function_selector(function_signature: &str) -> [u8; 4] {
		keccak_256(function_signature.as_bytes())[..]
			.try_into()
			.expect("keccak256 always returns 32-bytes")
	}

	#[test]
	fn encodes_number() {
		let mut amount: [u8; 32] = [0; 32];
		U256::from(12345u128).to_big_endian(&mut amount);
		assert_eq!(
			from_hex("0000000000000000000000000000000000000000000000000000000000003039").unwrap(),
			amount
		)
	}

	#[test]
	fn encodes_parachain_id() {
		assert_eq!(
			from_hex("0000000000000000000000000000000000000000000000000000000000000bb8").unwrap(),
			encode_parachain_id(3000)
		)
	}

	#[test]
	fn encodes_begin_dispute() {
		assert_eq!(
			from_hex("7fb686980000000000000000000000000000000000000000000000000000000000000bb8")
				.unwrap(),
			begin_dispute(3000)[..]
		)
	}
}
