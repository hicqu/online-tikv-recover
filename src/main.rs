use kvproto::raft_serverpb::RegionLocalState;
use std::cmp::{Ord, Ordering, PartialOrd};
use std::collections::{BTreeMap, BinaryHeap};
use std::ops::Bound::{Excluded, Included};

pub struct RecoverAction {
    pub state: RegionLocalState,
    pub create_on: Option<u64>,
}
impl RecoverAction {
    pub fn keep(state: RegionLocalState) -> Self {
        RecoverAction {
            state,
            create_on: None,
        }
    }
    pub fn create(state: RegionLocalState, store_id: u64) -> Self {
        RecoverAction {
            state,
            create_on: Some(store_id),
        }
    }
}

pub struct ReportedRegion {
    pub state: RegionLocalState,
    pub store_id: u64,
}
impl PartialEq for ReportedRegion {
    fn eq(&self, other: &Self) -> bool {
        let v1 = self.state.get_region().get_region_epoch().get_version();
        let v2 = other.state.get_region().get_region_epoch().get_version();
        v1 == v2
    }
}
impl Eq for ReportedRegion {}
impl PartialOrd for ReportedRegion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let v1 = self.state.get_region().get_region_epoch().get_version();
        let v2 = other.state.get_region().get_region_epoch().get_version();
        Some(v1.cmp(&v2))
    }
}
impl Ord for ReportedRegion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

fn start_key(rs: &RegionLocalState) -> &[u8] {
    rs.get_region().get_start_key()
}

fn end_key(rs: &RegionLocalState) -> &[u8] {
    rs.get_region().get_end_key()
}

fn region_has_valid_leader(_region_id: u64) -> bool {
    false
}

pub fn get_recover_strategy(mut region_states: Vec<ReportedRegion>) -> Vec<RecoverAction> {
    region_states.retain(|r| !region_has_valid_leader(r.state.get_region().id));
    let mut region_states = BinaryHeap::from(region_states);

    // Map[start_key] -> RecoverAction.
    let mut invalid_ranges: BTreeMap<Vec<u8>, RecoverAction> = Default::default();
    while let Some(reported_region) = region_states.pop() {
        let mut rs = reported_region.state;
        let from_store_id = reported_region.store_id;

        if invalid_ranges.is_empty() {
            let k = start_key(&rs).to_vec();
            invalid_ranges.insert(k, RecoverAction::keep(rs));
            continue;
        }

        for (sk, action) in {
            let start = Included(start_key(&rs).to_vec());
            let end = Excluded(end_key(&rs).to_vec());
            let mut range = invalid_ranges.range((start, end));

            let mut vec = Vec::new();
            while let Some((_, v)) = range.next() {
                let exists_state = &v.state;
                let exists_start_key = exists_state.get_region().get_start_key();
                let exists_end_key = exists_state.get_region().get_end_key();
                if exists_start_key == start_key(&rs) {
                    if exists_end_key >= end_key(&rs) || exists_end_key.is_empty() {
                        rs = RegionLocalState::default();
                        break;
                    }
                    rs.mut_region().set_start_key(exists_end_key.to_vec());
                } else if exists_start_key > start_key(&rs) {
                    let mut new_rs = RegionLocalState::new();
                    let new_start_key = start_key(&rs).clone();
                    new_rs.mut_region().set_start_key(new_start_key.to_vec());
                    new_rs.mut_region().set_end_key(exists_start_key.to_vec());
                    let action = RecoverAction::create(new_rs, from_store_id);
                    vec.push((new_start_key.to_vec(), action));

                    if exists_end_key.is_empty() {
                        rs = RegionLocalState::default();
                        break;
                    }
                    rs.mut_region().set_start_key(exists_end_key.to_vec());
                }
            }
            vec
        } {
            invalid_ranges.insert(sk, action);
        }

        if rs != RegionLocalState::default() {
            // TODO: complete it. There are 3 cases:
            // 1. the range is full covered by an exists range in `invalid_ranges`.
            // 2. the range doesn't overlaps with any exists range in `invalid_ranges`.
            // 3. the range overlaps with the last range in `invalid_ranges`.
        }
    }

    // TODO: Deal with ranges which overlap with all valid ranges.
    invalid_ranges.into_iter().map(|(_, v)| v).collect()
}

fn main() {}
