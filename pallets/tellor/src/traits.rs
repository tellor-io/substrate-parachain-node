use frame_support::dispatch::DispatchResult;
use frame_system::pallet_prelude::OriginFor;
use sp_std::boxed::Box;
use xcm::{opaque::VersionedXcm, VersionedMultiLocation};

// Simple trait to avoid taking a hard dependency on pallet-xcm.
pub trait Xcm<T: frame_system::Config> {
	fn send(
		origin: OriginFor<T>,
		dest: Box<VersionedMultiLocation>,
		message: Box<VersionedXcm>,
	) -> DispatchResult;
}
