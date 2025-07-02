use std::collections::BTreeMap;

use crate::pubkey::Pubkey;

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub struct VoteInit {
    pub node_pubkey: Pubkey,
    pub authority: Pubkey,
    pub commission: u8,
}

impl VoteInit {
    pub fn new(node_pubkey: Pubkey, authority: Pubkey, commission: u8) -> Self {
        Self {
            node_pubkey,
            authority,
            commission,
        }
    }

    pub const fn size_of() -> usize {
        32 + 32 + 32 + 1
    }
}

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct VoteState {
    /// the node that votes in this account
    pub node_pubkey: Pubkey,

    /// the signer for withdrawals
    pub authority: Pubkey,

    /// percentage (0-100) that represents what part of a rewards
    ///  payout should be given to this VoteAccount
    pub commission: u8,
}

impl VoteState {
    pub fn new(vote_init: &VoteInit) -> Self {
        Self {
            node_pubkey: vote_init.node_pubkey,
            authority: vote_init.authority,
            commission: vote_init.commission,
        }
    }

    pub const fn size_of_new() -> usize {
        32 + 32 + 1
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Self {
        bincode::deserialize(data).unwrap()
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct AuthorizedVoters {
    authorized_voters: BTreeMap<u64, Pubkey>,
}

impl AuthorizedVoters {
    pub fn new(epoch: u64, pubkey: Pubkey) -> Self {
        let mut authorized_voters = BTreeMap::new();
        authorized_voters.insert(epoch, pubkey);
        Self { authorized_voters }
    }

    pub fn get_authorized_voter(&self, epoch: u64) -> Option<Pubkey> {
        self.get_or_calculate_authorized_voter_for_epoch(epoch)
            .map(|(pubkey, _)| pubkey)
    }

    pub fn get_and_cache_authorized_voter_for_epoch(&mut self, epoch: u64) -> Option<Pubkey> {
        let res = self.get_or_calculate_authorized_voter_for_epoch(epoch);

        res.map(|(pubkey, existed)| {
            if !existed {
                self.authorized_voters.insert(epoch, pubkey);
            }
            pubkey
        })
    }

    pub fn insert(&mut self, epoch: u64, authorized_voter: Pubkey) {
        self.authorized_voters.insert(epoch, authorized_voter);
    }

    pub fn purge_authorized_voters(&mut self, current_epoch: u64) -> bool {
        // Iterate through the keys in order, filtering out the ones
        // less than the current epoch
        let expired_keys: Vec<_> = self
            .authorized_voters
            .range(0..current_epoch)
            .map(|(authorized_epoch, _)| *authorized_epoch)
            .collect();

        for key in expired_keys {
            self.authorized_voters.remove(&key);
        }

        // Have to uphold this invariant b/c this is
        // 1) The check for whether the vote state is initialized
        // 2) How future authorized voters for uninitialized epochs are set
        //    by this function
        assert!(!self.authorized_voters.is_empty());
        true
    }

    pub fn is_empty(&self) -> bool {
        self.authorized_voters.is_empty()
    }

    pub fn first(&self) -> Option<(&u64, &Pubkey)> {
        self.authorized_voters.iter().next()
    }

    pub fn last(&self) -> Option<(&u64, &Pubkey)> {
        self.authorized_voters.iter().next_back()
    }

    pub fn len(&self) -> usize {
        self.authorized_voters.len()
    }

    pub fn contains(&self, epoch: u64) -> bool {
        self.authorized_voters.get(&epoch).is_some()
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<u64, Pubkey> {
        self.authorized_voters.iter()
    }

    // Returns the authorized voter at the given epoch if the epoch is >= the
    // current epoch, and a bool indicating whether the entry for this epoch
    // exists in the self.authorized_voter map
    fn get_or_calculate_authorized_voter_for_epoch(&self, epoch: u64) -> Option<(Pubkey, bool)> {
        let res = self.authorized_voters.get(&epoch);
        if res.is_none() {
            // If no authorized voter has been set yet for this epoch,
            // this must mean the authorized voter remains unchanged
            // from the latest epoch before this one
            let res = self.authorized_voters.range(0..epoch).next_back();

            /*
            if res.is_none() {
                warn!(
                    "Tried to query for the authorized voter of an epoch earlier
                    than the current epoch. Earlier epochs have been purged"
                );
            }
            */

            res.map(|(_, pubkey)| (*pubkey, false))
        } else {
            res.map(|pubkey| (*pubkey, true))
        }
    }
}
