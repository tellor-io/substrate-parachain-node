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
	use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*, PalletId};
	use frame_system::pallet_prelude::*;
	use sp_core::{H160, U256};
	use sp_runtime::traits::AccountIdConversion;
	use sp_std::vec;

	type Address = H160;
	type Amount = U256;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type ContractAddress: Get<Address>;

		#[pallet::constant]
		type ParaId: Get<u32>;

		#[pallet::constant]
		type PalletIndex: Get<u8>;

		type Xcm: Xcm<Self>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type ContractAddress<T> = StorageValue<_, Address>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RegistrationSent,
		StakeReported { staker: Address, amount: Amount },
		ContractAddressSet { address: Address },
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		AccessDenied,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(Weight::from_ref_time(10_000) + T::DbWeight::get().writes(1))]
		pub fn report(
			origin: OriginFor<T>,
			staker: Address,
			amount: Amount,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin.clone())?;
			ensure!(who == T::PalletId::get().into_account_truncating(), Error::<T>::AccessDenied);
			Self::deposit_event(Event::StakeReported { staker, amount });
			Ok(().into())
		}

		#[pallet::weight(Weight::from_ref_time(10_000) + T::DbWeight::get().writes(1))]
		pub fn set_contract_address(
			origin: OriginFor<T>,
			address: Address,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin.clone())?;
			ContractAddress::<T>::set(Some(address));
			Self::deposit_event(Event::ContractAddressSet { address });
			Ok(().into())
		}

		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_ref_time(10_000) + T::DbWeight::get().writes(1))]
		pub fn begin_dispute(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin.clone())?;

			let begin_dispute = contracts::begin_dispute(T::ParaId::get());
			let message = crate::xcm::build_message::<T>(T::ContractAddress::get(), begin_dispute);
			T::Xcm::send(origin, crate::xcm::destination::<T>(), message)?;

			Ok(().into())
		}

		// /// An example dispatchable that may throw a custom error.
		// #[pallet::call_index(1)]
		// #[pallet::weight(Weight::from_ref_time(10_000) + T::DbWeight::get().reads_writes(1,1))]
		// pub fn cause_error(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
		// 	let _who = ensure_signed(origin)?;
		//
		// 	// Read a value from storage.
		// 	match <Something<T>>::get() {
		// 		// Return an error if the value has not been set.
		// 		None => Err(Error::<T>::NoneValue)?,
		// 		Some(old) => {
		// 			// Increment the value read from storage; will error in the event of overflow.
		// 			let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
		// 			// Update the value in storage with the incremented result.
		// 			<Something<T>>::put(new);
		// 			Ok(().into())
		// 		},
		// 	}
		// }
	}
}
