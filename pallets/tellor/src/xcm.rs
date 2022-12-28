use crate::{
	ethereum_xcm::{self, MAX_ETHEREUM_XCM_INPUT_SIZE},
	Config,
};
use codec::Encode;
use sp_core::{bounded::BoundedVec, ConstU32, H160};
use sp_std::{boxed::Box, vec};
use xcm::{opaque::VersionedXcm, prelude::*};

pub(super) fn destination<T: Config>() -> Box<VersionedMultiLocation> {
	let dest = MultiLocation { parents: 1, interior: X1(Parachain(2000)) };
	Box::new(VersionedMultiLocation::V1(dest))
}

pub(super) fn build_message<T: Config>(
	contract_address: H160,
	evm_call: BoundedVec<u8, ConstU32<MAX_ETHEREUM_XCM_INPUT_SIZE>>,
) -> Box<VersionedXcm> {
	// Balances pallet on destination chain
	let self_reserve = MultiLocation { parents: 0, interior: X1(PalletInstance(3)) };
	let fees = MultiAsset { id: Concrete(self_reserve), fun: Fungible(1_000_000_000_000_000_u128) };

	let call = ethereum_xcm::transact(contract_address, evm_call, 71_000.into(), None);

	let withdrawal_assets =
		MultiAssets::from_sorted_and_deduplicated_skip_checks(vec![fees.clone()]);

	// Construct xcm message
	Box::new(VersionedXcm::from(Xcm(vec![
		WithdrawAsset(withdrawal_assets),
		BuyExecution { fees, weight_limit: Unlimited },
		Transact {
			origin_type: OriginKind::SovereignAccount,
			require_weight_at_most: 5_000_000_000u64,
			call: call.into(),
		},
	])))
	.into()
}

// The fixed index of `pallet-xcm` within various runtimes.
#[derive(Clone, Eq, PartialEq, Encode)]
#[allow(dead_code)]
pub enum Xcm {
	#[codec(index = 103u8)]
	Moonbeam(XcmCall),
	#[codec(index = 28u8)]
	Moonbase(XcmCall),
	#[codec(index = 99u8)]
	Westend(XcmCall),
}

// The fixed index of calls available within `pallet-xcm`.
#[derive(Clone, Eq, PartialEq, Encode)]
#[allow(dead_code)]
pub enum XcmCall {
	#[codec(index = 0u8)]
	Send { dest: Box<VersionedMultiLocation>, message: Box<VersionedXcm> },
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::{bytes::from_hex, H160};

	#[test]
	fn encodes_send() {
		let contract_address: H160 =
			H160::from_slice(&from_hex("0xa72f549a1a12b9b49f30a7f3aeb1f4e96389c5d8").unwrap());
		let evm_call_data = from_hex("0xd09de08a").unwrap().try_into().unwrap();
		let call = ethereum_xcm::transact(contract_address, evm_call_data, 71_000.into(), None);

		let destination =
			VersionedMultiLocation::V1(MultiLocation { parents: 0, interior: X1(Parachain(1000)) });

		let mut multi_assets = MultiAssets::new();
		multi_assets.push(MultiAsset {
			id: Concrete(MultiLocation { parents: 0, interior: X1(PalletInstance(3)) }),
			fun: Fungible(100000000000000000_u128),
		});

		let message = VersionedXcm::from(Xcm(vec![
			WithdrawAsset(multi_assets),
			BuyExecution {
				fees: MultiAsset {
					id: Concrete(MultiLocation { parents: 0, interior: X1(PalletInstance(3)) }),
					fun: Fungible(100000000000000000_u128),
				},
				weight_limit: Unlimited,
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: 4000000000_u64,
				call: call.into(),
			},
		]));

		let send = super::Xcm::Westend(XcmCall::Send {
			dest: Box::new(destination),
			message: Box::new(message),
		})
		.encode();
		assert_eq!(from_hex("0x630001000100a10f020c00040000010403001300008a5d78456301130000010403001300008a5d784563010006010300286bee7901260001581501000000000000000000000000000000000000000000000000000000000000a72f549a1a12b9b49f30a7f3aeb1f4e96389c5d8000000000000000000000000000000000000000000000000000000000000000010d09de08a00").unwrap(),
				   send);
	}
}
