#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

mod weights;
pub mod xcm_config;

use cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
use smallvec::smallvec;
use sp_api::impl_runtime_apis;
use sp_core::{
	bounded::BoundedVec, crypto::KeyTypeId, ConstU128, ConstU16, Get, OpaqueMetadata, H256,
};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{AccountIdLookup, BlakeTwo256, Block as BlockT, IdentifyAccount, Verify},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, MultiSignature,
};

use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_support::{
	construct_runtime,
	dispatch::DispatchClass,
	parameter_types,
	traits::{ConstU32, ConstU64, ConstU8, Everything},
	weights::{
		constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier, Weight, WeightToFeeCoefficient,
		WeightToFeeCoefficients, WeightToFeePolynomial,
	},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot,
};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use xcm::latest::prelude::*;
use xcm_config::{XcmConfig, XcmOriginToTransactDispatchOrigin};

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// Polkadot imports
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};

use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};

// XCM Imports
use xcm::latest::prelude::BodyId;
use xcm_executor::XcmExecutor;

/// Import the tellor pallet.
use tellor::{
	DisputeId, EnsureGovernance, EnsureStaking, Feed, FeedId, QueryId, Tip, Tributes, VoteResult,
};
use tellor_runtime_api::{FeedDetailsWithQueryData, SingleTipWithQueryData, VoteInfo};

use xcm::latest::{Junctions, MultiLocation, SendError, Xcm};

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
/// node's balance type.
///
/// This should typically create a mapping between the following ranges:
///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
///   - `[Balance::min, Balance::max]`
///
/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
///   - Setting it to `0` will essentially disable the weight fee.
///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// in Rococo, extrinsic base weight (smallest non-zero weight) is mapped to 1 MILLIUNIT:
		// in our template, we map to 1/10 of that, or 1/10 MILLIUNIT
		let p = MILLIUNIT / 10;
		let q = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q,
		}]
	}
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;
	use sp_runtime::{generic, traits::BlakeTwo256};

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("oracle-consumer"),
	impl_name: create_runtime_str!("oracle-consumer"),
	authoring_version: 1,
	spec_version: 1,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
};

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
pub const MILLISECS_PER_BLOCK: u64 = 12000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
//       Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

// Unit = the base number of indivisible units for balances
pub const UNIT: Balance = 1_000_000_000_000;
pub const MILLIUNIT: Balance = 1_000_000_000;
pub const MICROUNIT: Balance = 1_000_000;

/// The existential deposit. Set to 1/10 of the Connected Relay Chain.
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 0.5 of a second of compute with a 12 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
	cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64,
);

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;

	// This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
	//  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
	// `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
	// the lazy contract deletion.
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u16 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type RuntimeCall = RuntimeCall;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	/// The ubiquitous origin type.
	type RuntimeOrigin = RuntimeOrigin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// Runtime version.
	type Version = Version;
	/// Converts a module to an index of this module in the runtime.
	type PalletInfo = PalletInfo;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = Everything;
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The action to take on a Runtime Upgrade
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
	type WeightInfo = ();
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = (CollatorSelection,);
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
}

parameter_types! {
	/// Relay Chain `TransactionByteFee` / 10
	pub const TransactionByteFee: Balance = 10 * MICROUNIT;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type OperationalFeeMultiplier = ConstU8<5>;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type OutboundXcmpMessageSource = XcmpQueue;
	type DmpMessageHandler = DmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = RelayNumberStrictlyIncreases;
}

impl parachain_info::Config for Runtime {}

impl cumulus_pallet_aura_ext::Config for Runtime {}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type WeightInfo = ();
	type PriceForSiblingDelivery = ();
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = CollatorSelection;
	// Essentially just Aura, but let's be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = ();
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<100_000>;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 1000;
	pub const MinCandidates: u32 = 5;
	pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const MaxInvulnerables: u32 = 100;
	pub const ExecutiveBody: BodyId = BodyId::Executive;
}

// We allow root only to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin = EnsureRoot<AccountId>;

impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = CollatorSelectionUpdateOrigin;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinCandidates = MinCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = ();
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
}

parameter_types! {
	pub const MinimumStakeAmount: u128 = 100 * 10u128.pow(18); // 100 TRB
	// https://github.com/tellor-io/dataSpecs/blob/main/types/SpotPrice.md#trbusd-spot-price
	pub StakingTokenPriceQueryId: H256 = H256([92,19,205,156,151,219,185,143,36,41,193,1,162,168,21,14,108,122,13,218,255,97,36,238,23,106,58,65,16,103,222,208]);
	pub StakingToLocalTokenPriceQueryId: H256 = H256([252, 212, 53, 69, 139, 47, 79, 224, 14, 207, 98, 192, 81, 195, 123, 170, 138, 241, 23, 4, 53, 70, 22, 191, 191, 171, 11, 101, 130, 16, 61, 30]);
	pub const TellorPalletId: PalletId = PalletId(*b"py/tellr");
	pub XcmFeesAsset : AssetId = AssetId::Concrete(PalletInstance(3).into()); // 'Self-Reserve' on EVM parachain (aka Balances pallet)
	pub FeeLocation : Junctions = Junctions::Here; // Native currency on this parachain
}

const DECIMALS: u8 = 12;

/// Configure the tellor pallet
impl tellor::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Asset = Balances;
	type Balance = Balance;
	type Decimals = ConstU8<DECIMALS>;
	type Fee = ConstU16<10>; // 1%
	type FeeLocation = FeeLocation;
	type Governance = xcm_config::TellorGovernance;
	type GovernanceOrigin = EnsureGovernance;
	type InitialDisputeFee = ConstU128<{ (100 / 10) * (5 * 10u128.pow(DECIMALS as u32)) }>; // (100 TRB / 10) * 5, where TRB 1:5 OCP
	type MaxClaimTimestamps = ConstU32<10>;
	type MaxFundedFeeds = ConstU32<10>;
	type MaxQueryDataLength = ConstU32<512>;
	type MaxTipsPerQuery = ConstU32<10>;
	type MaxValueLength = ConstU32<256>;
	type MaxVotes = ConstU32<10>;
	type MinimumStakeAmount = MinimumStakeAmount;
	type PalletId = TellorPalletId;
	type ParachainId = ParachainId;
	type RegisterOrigin = EnsureRoot<AccountId>;
	type Registry = xcm_config::TellorRegistry;
	type StakeAmountCurrencyTarget = ConstU128<{ 500 * 10u128.pow(18) }>;
	type Staking = xcm_config::TellorStaking;
	type StakingOrigin = EnsureStaking;
	type StakingTokenPriceQueryId = StakingTokenPriceQueryId;
	type StakingToLocalTokenPriceQueryId = StakingToLocalTokenPriceQueryId;
	type Time = Timestamp;
	type UpdateStakeAmountInterval = ConstU64<{ 12 * tellor::HOURS }>;
	type WeightToFee = ConstU128<10_000>;
	type Xcm = TellorConfig;
	type XcmFeesAsset = XcmFeesAsset;
	type XcmWeightToAsset = ConstU128<50_000>; // Moonbase Alpha: https://github.com/PureStake/moonbeam/blob/f19ba9de013a1c789425d3b71e8a92d54f2191af/runtime/moonbase/src/lib.rs#L135
}

pub struct TellorConfig;
impl tellor::SendXcm for TellorConfig {
	fn send_xcm(
		interior: impl Into<Junctions>,
		dest: impl Into<MultiLocation>,
		message: Xcm<()>,
	) -> Result<XcmHash, SendError> {
		PolkadotXcm::send_xcm(interior, dest, message)
	}
}

pub struct ParachainId;
impl Get<u32> for ParachainId {
	fn get() -> u32 {
		ParachainInfo::get().into()
	}
}

/// Configure the using-tellor sample pallet
impl using_tellor::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ConfigureOrigin = EnsureRoot<AccountId>;
	type Tellor = Tellor;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		// System support stuff.
		System: frame_system = 0,
		ParachainSystem: cumulus_pallet_parachain_system = 1,
		Timestamp: pallet_timestamp = 2,
		ParachainInfo: parachain_info = 3,

		// Monetary stuff.
		Balances: pallet_balances = 10,
		TransactionPayment: pallet_transaction_payment = 11,

		// Collator support. The order of these 4 are important and shall not change.
		Authorship: pallet_authorship = 20,
		CollatorSelection: pallet_collator_selection = 21,
		Session: pallet_session = 22,
		Aura: pallet_aura = 23,
		AuraExt: cumulus_pallet_aura_ext = 24,

		// XCM helpers.
		XcmpQueue: cumulus_pallet_xcmp_queue = 30,
		PolkadotXcm: pallet_xcm = 31,
		CumulusXcm: cumulus_pallet_xcm = 32,
		DmpQueue: cumulus_pallet_dmp_queue = 33,

		Sudo: pallet_sudo,

		// Tellor
		Tellor: tellor = 40,
		UsingTellor: using_tellor = 41,
	}
);

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	frame_benchmarking::define_benchmarks!(
		[frame_system, SystemBench::<Runtime>]
		[pallet_balances, Balances]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_collator_selection, CollatorSelection]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
	);
}

type StakeInfo = tellor::StakeInfo<Balance>;
type Value = BoundedVec<u8, <Runtime as tellor::Config>::MaxValueLength>;

impl_runtime_apis! {
	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	// Tellor AutoPay API
	impl tellor_runtime_api::TellorAutoPay<Block, AccountId, Balance> for Runtime {
		fn get_current_feeds(query_id: QueryId) -> Vec<FeedId>{
			Tellor::get_current_feeds(query_id)
		}

		fn get_current_tip(query_id: QueryId) -> Balance {
			Tellor::get_current_tip(query_id)
		}

		fn get_data_feed(feed_id: FeedId) -> Option<Feed<Balance>> {
			Tellor::get_data_feed(feed_id)
		}

		fn get_funded_feed_details() -> Vec<FeedDetailsWithQueryData<Balance>> {
			Tellor::get_funded_feed_details().into_iter()
			.map(|(details, query_data)| FeedDetailsWithQueryData {
				details: details,
				query_data: query_data.to_vec()})
			.collect()
		}

		fn get_funded_feeds() -> Vec<FeedId> {
			Tellor::get_funded_feeds()
		}

		fn get_funded_query_ids() -> Vec<QueryId>{
			Tellor::get_funded_query_ids()
		}

		fn get_funded_single_tips_info() -> Vec<SingleTipWithQueryData<Balance>> {
			Tellor::get_funded_single_tips_info().into_iter()
			.map(|( query_data, tip)| SingleTipWithQueryData {
				query_data: query_data.to_vec(),
				tip
			})
			.collect()
		}

		fn get_past_tip_count(query_id: QueryId) -> u32 {
			Tellor::get_past_tip_count(query_id)
		}

		fn get_past_tips(query_id: QueryId) -> Vec<Tip<Balance>> {
			Tellor::get_past_tips(query_id)
		}

		fn get_past_tip_by_index(query_id: QueryId, index: u32) -> Option<Tip<Balance>>{
			Tellor::get_past_tip_by_index(query_id, index)
		}

		fn get_query_id_from_feed_id(feed_id: FeedId) -> Option<QueryId>{
			Tellor::get_query_id_from_feed_id(feed_id)
		}

		fn get_reward_amount(feed_id: FeedId, query_id: QueryId, timestamps: Vec<tellor::Timestamp>) -> Balance{
			Tellor::get_reward_amount(feed_id, query_id, timestamps)
		}

		fn get_reward_claimed_status(feed_id: FeedId, query_id: QueryId, timestamp: tellor::Timestamp) -> bool{
			Tellor::get_reward_claimed_status(feed_id, query_id, timestamp)
		}

		fn get_reward_claim_status_list(feed_id: FeedId, query_id: QueryId, timestamps: Vec<tellor::Timestamp>) -> Vec<bool>{
			Tellor::get_reward_claim_status_list(feed_id, query_id, timestamps)
		}

		fn get_tips_by_address(user: AccountId) -> Balance {
			Tellor::get_tips_by_address(&user)
		}
	}

	// Tellor Oracle Api
	impl tellor_runtime_api::TellorOracle<Block, AccountId, BlockNumber, StakeInfo, Value> for Runtime {
		fn get_block_number_by_timestamp(query_id: QueryId, timestamp: tellor::Timestamp) -> Option<BlockNumber> {
			Tellor::get_block_number_by_timestamp(query_id, timestamp)
		}

		fn get_current_value(query_id: QueryId) -> Option<Value> {
			Tellor::get_current_value(query_id)
		}

		fn get_data_before(query_id: QueryId, timestamp: tellor::Timestamp) -> Option<(Value, tellor::Timestamp)>{
			Tellor::get_data_before(query_id, timestamp)
		}

		fn get_new_value_count_by_query_id(query_id: QueryId) -> u32 {
			Tellor::get_new_value_count_by_query_id(query_id) as u32
		}

		fn get_report_details(query_id: QueryId, timestamp: tellor::Timestamp) -> Option<(AccountId, bool)>{
			Tellor::get_report_details(query_id, timestamp)
		}

		fn get_reporter_by_timestamp(query_id: QueryId, timestamp: tellor::Timestamp) -> Option<AccountId>{
			Tellor::get_reporter_by_timestamp(query_id, timestamp)
		}

		fn get_reporter_last_timestamp(reporter: AccountId) -> Option<tellor::Timestamp>{
			Tellor::get_reporter_last_timestamp(reporter)
		}

		fn get_reporting_lock() -> tellor::Timestamp {
			Tellor::get_reporting_lock()
		}

		fn get_reports_submitted_by_address(reporter: AccountId) -> u128 {
			Tellor::get_reports_submitted_by_address(&reporter)
		}

		fn get_reports_submitted_by_address_and_query_id(reporter: AccountId, query_id: QueryId) -> u128 {
			Tellor::get_reports_submitted_by_address_and_query_id(reporter, query_id)
		}

		fn get_stake_amount() -> Tributes {
			Tellor::get_stake_amount()
		}

		fn get_staker_info(staker: AccountId) -> Option<StakeInfo>{
			Tellor::get_staker_info(staker)
		}

		fn get_time_of_last_new_value() -> Option<tellor::Timestamp> {
			Tellor::get_time_of_last_new_value()
		}

		fn get_timestamp_by_query_id_and_index(query_id: QueryId, index: u32) -> Option<tellor::Timestamp>{
			Tellor::get_timestamp_by_query_id_and_index(query_id, index)
		}

		fn get_index_for_data_before(query_id: QueryId, timestamp: tellor::Timestamp) -> Option<u32> {
			Tellor::get_index_for_data_before(query_id, timestamp)
		}

		fn get_timestamp_index_by_timestamp(query_id: QueryId, timestamp: tellor::Timestamp) -> Option<u32> {
			Tellor::get_timestamp_index_by_timestamp(query_id, timestamp)
		}

		fn get_total_stake_amount() -> Tributes {
			Tellor::get_total_stake_amount()
		}

		fn get_total_stakers() -> u128 {
			Tellor::get_total_stakers()
		}

		fn is_in_dispute(query_id: QueryId, timestamp: tellor::Timestamp) -> bool{
			Tellor::is_in_dispute(query_id, timestamp)
		}

		fn retrieve_data(query_id: QueryId, timestamp: tellor::Timestamp) -> Option<Value>{
			Tellor::retrieve_data(query_id, timestamp)
		}
	}

	// Tellor Governance Api
	impl tellor_runtime_api::TellorGovernance<Block, AccountId, Balance, BlockNumber, Value> for Runtime {
		fn did_vote(dispute_id: DisputeId, vote_round: u8, voter: AccountId) -> bool{
			Tellor::did_vote(dispute_id, vote_round, voter)
		}

		fn get_dispute_fee() -> Balance {
			Tellor::get_dispute_fee()
		}

		fn get_disputes_by_reporter(reporter: AccountId) -> Vec<DisputeId> {
			Tellor::get_disputes_by_reporter(reporter)
		}

		fn get_dispute_info(dispute_id: DisputeId) -> Option<(QueryId, tellor::Timestamp, Value, AccountId)> {
			Tellor::get_dispute_info(dispute_id)
		}

		fn get_open_disputes_on_id(query_id: QueryId) -> u128 {
			Tellor::get_open_disputes_on_id(query_id)
		}

		fn get_vote_count() -> u128 {
			Tellor::get_vote_count()
		}

		fn get_vote_info(dispute_id: DisputeId, vote_round: u8) -> Option<(VoteInfo<Balance,BlockNumber, tellor::Timestamp>,bool,Option<VoteResult>,AccountId)> {
			Tellor::get_vote_info(dispute_id, vote_round).map(|v| (
			VoteInfo{
					vote_round: v.vote_round,
					start_date: v.start_date,
					block_number: v.block_number,
					fee: v.fee,
					tally_date: v.tally_date,
					users_does_support: v.users.does_support,
					users_against: v.users.against,
					users_invalid_query: v.users.invalid_query,
					reporters_does_support: v.reporters.does_support,
					reporters_against: v.reporters.against,
					reporters_invalid_query: v.reporters.invalid_query,
				},
			v.executed,
			v.result,
			v.initiator))
		}

		fn get_vote_rounds(dispute_id: DisputeId) -> u8 {
			Tellor::get_vote_rounds(dispute_id)
		}

		fn get_vote_tally_by_address(voter: AccountId) -> u128 {
			Tellor::get_vote_tally_by_address(&voter)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect,
		) -> Weight {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here.
			Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, TrackedStorageKey};

			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {}

			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			use frame_support::traits::WhitelistedStorageKeys;
			let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

struct CheckInherents;

impl cumulus_pallet_parachain_system::CheckInherents<Block> for CheckInherents {
	fn check_inherents(
		block: &Block,
		relay_state_proof: &cumulus_pallet_parachain_system::RelayChainStateProof,
	) -> sp_inherents::CheckInherentsResult {
		let relay_chain_slot = relay_state_proof
			.read_slot()
			.expect("Could not read the relay chain slot from the proof");

		let inherent_data =
			cumulus_primitives_timestamp::InherentDataProvider::from_relay_chain_slot_and_duration(
				relay_chain_slot,
				sp_std::time::Duration::from_secs(6),
			)
			.create_inherent_data()
			.expect("Could not create the timestamp inherent data");

		inherent_data.check_extrinsics(block)
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}
