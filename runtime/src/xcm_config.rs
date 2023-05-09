use super::{
	AccountId, AllPalletsWithSystem, Balances, ParachainInfo, ParachainSystem, PolkadotXcm,
	Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, TellorPalletId, WeightToFee, XcmpQueue,
};
use core::{marker::PhantomData, ops::ControlFlow};
use frame_support::{
	ensure, log, match_types, parameter_types,
	traits::{ConstU32, Contains, Everything, Nothing},
	weights::Weight,
};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use polkadot_runtime_common::impls::ToAuthor;
use sp_runtime::traits::AccountIdConversion;
use tellor::ContractLocation;
use xcm::{latest::prelude::*, CreateMatcher, MatchXcm};
use xcm_builder::{
	AccountId32Aliases, AllowExplicitUnpaidExecutionFrom, AllowTopLevelPaidExecutionFrom,
	CurrencyAdapter, EnsureXcmOrigin, FixedWeightBounds, IsConcrete, NativeAsset, ParentIsPreset,
	RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
	SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeWeightCredit,
	UsingComponents, WithComputedOrigin,
};
use xcm_executor::{traits::ShouldExecute, XcmExecutor};

// TELLOR: define EVM smart contract parachain identifier
const MOONBASE: u32 = 2000;

parameter_types! {
	pub const RelayLocation: MultiLocation = MultiLocation::parent();
	pub const RelayNetwork: Option<NetworkId> = None;
	// TELLOR: add self reserve for fee payment
	pub SelfReserve: MultiLocation = MultiLocation { parents:0, interior: Here };
	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
	pub UniversalLocation: InteriorMultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
	// TELLOR: define contract locations on evm smart contract parachain, along with origins
	pub TellorRegistry: ContractLocation = (MOONBASE, [192,30,231,241,14,164,175,70,115,207,255,98,113,14,29,119,146,171,168,243]).into();
	pub TellorGovernance: ContractLocation = (MOONBASE, [62,214,33,55,197,219,146,124,177,55,194,100,85,150,145,22,191,12,35,203]).into();
	pub TellorGovernanceOrigin: RuntimeOrigin = tellor::Origin::Governance.into();
	pub TellorStaking: ContractLocation = (MOONBASE, [151,9,81,161,47,151,94,103,98,72,42,202,129,229,125,90,42,78,115,244]).into();
	pub TellorStakingOrigin: RuntimeOrigin = tellor::Origin::Staking.into();
	pub TellorPalletAccount: AccountId = TellorPalletId::get().into_account_truncating();
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the parent `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
	// TELLOR: simply map controller contract locations to Tellor pallet account for fee payment
	tellor::LocationToAccount<TellorGovernance, TellorPalletAccount, AccountId>,
	tellor::LocationToAccount<TellorStaking, TellorPalletAccount, AccountId>,
);

/// Means for transacting assets on this chain.
pub type LocalAssetTransactor = CurrencyAdapter<
	// Use this currency:
	Balances,
	// Use this currency when it is a fungible asset matching the given location or name:
	// TELLOR: use self reserve instead of relay location
	IsConcrete<SelfReserve>,
	// Do a simple punn to convert an AccountId32 MultiLocation into a native chain account ID:
	LocationToAccountId,
	// Our chain's account ID type (we can't get away without mentioning it explicitly):
	AccountId,
	// We don't track any teleports.
	(),
>;

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
	// TELLOR: convert expected controller contract locations to origins for securing pallet dispatchables
	// Converts origin of Tellor controller contracts on relevant smart contract parachain to corresponding Tellor pallet origin
	tellor::LocationToOrigin<TellorGovernance, TellorGovernanceOrigin, RuntimeOrigin>,
	tellor::LocationToOrigin<TellorStaking, TellorStakingOrigin, RuntimeOrigin>,
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `RuntimeOrigin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<RuntimeOrigin>,
);

parameter_types! {
	// One XCM operation is 1_000_000_000 weight - almost certainly a conservative estimate.
	// TELLOR: reduce proof_size unit cost to value which works within Moonbeams DEFAULT_PROOF_SIZE value constraint
	pub UnitWeightCost: Weight = Weight::from_parts(1_000_000_000, 25 * 1024);
	pub const MaxInstructions: u32 = 100;
	pub const MaxAssetsIntoHolding: u32 = 64;
}

match_types! {
	pub type ParentOrParentsExecutivePlurality: impl Contains<MultiLocation> = {
		MultiLocation { parents: 1, interior: Here } |
		MultiLocation { parents: 1, interior: X1(Plurality { id: BodyId::Executive, .. }) }
	};
}

//TODO: move DenyThenTry to polkadot's xcm module.
/// Deny executing the xcm message if it matches any of the Deny filter regardless of anything else.
/// If it passes the Deny, and matches one of the Allow cases then it is let through.
pub struct DenyThenTry<Deny, Allow>(PhantomData<Deny>, PhantomData<Allow>)
where
	Deny: ShouldExecute,
	Allow: ShouldExecute;

impl<Deny, Allow> ShouldExecute for DenyThenTry<Deny, Allow>
where
	Deny: ShouldExecute,
	Allow: ShouldExecute,
{
	fn should_execute<RuntimeCall>(
		origin: &MultiLocation,
		message: &mut [Instruction<RuntimeCall>],
		max_weight: Weight,
		weight_credit: &mut Weight,
	) -> Result<(), ()> {
		Deny::should_execute(origin, message, max_weight, weight_credit)?;
		Allow::should_execute(origin, message, max_weight, weight_credit)
	}
}

// See issue <https://github.com/paritytech/polkadot/issues/5233>
pub struct DenyReserveTransferToRelayChain;
impl ShouldExecute for DenyReserveTransferToRelayChain {
	fn should_execute<RuntimeCall>(
		origin: &MultiLocation,
		message: &mut [Instruction<RuntimeCall>],
		_max_weight: Weight,
		_weight_credit: &mut Weight,
	) -> Result<(), ()> {
		message.matcher().match_next_inst_while(
			|_| true,
			|inst| match inst {
				InitiateReserveWithdraw {
					reserve: MultiLocation { parents: 1, interior: Here },
					..
				} |
				DepositReserveAsset {
					dest: MultiLocation { parents: 1, interior: Here }, ..
				} |
				TransferReserveAsset {
					dest: MultiLocation { parents: 1, interior: Here }, ..
				} => {
					Err(()) // Deny
				},
				// An unexpected reserve transfer has arrived from the Relay Chain. Generally,
				// `IsReserve` should not allow this, but we just log it here.
				ReserveAssetDeposited { .. }
					if matches!(origin, MultiLocation { parents: 1, interior: Here }) =>
				{
					log::warn!(
						target: "xcm::barrier",
						"Unexpected ReserveAssetDeposited from the Relay Chain",
					);
					Ok(ControlFlow::Continue(()))
				},
				_ => Ok(ControlFlow::Continue(())),
			},
		)?;

		// Permit everything else
		Ok(())
	}
}

// TELLOR: define evm smart contract parachain multilocation
match_types! {
	pub type Moonbeam: impl Contains<MultiLocation> = {
		MultiLocation { parents: 1, interior: X1(Parachain(MOONBASE)) }
	};
}

pub type Barrier = DenyThenTry<
	DenyReserveTransferToRelayChain,
	(
		TakeWeightCredit,
		WithComputedOrigin<
			(
				// TELLOR: allow DescendOrigin, only for moonbeam, required to enable calls from smart contracts
				// Could be further improved to only accept specific contract addresses
				AllowTopLevelPaidExecutionDescendOriginFirst<Moonbeam>,
				AllowTopLevelPaidExecutionFrom<Everything>,
				AllowExplicitUnpaidExecutionFrom<ParentOrParentsExecutivePlurality>,
				// ^^^ Parent and its exec plurality get free execution
			),
			UniversalLocation,
			ConstU32<8>,
		>,
	),
>;

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = XcmRouter;
	// How to withdraw and deposit an asset.
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = XcmOriginToTransactDispatchOrigin;
	type IsReserve = NativeAsset;
	type IsTeleporter = (); // Teleporting is disabled.
	type UniversalLocation = UniversalLocation;
	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	// TELLOR: use self reserve for purchasing weight credit
	type Trader = UsingComponents<WeightToFee, SelfReserve, AccountId, Balances, ToAuthor<Runtime>>;
	type ResponseHandler = PolkadotXcm;
	type AssetTrap = PolkadotXcm;
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
	type AssetLocker = ();
	type AssetExchanger = ();
	type FeeManager = ();
	type MessageExporter = ();
	type UniversalAliases = Nothing;
	type CallDispatcher = RuntimeCall;
	type SafeCallFilter = Everything;
}

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, (), ()>,
	// ..and XCMP to communicate with the sibling chains.
	XcmpQueue,
);

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
}

impl pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmExecuteFilter = Nothing;
	// ^ Disable dispatchable execute on the XCM pallet.
	// Needs to be `Everything` for local testing.
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = Everything;
	type XcmReserveTransferFilter = Nothing;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type UniversalLocation = UniversalLocation;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;

	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	// ^ Override for AdvertisedXcmVersion default
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = Balances;
	type CurrencyMatcher = ();
	type TrustedLockers = ();
	type SovereignAccountOf = LocationToAccountId;
	type MaxLockers = ConstU32<8>;
	type WeightInfo = pallet_xcm::TestWeightInfo;
	#[cfg(feature = "runtime-benchmarks")]
	type ReachableDest = ReachableDest;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

// TELLOR: add barrier implementation from Moonbeam: taken from moonbeam's xcm_primitives::barriers::AllowTopLevelPaidExecutionDescendOriginFirst
/// Barrier allowing a top level paid message with DescendOrigin instruction first
pub struct AllowTopLevelPaidExecutionDescendOriginFirst<T>(PhantomData<T>);
impl<T: Contains<MultiLocation>> ShouldExecute for AllowTopLevelPaidExecutionDescendOriginFirst<T> {
	fn should_execute<Call>(
		origin: &MultiLocation,
		message: &mut [Instruction<Call>],
		max_weight: Weight,
		_weight_credit: &mut Weight,
	) -> Result<(), ()> {
		log::trace!(
			target: "xcm::barriers",
			"AllowTopLevelPaidExecutionDescendOriginFirst origin:
			{:?}, message: {:?}, max_weight: {:?}, weight_credit: {:?}",
			origin, message, max_weight, _weight_credit,
		);
		ensure!(T::contains(origin), ());
		let mut iter = message.iter_mut();
		// Make sure the first instruction is DescendOrigin
		iter.next()
			.filter(|instruction| matches!(instruction, DescendOrigin(_)))
			.ok_or(())?;

		// Then WithdrawAsset
		iter.next()
			.filter(|instruction| matches!(instruction, WithdrawAsset(_)))
			.ok_or(())?;

		// Then BuyExecution
		let i = iter.next().ok_or(())?;
		match i {
			BuyExecution { weight_limit: Limited(ref mut weight), .. }
				if weight.all_gte(max_weight) =>
			{
				weight.set_ref_time(max_weight.ref_time());
				weight.set_proof_size(max_weight.proof_size());
				Ok(())
			},
			BuyExecution { ref mut weight_limit, .. } if weight_limit == &Unlimited => {
				*weight_limit = Limited(max_weight);
				Ok(())
			},
			_ => Err(()),
		}
	}
}
