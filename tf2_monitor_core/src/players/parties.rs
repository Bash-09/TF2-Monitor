use std::collections::{HashMap, HashSet};

use steamid_ng::SteamID;

use super::friends::FriendInfo;

pub struct Parties {
    parties: Vec<HashSet<SteamID>>,
}

/// Groups accounts by those who are friends. Usually indicative of people who are
/// partied together in a casual match.
impl Parties {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            parties: Vec::new(),
        }
    }

    #[must_use]
    pub fn parties(&self) -> &[HashSet<SteamID>] {
        &self.parties
    }

    /// Given a set of players and all of their friends, as well as a list to limit which accounts will be analysed,
    /// create a set of groups where all the members in a group are friends with each other.
    pub fn find_parties(&mut self, friends: &HashMap<SteamID, FriendInfo>, connected: &[SteamID]) {
        let are_friends = |a: SteamID, b: SteamID| {
            friends
                .get(&a)
                .is_some_and(|fi| fi.friends().iter().any(|f| f.steamid == b))
        };

        let mut parties: Vec<HashSet<_>> = Vec::new();

        // For all the (connected) players
        for (&s, fi) in friends.iter().filter(|(s, _)| connected.contains(s)) {
            // See if there's any parties where the player is friends with all members
            // If yes, create a copy of that party with itself added
            let new_parties: Vec<_> = parties
                .iter()
                .filter(|&p| p.iter().all(|&s2| are_friends(s, s2)))
                .map(|p| {
                    let mut p = p.clone();
                    p.insert(s);
                    p
                })
                .collect();

            parties.extend(new_parties);

            // For all of the (connected) friends
            // Create a new party containing themself and that friend
            let new_parties: Vec<_> = fi
                .friends()
                .iter()
                .map(|f| f.steamid)
                .filter(|s2| connected.contains(s2))
                .map(|s2| {
                    let mut new_party = HashSet::new();
                    new_party.insert(s);
                    new_party.insert(s2);
                    new_party
                })
                .collect();

            parties.extend(new_parties);
        }

        self.parties.clear();

        // Add parties back
        'outer: for new_p in parties {
            let mut to_remove = Vec::new();

            for (i, other_p) in self.parties.iter().enumerate() {
                // If the party is a subset of one of the parties in the final list, skip it
                if new_p.is_subset(other_p) {
                    continue 'outer;
                }

                // If the party is a superset of one of the parties in the final list, replace it
                // (and any others which it is also a superset of)
                if new_p.is_superset(other_p) {
                    to_remove.push(i);
                }
            }

            // Remove other sets (in reverse order to not screw up indexing)
            to_remove.into_iter().rev().for_each(|i| {
                self.parties.remove(i);
            });

            // Finally add this set
            self.parties.push(new_p);
        }
    }
}

impl Default for Parties {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unreadable_literal)]

    use std::collections::{HashMap, HashSet};

    use steamid_ng::SteamID;

    use crate::players::friends::{Friend, FriendInfo};

    use super::Parties;

    #[test]
    pub fn party_generation() {
        let s: Vec<_> = [0, 1, 2, 3, 4, 5, 6]
            .iter()
            .map(|&s| SteamID::from(s))
            .collect();

        let friends: HashMap<SteamID, Vec<SteamID>> = HashMap::from([
            (s[1], vec![s[2], s[3], s[4], s[5]]),
            (s[2], vec![s[1], s[4], s[6]]),
            (s[3], vec![s[1], s[5], s[6]]),
            (s[4], vec![s[1], s[2], s[5]]),
            (s[5], vec![s[1], s[3], s[4]]),
            (s[6], vec![s[2], s[3]]),
        ]);

        let friends: HashMap<SteamID, FriendInfo> = friends
            .into_iter()
            .map(|(s, fi)| {
                (
                    s,
                    FriendInfo {
                        public: None,
                        friends: fi
                            .into_iter()
                            .map(|s| Friend {
                                steamid: s,
                                friend_since: 0,
                            })
                            .collect(),
                    },
                )
            })
            .collect();

        let mut parties = Parties::new();
        parties.find_parties(&friends, &s);

        println!("All parties:");
        for p in parties.parties() {
            print!("\t");
            for s in p {
                print!("{}, ", u64::from(*s));
            }
            println!();
        }
        println!();
        println!();

        let expected_parties: &[&[SteamID]] = &[
            &[s[1], s[2], s[4]],
            &[s[1], s[3], s[5]],
            &[s[1], s[4], s[5]],
            &[s[2], s[6]],
            &[s[3], s[6]],
        ];

        let expected_parties: Vec<HashSet<SteamID>> = expected_parties
            .iter()
            .map(|&l| l.iter().copied().collect::<HashSet<_>>())
            .collect();

        for p in &expected_parties {
            print!("Party: ");
            for s in p {
                print!("{}, ", u64::from(*s));
            }
            println!();

            assert!(parties.parties.contains(p));
        }

        assert!(parties.parties().len() == expected_parties.len());
    }
}
