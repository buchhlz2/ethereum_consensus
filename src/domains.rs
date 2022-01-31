use crate::primitives::{Domain, Root};
use ssz_rs::prelude::*;

#[derive(Clone, Copy)]
pub enum DomainType {
    BeaconProposer,
    BeaconAttester,
    Randao,
    Deposit,
    VoluntaryExit,
    SelectionProof,
    AggregateAndProof,
    SyncCommittee,
    SyncCommitteeSelectionProof,
    ContributionAndProof,
}

impl DomainType {
    pub fn as_bytes(&self) -> [u8; 4] {
        let data = *self as u32;
        data.to_le_bytes()
    }
}

#[derive(Default, Debug, SimpleSerialize)]
pub struct SigningData {
    pub object_root: Root,
    pub domain: Domain,
}
