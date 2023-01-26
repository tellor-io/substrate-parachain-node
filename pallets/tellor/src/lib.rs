#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod ethereum_xcm;
pub mod traits;
mod xcm;

#[frame_support::pallet]
pub mod pallet {
	use super::traits::Xcm;
	use crate::ethereum_xcm::contracts;
	use frame_support::{
		dispatch::DispatchResultWithPostInfo, error::BadOrigin, pallet_prelude::*, PalletId,
	};
	use frame_system::{pallet_prelude::*, RawOrigin};
	use sp_core::{H160, U256};
	use sp_runtime::traits::AccountIdConversion;
	use sp_std::{prelude::*, result, vec};

	type Address = H160;
	type Amount = U256;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The runtime origin type.
		type RuntimeOrigin: From<<Self as frame_system::Config>::RuntimeOrigin>
			+ Into<result::Result<Origin, <Self as Config>::RuntimeOrigin>>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type StakingContractAddress: Get<Address>;

		#[pallet::constant]
		type GovernanceContractAddress: Get<Address>;

		#[pallet::constant]
		type ParaId: Get<u32>;

		#[pallet::constant]
		type PalletIndex: Get<u8>;

		/// The caller origin, overarching type of all pallets origins.
		type PalletsOrigin: Parameter +
			Into<<Self as frame_system::Config>::RuntimeOrigin> +
			IsType<<<Self as frame_system::Config>::RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin>;

		type Xcm: Xcm<Self>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Origin for the Tellor module.
	#[pallet::origin]
	#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub enum Origin {
		/// It comes from the governance controller contract.
		Governance,
		/// It comes from the staking controller contract.
		Staking,
	}

	#[pallet::storage]
	pub type ContractAddress<T> = StorageValue<_, Address>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RegistrationSent,
		StakeReported {
			// The (remote) address of the staker.
			staker: Address,
			// The (local) address of the oracle reporter.
			reporter: T::AccountId,
			amount: Amount,
		},
		DisputeCompleted {
			outcome: DisputeOutcome,
		},
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		AccessDenied,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub enum DisputeOutcome {
		Reporter,
		Disputer,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// Notification of stake from staking contract
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_ref_time(10_000) + T::DbWeight::get().writes(1))]
		pub fn report_stake(
			origin: OriginFor<T>,
			staker: Address,
			reporter: T::AccountId,
			amount: Amount,
		) -> DispatchResultWithPostInfo {
			// ensure origin is staking controller contract
			ensure_staking(<T as Config>::RuntimeOrigin::from(origin))?;

			// todo: reclaim some xcm fees from reporter?

			Self::deposit_event(Event::StakeReported { staker, reporter, amount });
			Ok(().into())
		}

		// Sending notification to governance contract
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_ref_time(10_000) + T::DbWeight::get().writes(1))]
		pub fn begin_dispute(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;

			// todo: lock dispute fee etc.

			// Send message to begin dispute using pallet account as origin
			let origin = RawOrigin::Signed(T::PalletId::get().into_account_truncating()).into();
			let begin_dispute = contracts::begin_dispute(T::ParaId::get());
			let message =
				crate::xcm::build_message::<T>(T::GovernanceContractAddress::get(), begin_dispute);
			T::Xcm::send(origin, crate::xcm::destination::<T>(), message)?;

			Ok(().into())
		}

		// Notification of dispute outcome from governance contract
		#[pallet::weight(Weight::from_ref_time(10_000) + T::DbWeight::get().writes(1))]
		pub fn report_dispute_outcome(
			origin: OriginFor<T>,
			outcome: DisputeOutcome,
		) -> DispatchResultWithPostInfo {
			// ensure origin is pallet account
			let who = ensure_signed(origin.clone())?;
			ensure!(who == T::PalletId::get().into_account_truncating(), Error::<T>::AccessDenied);

			// todo: reclaim some xcm fees from reporter?

			Self::deposit_event(Event::DisputeCompleted { outcome });
			Ok(().into())
		}
	}

	/// Ensure that the origin `o` represents is the staking controller contract.
	/// Returns `Ok` if it does or an `Err` otherwise.
	fn ensure_staking<OuterOrigin>(o: OuterOrigin) -> Result<(), BadOrigin>
	where
		OuterOrigin: Into<Result<Origin, OuterOrigin>>,
	{
		match o.into() {
			Ok(Origin::Staking) => Ok(()),
			_ => Err(BadOrigin),
		}
	}
}
