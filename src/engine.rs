use crate::card::{Book, Card};
use crate::printer::PrettyDisplay;
use crate::{Ask, AskOutcome, Declare, Event, Fish};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::rc::Rc;
use std::vec::Vec;

// Once a player is logically excluded from owning a card,
// they may only gain it again through a public event
#[derive(Debug)]
pub struct Hand {
    slots: Vec<Option<Constraint>>,
    excluded_cards: HashSet<Card>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Constraint {
    InBook(Book),
    IsCard(Card),
}

// type ProbDist = HashMap<usize, f32>;

#[derive(Debug)]
pub struct Engine {
    num_players: Rc<RefCell<usize>>,
    num_cards: Rc<RefCell<usize>>,
    hand_map: Rc<RefCell<HashMap<usize, Hand>>>,
}

impl Engine {
    pub fn init(g: &Fish) -> Self {
        let num_players = g.num_players();
        let num_cards = g.num_cards();
        let hand_map = (0..num_players)
            .map(|i| {
                (
                    i,
                    Hand {
                        slots: vec![None; num_cards / num_players],
                        excluded_cards: HashSet::new(),
                    },
                )
            })
            .collect();

        Engine {
            num_cards: Rc::new(RefCell::new(num_cards)),
            num_players: Rc::new(RefCell::new(num_players)),
            hand_map: Rc::new(RefCell::new(hand_map)),
        }
    }

    pub fn reset(&self, g: &Fish) {
        let new_engine: Engine = Engine::init(g);
        *self.num_players.borrow_mut() = new_engine.num_players.take();
        *self.num_cards.borrow_mut() = new_engine.num_cards.take();
        *self.hand_map.borrow_mut() = new_engine.hand_map.take();
    }

    pub fn register_hand(&self, player: usize, cards: &[Card]) {
        cards.iter().for_each(|card| self.has_card(player, *card));
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
                self.remove_card(askee, card);
                self.add_card(asker, card);
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
            Event::Declare(Declare { actual_cards, .. }) => {
                for (player, cards) in actual_cards.iter() {
                    for card in cards {
                        self.remove_card(*player, *card);
                    }
                }
            }
        }
    }

    /// Player owns book. Update a None constraint if player does not already
    /// have a card of that book or hold the OwnBook constraint
    pub fn has_book(&self, player: usize, book: Book) {
        let mut hand_map = self.hand_map.borrow_mut();

        let hand = hand_map.get_mut(&player).unwrap();
        hand.slots.sort_by_key(|slot| match slot {
            Some(Constraint::IsCard(_)) => 0,
            Some(Constraint::InBook(_)) => 1,
            None => 2,
        });

        for slot in hand.slots.iter_mut() {
            match slot {
                Some(Constraint::IsCard(c)) if book == c.book() => return,
                Some(Constraint::InBook(b)) if book == *b => return,
                None => {
                    *slot = Some(Constraint::InBook(book));
                    return;
                }
                _ => continue,
            }
        }
        panic!("No slot available to add book constraint");
    }

    /// Player has a card. Update constraints if there are any
    pub fn has_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();

        for (id, hand) in hand_map.iter_mut() {
            if *id == player {
                hand.slots.sort_by_key(|slot| match slot {
                    Some(Constraint::IsCard(_)) => 0,
                    Some(Constraint::InBook(_)) => 1,
                    None => 2,
                });
                if let Some(idx) = hand.slots.iter().position(|slot| match slot {
                    Some(Constraint::IsCard(c)) if *c == card => true,
                    Some(Constraint::InBook(b)) if *b == card.book() => true,
                    None => true,
                    _ => false,
                }) {
                    hand.slots[idx] = Some(Constraint::IsCard(card))
                } else {
                    panic!("No slot available to add card constraint");
                }
            } else {
                hand.excluded_cards.insert(card);
            }
        }
    }

    /// Add a card to one of the player's slots
    /// And add it to the excluded cards of all other players
    pub fn add_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();
        for (id, hand) in hand_map.iter_mut() {
            if *id == player {
                hand.excluded_cards.insert(card);
                hand.slots.push(Some(Constraint::IsCard(card)));
            } else {
                hand.excluded_cards.insert(card);
            }
        }
    }

    /// Player no longer owns a card. Remove the first OwnCard constraint,
    /// OwnBook constraint, or a None constraint in that order
    pub fn remove_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();
        let hand = hand_map.get_mut(&player).unwrap();
        hand.slots.sort_by_key(|slot| match slot {
            Some(Constraint::IsCard(_)) => 0,
            Some(Constraint::InBook(_)) => 1,
            None => 2,
        });

        if let Some(idx) = hand.slots.iter().position(|slot| match slot {
            Some(Constraint::IsCard(c)) if *c == card => true,
            Some(Constraint::InBook(b)) if *b == card.book() => true,
            None => true,
            _ => false,
        }) {
            hand.slots.remove(idx);
        } else {
            panic!("No slot available to remove");
        }
    }

    /// Players do not own the card
    pub fn not_own_card(&self, player: usize, card: Card) {
        let mut hand_map = self.hand_map.borrow_mut();
        let hand = hand_map.get_mut(&player).unwrap();
        hand.excluded_cards.insert(card);
    }

    // TODO: naive prune, need to see if incremental prune is possible
    pub fn prune(&self) -> HashMap<usize, Vec<Vec<Card>>> {
        let mut output = HashMap::new();
        let all_cards: HashSet<Card> = { 0..54 }.map(|n| Card { num: n }).collect();

        let hand_map = self.hand_map.borrow();
        for (player, hand) in hand_map.iter() {
            output.insert(
                *player,
                hand.slots
                    .iter()
                    .map(|slot| match slot {
                        Some(Constraint::IsCard(card)) => vec![*card],
                        Some(Constraint::InBook(book)) => book
                            .cards()
                            .into_iter()
                            .filter(|c| !hand.excluded_cards.contains(c))
                            .collect(),
                        None => all_cards
                            .clone()
                            .into_iter()
                            .filter(|c| !hand.excluded_cards.contains(c))
                            .collect(),
                    })
                    .collect(),
            );
        }
        output
    }

    pub fn num_players(&self) -> usize {
        return *self.num_players.borrow();
    }
}

pub type Slot = Option<Constraint>;
