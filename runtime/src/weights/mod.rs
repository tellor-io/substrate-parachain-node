// This file is part of Substrate.

// Copyright (C) 2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Expose the auto generated weight files.

pub mod block_weights;
pub mod extrinsic_weights;
pub mod paritydb_weights;
pub mod rocksdb_weights;
// Tellor: This module is used to provide actual weights related to fungible assets from Moonbeam and are used in calculating accurate weights for execution on destination parachain,
// see here for moonbeam's weights: https://github.com/PureStake/moonbeam/blob/master/pallets/moonbeam-xcm-benchmarks/src/weights/moonbeam_xcm_benchmarks_fungible.rs,
// Note: Modified file and used default weights for xcm instructions as current moonbeam implementation is dependent on pallet_erc20_xcm_bridge, which is not currently applicable
#[allow(dead_code)]
pub mod moonbeam_xcm_benchmarks_fungible;
// Tellor: This module is used to provide actual weights for xcm instructions from Moonbeam and are used in calculating accurate weights for execution on destination parachain,
// see here for moonbeam's weights: https://github.com/PureStake/moonbeam/blob/master/pallets/moonbeam-xcm-benchmarks/src/weights/moonbeam_xcm_benchmarks_generic.rs
pub mod moonbeam_xcm_benchmarks_generic;

pub use block_weights::constants::BlockExecutionWeight;
pub use extrinsic_weights::constants::ExtrinsicBaseWeight;
pub use paritydb_weights::constants::ParityDbWeight;
pub use rocksdb_weights::constants::RocksDbWeight;
