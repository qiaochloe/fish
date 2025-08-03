use crate::card::{Book, Card};
use crate::printer::PrettyDisplay;
use crate::{Ask, AskOutcome, Declare, Event, Fish, Player};
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use std::vec::Vec;

trait ToBits {
    fn to_bits(self) -> Vec<bool>;
}

impl ToBits for u64 {
    fn to_bits(self) -> Vec<bool> {
        (0..64).map(|i| (self & 1 << i) != 0).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Slot {
    possible: u64,
    owner: usize,
    dirty: bool,
}

#[derive(Debug)]
pub struct Engine {
    num_players: Rc<RefCell<usize>>,
    num_cards: Rc<RefCell<usize>>,
    slots: Rc<RefCell<Vec<Slot>>>,
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
    pub fn init(g: &Fish) -> Self {
        let num_players = g.num_players();
        let num_cards = g.num_cards();
        let default_mask = (1 << num_cards) - 1;
        let slots = (0..num_cards)
            .map(|i| Slot {
                possible: default_mask,
                owner: i / (num_cards / num_players),
                dirty: false,
            })
            .collect();

        Engine {
            num_cards: Rc::new(RefCell::new(num_cards)),
            num_players: Rc::new(RefCell::new(num_players)),
            slots: Rc::new(RefCell::new(slots)),
        }
    }

    pub fn reset(&self, g: &Fish) {
        let new_engine: Engine = Engine::init(g);
        *self.num_players.borrow_mut() = new_engine.num_players.take();
        *self.num_cards.borrow_mut() = new_engine.num_cards.take();
        *self.slots.borrow_mut() = new_engine.slots.take();
    }

    pub fn register_hand(&self, player: usize, cards: &[Card]) {
        let mut slots = self.slots.borrow_mut();
        assert_eq!(
            cards.len(),
            slots.iter().filter(|slot| slot.owner == player).count()
        );
        slots
            .iter_mut()
            .filter(|slot| slot.owner == player)
            .zip(cards.iter())
            .for_each(|(slot, card)| {
                assert_eq!(slot.possible, ((1 << self.num_cards()) - 1));
                slot.possible = card.mask();
                slot.dirty = true;
            });
        drop(slots);
        self.prune();
    }

    pub fn update_constraints(&self, event: Event) {
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
                let mut slots = self.slots.borrow_mut();
                for (player, cards) in actual_cards.iter() {
                    for card in cards {
                        // TODO: is there a more efficient way to do this
                        let idx = self.find_card(&slots, *player, *card).unwrap();
                        slots.remove(idx);
                    }
                }
                for slot in slots.iter_mut() {
                    if slot.possible & book.mask() != 0 {
                        slot.dirty = true;
                        slot.possible &= !book.mask();
                    }
                }
            }
        }
        self.prune();
    }

    /// Player owns book. Update a Slot if player does not already
    /// have a card of that book.
    fn has_book(&self, player: usize, book: Book) {
        let mut slots = self.slots.borrow_mut();

        for slot in slots.iter_mut() {
            if slot.owner == player && slot.possible & !book.mask() == 0 {
                return; // Player already owns the book
            }
        }

        for slot in slots.iter_mut() {
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
    fn move_card(&self, from: usize, to: usize, card: Card) {
        let mut slots = self.slots.borrow_mut();
        let idx = self.find_card(&slots, from, card).unwrap();
        let slot = &mut slots[idx];
        slot.owner = to;
        slot.possible = card.mask();
        slot.dirty = true;
    }

    /// Player does not own the card
    fn not_own_card(&self, player: usize, card: Card) {
        self.slots
            .borrow_mut()
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
            .borrow()
            .iter()
            .map(|slot| {
                let mut bits = slot.possible.to_bits();
                bits.truncate(self.num_cards());
                (slot.owner, bits, slot.possible.count_ones(), slot.possible)
            })
            .collect();
        data.sort_by_key(|(owner, _, ones, possible)| (*owner, *ones, *possible));
        data.into_iter()
            .map(|(owner, bits, _, _)| (owner, bits))
            .collect()
    }

    fn prune(&self) {
        let mut slots = self.slots.borrow_mut();
        while let Some(check_slot) = slots.iter_mut().find(|slot| slot.dirty) {
            check_slot.dirty = false;
            let mask = check_slot.possible;
            if slots.iter().filter(|&slot| slot.possible == mask).count()
                == mask.count_ones() as usize
            {
                for slot in slots.iter_mut() {
                    if slot.possible != mask && slot.possible & mask != 0 {
                        slot.dirty = true;
                        slot.possible &= !mask;
                    }
                }
            }
        }
    }

    fn num_cards(&self) -> usize {
        *self.num_cards.borrow()
    }

    pub fn assert_sanity(&self, players: &Vec<Player>) {
        let mut slots = self.slots.borrow().clone();
        for player in players {
            for card in player.cards.iter() {
                let idx = self
                    .find_card(&slots, player.idx, *card)
                    .unwrap_or_else(|| {
                        panic!("No valid slot for card {}", card.to_pretty_string())
                    });
                slots.remove(idx);
            }
        }
        assert_eq!(slots.len(), 0, "Too many slots");
        println!("Constraints are valid!");
    }
}
