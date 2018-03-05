// Copyright 2018 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement.  This, along with the Licenses can be
// found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

use std::{cmp, mem};
use std::collections::BTreeMap;
use tiny_keccak::sha3_256;

/// SHA3-256 hash digest.
pub type Digest256 = [u8; 32];

/// Gossip protocol handler
pub struct Gossip {
    // (hash_msg, ((counter, rounds)))
    messages: BTreeMap<Digest256, ((u8, u8), Vec<u8>)>,
    total_peers: u64,
    // state B -> State C, which is ctrmax (lnlnN)
    hot_rounds: u8,
    // state C -> state D, which is lnlnN for state C
    cold_rounds: u8,
    // Defines the termination of rumor no matter the counter status
    // To avoid the situation that hot_rounds doesn't get increased as all other peers evolved out
    // of State B & C already.
    terminate_rounds: u8,
    // records the coutners of a message received ruing one round.
    // Which will be used for calculating counter for the local message.
    hits: BTreeMap<Digest256, Vec<u8>>,
}

impl Gossip {
    pub fn new() -> Self {
        Gossip {
            messages: BTreeMap::new(),
            total_peers: 0,
            hot_rounds: 0,
            cold_rounds: 0,
            terminate_rounds: 0,
            hits: BTreeMap::new(),
        }
    }

    pub fn add_peer(&mut self) {
        self.total_peers += 1;
        let f = self.total_peers as f64;
        self.hot_rounds = cmp::max(1, f.ln().ln() as u8);
        self.cold_rounds = cmp::max(2, 2 * self.hot_rounds);
        self.terminate_rounds = cmp::max(self.cold_rounds, f.ln() as u8);
    }

    pub fn messages(&self) -> Vec<Vec<u8>> {
        self.messages.values().map(|v| v.1.clone()).collect()
    }

    pub fn inform(&mut self, msg: Vec<u8>) {
        let msg_hash = sha3_256(&msg);
        let _ = self.messages.entry(msg_hash).or_insert(((0, 0), msg));
    }

    pub fn receive(&mut self, count: u8, msg: Vec<u8>) {
        let msg_hash = sha3_256(&msg);
        let entry = self.messages.entry(msg_hash).or_insert(
            ((count, count), msg),
        );
        // When received a copy from peer, update local counter if the incoming counter is greater.
        if (entry.0).0 < count {
            (entry.0).0 = count;
        }
        let hit_entry = self.hits.entry(msg_hash).or_insert_with(Vec::new);
        hit_entry.push(count);
    }

    pub fn get_push_list(&mut self) -> Vec<(u8, Vec<u8>)> {
        let push_list: Vec<(u8, Vec<u8>)> = self.messages
            .iter()
            .filter_map(|(_k, v)| if (v.0).0 <= self.hot_rounds &&
                (v.0).1 <= self.terminate_rounds
            {
                Some(((v.0).0, v.1.clone()))
            } else {
                None
            })
            .collect();
        for v in self.messages.values_mut() {
            if (v.0).0 > self.hot_rounds && (v.0).0 <= self.cold_rounds {
                (v.0).0 += 1;
            }
            if (v.0).1 <= self.terminate_rounds {
                (v.0).1 += 1;
            }
        }

        // Getting push_list indicates a new round is started.
        // Hence the counters need to be updated according to the peers' counter received during
        // the prev-completed round.
        let hits_map = mem::replace(&mut self.hits, BTreeMap::new());
        for (k, v) in &mut self.messages {
            if let Some(hits) = hits_map.get(k) {
                let mut less = 0;
                let mut greater_or_equal = 0;
                for hit in hits {
                    if *hit < (v.0).0 {
                        less += 1;
                    } else {
                        greater_or_equal += 1;
                    }
                }
                if greater_or_equal > less && (v.0).0 <= self.hot_rounds {
                    (v.0).0 += 1;
                }
            }
        }

        push_list
    }

    pub fn handle_pull(&self) -> Vec<(u8, Vec<u8>)> {
        self.messages
            .iter()
            .filter_map(|(_k, v)| if (v.0).0 <= self.cold_rounds &&
                (v.0).1 <= self.terminate_rounds
            {
                Some(((v.0).0, v.1.clone()))
            } else {
                None
            })
            .collect()
    }
}
