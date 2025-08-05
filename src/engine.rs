use crate::card::{Book, Card};
use crate::printer::PrettyDisplay;
use crate::{Ask, AskOutcome, Declare, Event};
use num_rational::Ratio;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::vec::Vec;
use strum::IntoEnumIterator;

trait ToBits {
    fn to_bits(self) -> Vec<bool>;
}

impl ToBits for u64 {
    fn to_bits(self) -> Vec<bool> {
        (0..64).map(|i| (self & 1 << i) != 0).collect()
    }
}

#[derive(Debug, Clone)]
struct Slot {
    possible: u64,
    owner: usize,
    dirty: bool,
}

#[derive(Debug)]
pub struct Engine {
    num_players: usize,
    num_cards: usize,
    player_idx: usize,
    slots: Vec<Slot>,
    pub request: EventRequest,
}

#[derive(Debug, Clone)]
pub enum EventRequest {
    Ask {
        askee: usize,
        card: Card,
    },
    Declare {
        book: Book,
        guessed_cards: HashMap<usize, HashSet<Card>>,
    },
    None,
}

impl Card {
    fn mask(&self) -> u64 {
        1 << self.num
    }
}

impl Book {
    fn mask(&self) -> u64 {
        self.cards()
            .into_iter()
            .fold(0, |acc, card| acc | card.mask())
    }
}

impl Engine {
    pub fn init(num_players: usize, num_cards: usize, player: usize, cards: &[Card]) -> Self {
        let default_mask = cards
            .iter()
            .fold((1 << num_cards) - 1, |acc, card| acc ^ card.mask());
        let mut cards = cards.iter();
        let slots = (0..num_cards)
            .map(|i| {
                let owner = i / (num_cards / num_players);
                Slot {
                    possible: if owner == player {
                        cards.next().unwrap().mask()
                    } else {
                        default_mask
                    },
                    owner,
                    dirty: false,
                }
            })
            .collect();

        Engine {
            num_players,
            num_cards,
            player_idx: player,
            slots,
            request: EventRequest::None,
        }
    }

    pub fn update(&mut self, event: Event) {
        match event {
            Event::Ask(Ask {
                asker,
                askee,
                card,
                outcome: AskOutcome::Success,
            }) => {
                // Asker has 1 card of the book
                self.has_book(asker, card.book());
                self.move_card(askee, asker, card);
            }
            Event::Ask(Ask {
                asker,
                askee,
                card,
                outcome: AskOutcome::Failure,
            }) => {
                // Asker has 1 card of the book
                // Askee does not have the card
                self.has_book(asker, card.book());
                self.not_own_card(asker, card);
                self.not_own_card(askee, card);
            }
            Event::Declare(Declare {
                book, actual_cards, ..
            }) => {
                for (player, cards) in actual_cards.iter() {
                    for card in cards {
                        // TODO: is there a more efficient way to do this
                        let idx = self.find_card(&self.slots, *player, *card).unwrap();
                        self.slots.remove(idx);
                    }
                }
                for slot in self.slots.iter_mut() {
                    if slot.possible & book.mask() != 0 {
                        slot.dirty = true;
                        slot.possible &= !book.mask();
                    }
                }
            }
        }

        self.prune();
        self.update_request();
    }

    pub fn update_request(&mut self) {
        // DECLARATION
        let team = self
            .slots
            .iter()
            .filter(|slot| slot.owner % 2 == self.player_idx % 2 && slot.possible.count_ones() == 1)
            .fold(0, |acc, slot| acc | slot.possible);
        for book in Book::iter() {
            if team & book.mask() == book.mask() {
                let mut guessed_cards = HashMap::<usize, HashSet<Card>>::from_iter(
                    (self.player_idx % 2..self.num_players)
                        .step_by(2)
                        .map(|p| (p, HashSet::new())),
                );
                for slot in self.slots.iter() {
                    if slot.owner % 2 == self.player_idx % 2 && slot.possible & book.mask() != 0 {
                        guessed_cards.get_mut(&slot.owner).unwrap().insert(Card {
                            num: slot.possible.trailing_zeros() as u8,
                        });
                    }
                }
                self.request = EventRequest::Declare {
                    book,
                    guessed_cards,
                };
                return;
            }
        }

        // ASK
        let owned = self
            .slots
            .iter()
            .filter(|slot| slot.owner == self.player_idx)
            .fold(0, |acc, slot| acc | slot.possible);
        let mut counts = vec![vec![0u8; self.num_cards]; self.num_players];
        self.slots.iter().for_each(|slot| {
            counts[slot.owner]
                .iter_mut()
                .zip(slot.possible.to_bits().iter())
                .for_each(|(count, &possible)| {
                    if possible {
                        *count += 1;
                    }
                })
        });

        // Highest proportion
        let denominator: Vec<u8> = (0..self.num_cards)
            .map(|col| counts.iter().map(|row| row[col]).sum())
            .collect();
        self.request = EventRequest::None;
        let mut best_chance = None;
        for num in 0..self.num_cards {
            if owned & 1 << num != 0 || owned & (Card { num: num as u8 }).book().mask() == 0 {
                continue;
            }
            for player in ((self.player_idx % 2) ^ 1..self.num_players).step_by(2) {
                let chance = Ratio::new(counts[player][num], denominator[num]);
                if best_chance.map_or(true, |best| chance > best || chance == best && rand::random_bool(1.0 / 2.0)) {
                    self.request = EventRequest::Ask {
                        askee: player,
                        card: Card { num: num as u8 },
                    };
                    best_chance = Some(chance);
                    if chance == 1.into() {
                        break;
                    }
                }
            }
        }
    }

    /// Player owns book. Update a Slot if player does not already
    /// have a card of that book.
    fn has_book(&mut self, player: usize, book: Book) {
        for slot in self.slots.iter_mut() {
            if slot.owner == player && slot.possible & !book.mask() == 0 {
                return; // Player already owns the book
            }
        }

        for slot in self.slots.iter_mut() {
            if slot.owner == player && slot.possible & book.mask() != 0 {
                slot.possible &= book.mask();
                slot.dirty = true;
                return;
            }
        }

        panic!("No slot available to add book constraint");
    }

    fn find_card(&self, slots: &Vec<Slot>, player: usize, card: Card) -> Option<usize> {
        slots
            .iter()
            .position(|slot| slot.owner == player && slot.possible == card.mask())
            .or(slots.iter().position(|slot| {
                slot.owner == player
                    && slot.possible & !card.book().mask() == 0
                    && slot.possible & card.mask() != 0
            }))
            .or(slots
                .iter()
                .position(|slot| slot.owner == player && slot.possible & card.mask() != 0))
    }

    /// Change the owner of the most constrained slot.
    fn move_card(&mut self, from: usize, to: usize, card: Card) {
        let idx = self.find_card(&self.slots, from, card).unwrap();
        let slot = &mut self.slots[idx];
        slot.owner = to;
        slot.possible = card.mask();
        slot.dirty = true;
    }

    /// Player does not own the card
    fn not_own_card(&mut self, player: usize, card: Card) {
        self.slots
            .iter_mut()
            .filter(|slot| slot.owner == player)
            .for_each(|slot| {
                slot.possible &= !card.mask();
                slot.dirty = true;
            });
    }

    pub fn to_matrix(&self) -> Vec<(usize, Vec<bool>)> {
        let mut data: Vec<(usize, Vec<bool>, u32, u64)> = self
            .slots
            .iter()
            .map(|slot| {
                let mut bits = slot.possible.to_bits();
                bits.truncate(self.num_cards);
                (slot.owner, bits, slot.possible.count_ones(), slot.possible)
            })
            .collect();
        data.sort_by_key(|(owner, _, ones, possible)| (*owner, *ones, *possible));
        data.into_iter()
            .map(|(owner, bits, _, _)| (owner, bits))
            .collect()
    }

    fn prune(&mut self) {
        while let Some(check_slot) = self.slots.iter_mut().find(|slot| slot.dirty) {
            check_slot.dirty = false;
            let mask = check_slot.possible;
            if self
                .slots
                .iter()
                .filter(|&slot| slot.possible == mask)
                .count()
                == mask.count_ones() as usize
            {
                for slot in self.slots.iter_mut() {
                    if slot.possible != mask && slot.possible & mask != 0 {
                        slot.dirty = true;
                        slot.possible &= !mask;
                    }
                }
            }
        }
    }

    pub fn assert_sanity(&self, players: &Vec<(usize, Vec<Card>)>) {
        let mut slots = self.slots.clone();
        for (idx, cards) in players {
            for card in cards.iter() {
                let idx = self.find_card(&slots, *idx, *card).unwrap_or_else(|| {
                    panic!("No valid slot for card {}", card.to_pretty_string())
                });
                slots.remove(idx);
            }
        }
        assert_eq!(slots.len(), 0, "Too many slots");
    }
}
