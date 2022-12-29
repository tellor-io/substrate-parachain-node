use crate as tellor;
use frame_support::{dispatch::DispatchResult, parameter_types, traits::Everything, PalletId};
use frame_system as system;
use frame_system::pallet_prelude::OriginFor;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use xcm::{opaque::VersionedXcm, VersionedMultiLocation};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Tellor: tellor::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const ParaId: u32 = 3000;
	pub const TellorPalletId: PalletId = PalletId(*b"py/tellr");
	// Receiving
	pub const TellorContractAddress : [u8;20] = [192,30,231,241,14,164,175,70,115,207,255,98,113,14,29,119,146,171,168,243];
	// Sending
	pub const TellorStakingContractAddress : [u8;20] = [151,9,81,161,47,151,94,103,98,72,42,202,129,229,125,90,42,78,115,244];
	pub const TellorGovernanceContractAddress : [u8;20] = [62,214,33,55,197,219,146,124,177,55,194,100,85,150,145,22,191,12,35,203];
}

impl tellor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = TellorPalletId;
	type StakingContractAddress = TellorStakingContractAddress;
	type GovernanceContractAddress = TellorGovernanceContractAddress;
	type ParaId = ParaId;
	type Xcm = Test;
}

impl crate::traits::Xcm<Test> for Test {
	fn send(
		_origin: OriginFor<Test>,
		_dest: Box<VersionedMultiLocation>,
		_message: Box<VersionedXcm>,
	) -> DispatchResult {
		todo!()
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}
