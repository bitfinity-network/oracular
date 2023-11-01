use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;

use candid::CandidType;
use did::H160;
use ic_exports::ic_cdk_timers::TimerId;
use ic_stable_structures::{
    Bound, ChunkSize, SlicedStorable, StableUnboundedMap, Storable, UnboundedMapStructure,
};
use serde::{Deserialize, Serialize};

use crate::canister::{EvmDestination, Origin};
use crate::error::{Error, Result};
use crate::memory::{MemoryType, MEMORY_MANAGER, ORACLE_STORAGE_MEMORY_ID};

/// Storage for Oracle metadata
#[derive(Debug, Default, Clone)]
pub struct OracleStorage {}

impl OracleStorage {
    /// Creates a new Oracle
    pub fn add_oracle(
        &self,
        user_address: H160,
        origin: Origin,
        timestamp: u64,
        timer_id: TimerId,
        evm: EvmDestination,
    ) {
        ORACLE_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            let metadata = StorableOracleMetadata {
                origin,
                timer_id,
                timer_interval: timestamp,
                evm: evm.clone(),
            };

            let mut map = storage.get(&user_address).unwrap_or_default();

            map.0.insert(evm.contract, metadata);
            storage.insert(&user_address, &map);
        });
    }

    pub fn get_oracle_by_address(
        &self,
        user_address: H160,
        evm_contract_address: H160,
    ) -> Result<OracleMetadata> {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow();

            let vec = storage.get(&user_address).ok_or(Error::UserNotFound)?;

            vec.0
                .get(&evm_contract_address)
                .cloned()
                .map(Into::into)
                .ok_or(Error::OracleNotFound)
        })
    }

    /// Returns the timer id of the oracle
    pub fn get_timer_id_by_address(
        &self,
        user_address: H160,
        evm_contract_address: H160,
    ) -> Result<TimerId> {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow();

            let vec = storage.get(&user_address).ok_or(Error::UserNotFound)?;

            vec.0
                .get(&evm_contract_address)
                .map(|metadata| metadata.timer_id)
                .ok_or(Error::OracleNotFound)
        })
    }

    pub fn get_user_oracles(&self, user_address: H160) -> Result<Vec<(H160, OracleMetadata)>> {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow();

            let vec = storage.get(&user_address).ok_or(Error::UserNotFound)?;

            Ok(vec
                .0
                .iter()
                .map(|(k, v)| (k.clone(), v.clone().into()))
                .collect())
        })
    }

    pub fn get_oracles(&self) -> Vec<(H160, BTreeMap<H160, OracleMetadata>)> {
        ORACLE_STORAGE.with(|storage| {
            let storage = storage.borrow();
            storage
                .iter()
                .map(|(k, v)| {
                    (
                        k,
                        v.0.iter()
                            .map(|(k, v)| (k.clone(), v.clone().into()))
                            .collect(),
                    )
                })
                .collect()
        })
    }

    pub fn remove_oracle_by_address(
        &self,
        user_address: H160,
        evm_contract_address: H160,
    ) -> Result<()> {
        ORACLE_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            let mut map = storage.get(&user_address).ok_or(Error::UserNotFound)?;

            map.0
                .remove(&evm_contract_address)
                .ok_or(Error::OracleNotFound)?;

            if map.0.is_empty() {
                storage.remove(&user_address).expect("User should exist");
            } else {
                storage.insert(&user_address, &map);
            }

            Ok(())
        })
    }

    pub fn update_oracle_metadata(
        &self,
        user_address: H160,
        evm_contract_address: H160,
        new_timer_id: Option<TimerId>,
        update_metadata: UpdateOracleMetadata,
    ) -> Result<()> {
        ORACLE_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();

            let mut metadata_collection = storage.get(&user_address).ok_or(Error::UserNotFound)?;

            let metadata = metadata_collection
                .0
                .get_mut(&evm_contract_address)
                .ok_or(Error::OracleNotFound)?;

            if let Some(origin) = update_metadata.origin {
                metadata.origin = origin;
            }
            if let Some(timestamp) = update_metadata.timestamp {
                metadata.timer_interval = timestamp;
            }
            if let Some(evm) = update_metadata.evm {
                metadata.evm = evm;
            }
            if let Some(timer_id) = new_timer_id {
                metadata.timer_id = timer_id;
            }

            storage.insert(&user_address, &metadata_collection);

            Ok(())
        })
    }

    pub fn clear(&self) {
        ORACLE_STORAGE.with(|storage| {
            let mut storage = storage.borrow_mut();
            storage.clear();
        });
    }
}

thread_local! {
    static ORACLE_STORAGE: RefCell<StableUnboundedMap<H160, MetadataCollection, MemoryType>> = RefCell::new(StableUnboundedMap::new(MEMORY_MANAGER.with(|mm|mm.get(ORACLE_STORAGE_MEMORY_ID))));
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorableOracleMetadata {
    pub origin: Origin,
    pub timer_interval: u64,
    pub timer_id: TimerId,
    pub evm: EvmDestination,
}

impl Storable for MetadataCollection {
    fn to_bytes(&self) -> Cow<[u8]> {
        did::codec::bincode_encode(&self).into()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        did::codec::bincode_decode(&bytes)
    }

    const BOUND: Bound = Bound::Unbounded;
}

/// Collection of oracle metadata
/// The key is the EVM contract address
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataCollection(BTreeMap<H160, StorableOracleMetadata>);

impl SlicedStorable for MetadataCollection {
    const CHUNK_SIZE: ChunkSize = 64;
}

/// Struct used to store the oracle metadata
#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub struct OracleMetadata {
    /// The origin of the oracle
    pub origin: Origin,
    /// The interval at which the oracle should be called
    pub timer_interval: u64,
    /// The destination of the oracle
    pub evm: EvmDestination,
}

impl From<StorableOracleMetadata> for OracleMetadata {
    fn from(storable: StorableOracleMetadata) -> Self {
        Self {
            origin: storable.origin,
            timer_interval: storable.timer_interval,
            evm: storable.evm,
        }
    }
}

/// Struct used to update the oracle metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize, CandidType)]
pub struct UpdateOracleMetadata {
    pub origin: Option<Origin>,
    pub evm: Option<EvmDestination>,
    pub timestamp: Option<u64>,
}

impl UpdateOracleMetadata {
    pub fn is_none(&self) -> bool {
        self.origin.is_none() && self.evm.is_none() && self.timestamp.is_none()
    }
}

#[cfg(test)]
mod tests {
    use slotmap::KeyData;

    use super::*;
    use crate::canister::{EvmOrigin, HttpOrigin};
    use crate::provider::Provider;

    #[test]
    fn clear_oracle_storage() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address = H160::from_slice(&[2; 20]);

        let origin = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination = EvmDestination {
            contract: evm_contract_address.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin.clone(),
            100,
            TimerId::default(),
            destination.clone(),
        );

        oracle_storage.clear();

        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address, evm_contract_address)
            .unwrap_err();

        assert_eq!(oracle_metadata, Error::UserNotFound);
    }

    #[test]
    fn test_add_oracle() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address = H160::from_slice(&[2; 20]);

        let origin = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination = EvmDestination {
            contract: evm_contract_address.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin.clone(),
            100,
            TimerId::default(),
            destination.clone(),
        );

        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address, evm_contract_address)
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin);
        assert_eq!(oracle_metadata.timer_interval, 100);
        assert_eq!(oracle_metadata.evm, destination);
    }

    #[test]
    fn test_add_multiple_oracles() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address = H160::from_slice(&[2; 20]);

        let origin1 = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination1 = EvmDestination {
            contract: evm_contract_address.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin1.clone(),
            100,
            TimerId::default(),
            destination1.clone(),
        );

        let origin2 = Origin::Evm(EvmOrigin {
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
            target_address: H160::from_slice(&[3; 20]),
            method: String::from("getPrice"),
        });

        let destination2 = EvmDestination {
            contract: H160::from_slice(&[4; 20]),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin2.clone(),
            50,
            TimerId::default(),
            destination2.clone(),
        );

        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address.clone(), evm_contract_address)
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin1);
        assert_eq!(oracle_metadata.timer_interval, 100);
        assert_eq!(oracle_metadata.evm, destination1);

        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address, H160::from_slice(&[4; 20]))
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin2);
        assert_eq!(oracle_metadata.timer_interval, 50);
        assert_eq!(oracle_metadata.evm, destination2);
    }

    #[test]
    fn test_update_oracle_metadata() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address = H160::from_slice(&[2; 20]);

        let origin1 = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination1 = EvmDestination {
            contract: evm_contract_address.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin1.clone(),
            100,
            TimerId::default(),
            destination1.clone(),
        );

        // Assert that the oracle metadata is correct
        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address.clone(), evm_contract_address.clone())
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin1);

        let new_origin = Origin::Evm(EvmOrigin {
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
            target_address: H160::from_slice(&[3; 20]),
            method: String::from("getPrice"),
        });

        // Update the oracle metadata
        let update_metadata = UpdateOracleMetadata {
            origin: Some(new_origin.clone()),
            evm: None,
            timestamp: None,
        };

        oracle_storage
            .update_oracle_metadata(
                user_address.clone(),
                evm_contract_address.clone(),
                None,
                update_metadata,
            )
            .unwrap();

        // Assert that the oracle metadata is updated
        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address, evm_contract_address)
            .unwrap();

        assert_eq!(oracle_metadata.origin, new_origin,);
    }

    #[test]
    fn test_remove_single_oracle_by_address() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address = H160::from_slice(&[2; 20]);

        let origin1 = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination1 = EvmDestination {
            contract: evm_contract_address.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin1.clone(),
            100,
            TimerId::default(),
            destination1.clone(),
        );

        // Assert that the oracle metadata is correct
        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address.clone(), evm_contract_address.clone())
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin1);

        // Remove the oracle
        oracle_storage
            .remove_oracle_by_address(user_address.clone(), evm_contract_address.clone())
            .unwrap();

        // Assert that the oracle is removed
        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address, evm_contract_address)
            .unwrap_err();

        assert_eq!(oracle_metadata, Error::UserNotFound); // If the user has no oracles, the user is removed
    }

    #[test]
    fn test_remove_multiple_oracles_by_address() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address1 = H160::from_slice(&[2; 20]);
        let evm_contract_address2 = H160::from_slice(&[3; 20]);

        let origin1 = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination1 = EvmDestination {
            contract: evm_contract_address1.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin1.clone(),
            100,
            TimerId::default(),
            destination1.clone(),
        );

        let origin2 = Origin::Evm(EvmOrigin {
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
            target_address: H160::from_slice(&[3; 20]),
            method: String::from("getPrice"),
        });

        let destination2 = EvmDestination {
            contract: evm_contract_address2.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address.clone(),
            origin2.clone(),
            50,
            TimerId::default(),
            destination2.clone(),
        );

        // Assert that the oracle metadata is correct
        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address.clone(), evm_contract_address1.clone())
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin1);

        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address.clone(), evm_contract_address2.clone())
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin2);

        // Remove the oracle
        oracle_storage
            .remove_oracle_by_address(user_address.clone(), evm_contract_address1.clone())
            .unwrap();

        // Assert that the oracle is removed
        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address.clone(), evm_contract_address1)
            .unwrap_err();

        assert_eq!(oracle_metadata, Error::OracleNotFound);

        // Assert that the other oracle is still there
        let oracle_metadata = oracle_storage
            .get_oracle_by_address(user_address, evm_contract_address2)
            .unwrap();

        assert_eq!(oracle_metadata.origin, origin2);
    }

    #[test]
    fn test_get_user_oracles() {
        let oracle_storage = OracleStorage::default();

        let user_address1 = H160::from_slice(&[1; 20]);
        let user_address2 = H160::from_slice(&[2; 20]);

        let evm_contract_address1 = H160::from_slice(&[3; 20]);
        let evm_contract_address2 = H160::from_slice(&[4; 20]);

        let origin1 = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination1 = EvmDestination {
            contract: evm_contract_address1.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        let origin2 = Origin::Evm(EvmOrigin {
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
            target_address: H160::from_slice(&[3; 20]),
            method: String::from("getPrice"),
        });

        let destination2 = EvmDestination {
            contract: evm_contract_address2.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        oracle_storage.add_oracle(
            user_address1.clone(),
            origin1.clone(),
            100,
            TimerId::default(),
            destination1.clone(),
        );

        oracle_storage.add_oracle(
            user_address2.clone(),
            origin2.clone(),
            50,
            TimerId::default(),
            destination1.clone(),
        );

        oracle_storage.add_oracle(
            user_address2.clone(),
            origin1.clone(),
            50,
            TimerId::default(),
            destination2.clone(),
        );

        let user_oracles = oracle_storage.get_user_oracles(user_address1).unwrap();

        assert_eq!(user_oracles.len(), 1);

        let user_oracles = oracle_storage.get_user_oracles(user_address2).unwrap();

        assert_eq!(user_oracles.len(), 2);

        let user_oracles = oracle_storage
            .get_user_oracles(H160::from_slice(&[5; 20]))
            .unwrap_err();

        assert_eq!(user_oracles, Error::UserNotFound);

        let user_oracles = oracle_storage.get_oracles();

        assert_eq!(user_oracles.len(), 2);
    }

    #[test]
    fn test_get_timer_id_by_address() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address = H160::from_slice(&[2; 20]);

        let origin1 = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination1 = EvmDestination {
            contract: evm_contract_address.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        let key: KeyData = serde_json::from_str(r#"{"idx":1,"version":1}"#).unwrap();

        let timer = TimerId::from(key);

        oracle_storage.add_oracle(
            user_address.clone(),
            origin1.clone(),
            100,
            timer,
            destination1.clone(),
        );

        let timer_id = oracle_storage
            .get_timer_id_by_address(user_address, evm_contract_address)
            .unwrap();

        assert_eq!(timer_id, timer);
    }

    #[test]
    fn test_oracle_addition_with_timer_id() {
        let oracle_storage = OracleStorage::default();

        let user_address = H160::from_slice(&[1; 20]);
        let evm_contract_address = H160::from_slice(&[2; 20]);

        let origin1 = Origin::Http(HttpOrigin {
            url: String::from("https://example.com"),
            json_path: String::from("data"),
        });

        let destination1 = EvmDestination {
            contract: evm_contract_address.clone(),
            provider: Provider {
                chain_id: 1,
                hostname: String::from("https://example.com"),
            },
        };

        let key: KeyData = serde_json::from_str(r#"{"idx":1,"version":1}"#).unwrap();

        let timer = TimerId::from(key);

        oracle_storage.add_oracle(
            user_address.clone(),
            origin1.clone(),
            100,
            timer,
            destination1.clone(),
        );

        let oracle_metadata = oracle_storage
            .get_timer_id_by_address(user_address, evm_contract_address)
            .unwrap();

        assert_eq!(oracle_metadata, timer);
    }
}
